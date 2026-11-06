//! XLSX parser implementation.

use std::borrow::Cow;

use crate::container::OoxmlContainer;
use crate::decode::{normalize_line_endings, resolve_general_ref};
use crate::error::{Error, Result};
use crate::model::{
    Block, Cell, CellAlignment, Document, InlineImage, Metadata, Paragraph, Resource, ResourceRole,
    Section, Table, TextRun,
};
use crate::sheet::{MergeRange, SheetTableBuilder};
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use super::shared_strings::SharedStrings;
use super::styles::Styles;

/// Sheet info from workbook.xml.
#[derive(Debug, Clone)]
struct SheetInfo {
    name: String,

    rel_id: String,
}

struct BuildSheetCellContext<'a> {
    hyperlink_map: &'a HashMap<String, String>,
    comment_map: &'a HashMap<String, String>,
    is_header: bool,
}

/// Parser for XLSX (Excel) workbooks.
pub struct XlsxParser {
    container: OoxmlContainer,
    shared_strings: SharedStrings,
    styles: Styles,
    sheets: Vec<SheetInfo>,
    relationships: HashMap<String, String>,
}

impl XlsxParser {
    /// Open an XLSX file for parsing.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let container = OoxmlContainer::open(path)?;
        Self::from_container(container)
    }

    /// Create a parser from bytes.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        let container = OoxmlContainer::from_bytes(data)?;
        Self::from_container(container)
    }

    /// Create a parser from a container.
    fn from_container(container: OoxmlContainer) -> Result<Self> {
        // Parse shared strings — absent is OK, malformed bytes must surface.
        let shared_strings = match container.read_xml_optional("xl/sharedStrings.xml")? {
            Some(xml) => SharedStrings::parse(&xml)?,
            None => SharedStrings::default(),
        };

        // Parse styles for number formats — absent is OK, malformed bytes must surface.
        let styles = match container.read_xml_optional("xl/styles.xml")? {
            Some(xml) => Styles::parse(&xml),
            None => Styles::default(),
        };

        // Parse workbook relationships
        let relationships = Self::parse_workbook_rels(&container)?;

        // Parse workbook for sheet info
        let sheets = Self::parse_workbook(&container)?;

        Ok(Self {
            container,
            shared_strings,
            styles,
            sheets,
            relationships,
        })
    }

    /// Parse workbook relationships.
    fn parse_workbook_rels(container: &OoxmlContainer) -> Result<HashMap<String, String>> {
        Ok(container
            .read_required_relationships_for_part("xl/workbook.xml")?
            .into_targets_by_id())
    }

    /// Parse workbook.xml for sheet info.
    fn parse_workbook(container: &OoxmlContainer) -> Result<Vec<SheetInfo>> {
        let mut sheets = Vec::new();
        let xml = container.read_xml("xl/workbook.xml")?;

        let mut reader = crate::decode::reader_for(&xml);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Empty(e)) | Ok(quick_xml::events::Event::Start(e))
                    if e.name().local_name().as_ref() == "sheet" =>
                {
                    let mut name = String::new();
                    let mut rel_id = String::new();

                    for attr in e.attributes().flatten() {
                        // The relationship id is `<prefix>:id` for whatever prefix the
                        // writer bound to the relationships namespace.
                        match attr.key.local_name().as_ref() {
                            "name" => {
                                name = attr.value.to_string();
                            }
                            "id" => {
                                rel_id = attr.value.to_string();
                            }
                            _ => {}
                        }
                    }

                    if !name.is_empty() {
                        sheets.push(SheetInfo { name, rel_id });
                    }
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(e) => return Err(e.into()),
                _ => {}
            }
            buf.clear();
        }

        Ok(sheets)
    }

    /// Parse the workbook and return a Document model.
    pub fn parse(&mut self) -> Result<Document> {
        let mut doc = Document::new();
        doc.format = crate::detect::FormatType::Xlsx;

        // Parse metadata
        doc.metadata = self.parse_metadata()?;

        // Resolve "Place in Cell" rich-value images once per workbook
        let rich_value_images = self.parse_rich_value_images()?;

        // Parse each sheet as a section with a table
        for (idx, sheet) in self.sheets.clone().iter().enumerate() {
            let section = self.parse_sheet_as_section(idx, sheet, &rich_value_images)?;
            doc.add_section(section);
        }

        // Extract resources (images, media)
        self.extract_resources(&mut doc)?;

        Ok(doc)
    }

    /// Stream sections (sheets) one at a time, calling `f` for each event.
    ///
    /// See [`crate::parse_file_streaming`] for the full API contract.
    #[cfg(test)]
    pub fn for_each_section<F>(
        &mut self,
        opts: crate::streaming::SectionStreamOptions,
        mut f: F,
    ) -> Result<()>
    where
        F: FnMut(crate::streaming::ParseEvent<'_>) -> std::ops::ControlFlow<()>,
    {
        let metadata = self.parse_metadata()?;
        let section_count = self.sheets.len();

        // Build image_map from resources before streaming sections.
        let mut dummy_doc = Document::new();
        self.extract_resources(&mut dummy_doc)?;
        let image_map: std::collections::HashMap<String, String> = dummy_doc
            .resources
            .iter()
            .filter_map(|(id, r)| r.filename.as_ref().map(|name| (id.clone(), name.clone())))
            .collect();

        if f(crate::streaming::ParseEvent::DocumentStart {
            metadata: &metadata,
            section_count,
            image_map,
        })
        .is_break()
        {
            return Ok(());
        }

        let rich_value_images = self.parse_rich_value_images()?;

        for (idx, sheet) in self.sheets.clone().iter().enumerate() {
            let section_result = self.parse_sheet_as_section(idx, sheet, &rich_value_images);

            match section_result {
                Ok(section) => {
                    if f(crate::streaming::ParseEvent::SectionParsed(&section)).is_break() {
                        return Ok(());
                    }
                }
                Err(e) => {
                    if opts.lenient {
                        if f(crate::streaming::ParseEvent::SectionFailed {
                            index: idx,
                            error: e,
                        })
                        .is_break()
                        {
                            return Ok(());
                        }
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        if f(crate::streaming::ParseEvent::DocumentEnd).is_break() {
            return Ok(());
        }

        if opts.extract_resources {
            for (id, resource) in dummy_doc.resources {
                let name = resource.filename.clone().unwrap_or(id);
                if f(crate::streaming::ParseEvent::ResourceExtracted {
                    name,
                    data: resource.data,
                })
                .is_break()
                {
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    /// Parse a single sheet into a Section.
    fn parse_sheet_as_section(
        &self,
        idx: usize,
        sheet: &SheetInfo,
        rich_value_images: &HashMap<u32, String>,
    ) -> Result<Section> {
        let mut section = Section::new(idx);
        section.name = Some(sheet.name.clone());

        if let Some(target) = self.relationships.get(&sheet.rel_id) {
            let sheet_path = if let Some(stripped) = target.strip_prefix('/') {
                stripped.to_string()
            } else {
                format!("xl/{}", target)
            };

            if let Some(xml) = self.container.read_xml_optional(&sheet_path)? {
                let sheet_dir = sheet_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
                let sheet_rels = self.read_optional_relationships_for_part(&sheet_path)?;
                let hyperlink_map = Self::parse_hyperlinks(&xml, &sheet_rels);
                let comment_map =
                    Self::find_and_parse_comments(&self.container, &sheet_rels, sheet_dir)?;
                let table =
                    self.parse_sheet(&xml, &hyperlink_map, &comment_map, rich_value_images)?;
                section.add_block(Block::Table(table));

                // The sheet's background picture (`<picture r:id>`), drawn behind the cells.
                section.background_image = sheet_picture_rel(&xml)
                    .and_then(|id| sheet_rels.get(&id))
                    .map(|(_, target)| {
                        let path = Self::resolve_relative_path(sheet_dir, target);
                        path.rsplit('/').next().unwrap_or(&path).to_string()
                    });

                let images = self.parse_sheet_drawing_images(&sheet_path)?;
                for image in images {
                    section.add_block(image);
                }
            }
        }

        Ok(section)
    }

    /// Parse metadata from docProps/core.xml.
    fn parse_metadata(&self) -> Result<Metadata> {
        // Use shared metadata parsing from container
        let mut meta = self.container.parse_core_metadata()?;
        // Set sheet count
        meta.page_count = Some(self.sheets.len() as u32);
        Ok(meta)
    }

    /// Parse the merged ranges (`<mergeCell ref="A1:C3">`) of a worksheet.
    fn parse_merge_cells(xml: &str) -> Vec<MergeRange> {
        let mut ranges = Vec::new();
        let mut reader = crate::decode::reader_for(xml);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Empty(ref e))
                | Ok(quick_xml::events::Event::Start(ref e))
                    if e.name().local_name().as_ref() == "mergeCell" =>
                {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == "ref" {
                            let range = attr.value.as_ref();
                            // Parse range like "A1:C3" or "G11:H11"
                            if let Some((start, end)) = range.split_once(':') {
                                if let (Some((start_col, start_row)), Some((end_col, end_row))) =
                                    (Self::parse_cell_ref(start), Self::parse_cell_ref(end))
                                {
                                    ranges.push(MergeRange {
                                        left: start_col.min(end_col),
                                        top: start_row.min(end_row),
                                        right: start_col.max(end_col),
                                        bottom: start_row.max(end_row),
                                    });
                                }
                            }
                        }
                    }
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        ranges
    }

    /// Parse a cell reference like `"A1"` into `(0-based column, 1-based row)`.
    ///
    /// ECMA-376 writes a reference as column letters then row digits, within the sheet:
    /// columns `A`..`XFD` (16 384) and rows 1..1 048 576. Anything else is no position --
    /// the cell takes the next column -- rather than a column number computed from it: the
    /// letters of a long attribute overflowed the column, and a column far past the sheet
    /// would make the row fill every column up to it.
    fn parse_cell_ref(cell_ref: &str) -> Option<(u32, u32)> {
        const MAX_COLUMNS: u32 = 16_384;
        const MAX_ROWS: u32 = 1_048_576;

        let digits = cell_ref.find(|c: char| c.is_ascii_digit())?;
        let (letters, row) = cell_ref.split_at(digits);
        if letters.is_empty()
            || letters.len() > 3
            || !letters.bytes().all(|b| b.is_ascii_alphabetic())
        {
            return None;
        }
        if !row.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }

        // Column letters in bijective base 26: A = 1, ..., Z = 26, AA = 27, ...
        let col = letters.bytes().fold(0u32, |col, b| {
            col * 26 + u32::from(b.to_ascii_uppercase() - b'A' + 1)
        });
        let row: u32 = row.parse().ok()?;
        if col > MAX_COLUMNS || !(1..=MAX_ROWS).contains(&row) {
            return None;
        }
        Some((col - 1, row))
    }

    /// Parse a worksheet XML into a table.
    ///
    /// `hyperlink_map` maps uppercase cell references (e.g. "A1") to URLs.
    /// When a cell matches, all its TextRuns get the hyperlink URL set.
    ///
    /// `comment_map` maps uppercase cell references (e.g. "A1") to comment text.
    /// When a cell matches, the comment is appended as an italic TextRun.
    fn parse_sheet(
        &self,
        xml: &str,
        hyperlink_map: &HashMap<String, String>,
        comment_map: &HashMap<String, String>,
        rich_value_images: &HashMap<u32, String>,
    ) -> Result<Table> {
        // First pass: parse merge cells
        let mut sheet = SheetTableBuilder::new(Self::parse_merge_cells(xml))?;
        let mut reader = crate::decode::reader_for(xml);
        // IMPORTANT: Don't trim text - preserve whitespace from xml:space="preserve" elements
        // Excel cell values may contain significant leading/trailing spaces
        reader.config_mut().trim_text(false);

        let mut buf = Vec::new();
        let mut in_row = false;
        let mut in_cell = false;
        let mut in_value = false;
        let mut current_cell_type: Option<String> = None;
        let mut current_cell_ref: Option<String> = None;
        let mut current_cell_style: Option<usize> = None;
        let mut current_cell_vm: Option<u32> = None;
        let mut current_cell_value = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Start(ref e)) => {
                    match e.name().local_name().as_ref() {
                        "row" => {
                            in_row = true;
                            sheet.start_row(
                                e.attributes()
                                    .flatten()
                                    .find(|a| a.key.as_ref() == "r")
                                    .and_then(|a| a.value.parse().ok()),
                            );
                        }
                        "c" if in_row => {
                            in_cell = true;
                            current_cell_type = None;
                            current_cell_ref = None;
                            current_cell_style = None;
                            current_cell_vm = None;
                            current_cell_value.clear();
                            Self::parse_cell_attributes(
                                e,
                                &mut current_cell_type,
                                &mut current_cell_ref,
                                &mut current_cell_style,
                                &mut current_cell_vm,
                            );
                        }
                        "v" if in_cell => {
                            in_value = true;
                        }
                        "t" if in_cell => {
                            // Inline string
                            in_value = true;
                        }
                        _ => {}
                    }
                }
                Ok(quick_xml::events::Event::Empty(ref e)) => {
                    match e.name().local_name().as_ref() {
                        "c" if in_row => {
                            current_cell_type = None;
                            current_cell_ref = None;
                            current_cell_style = None;
                            current_cell_vm = None;
                            current_cell_value.clear();
                            Self::parse_cell_attributes(
                                e,
                                &mut current_cell_type,
                                &mut current_cell_ref,
                                &mut current_cell_style,
                                &mut current_cell_vm,
                            );

                            let mut cell = self.build_sheet_cell(
                                &current_cell_value,
                                current_cell_type.as_deref(),
                                current_cell_style,
                                current_cell_ref.as_deref(),
                                BuildSheetCellContext {
                                    hyperlink_map,
                                    comment_map,
                                    is_header: sheet.in_first_row(),
                                },
                            )?;
                            Self::attach_rich_value_image(
                                &mut cell,
                                current_cell_vm,
                                current_cell_type.as_deref(),
                                rich_value_images,
                            );

                            sheet.place(
                                cell,
                                current_cell_ref.as_deref().and_then(Self::parse_cell_ref),
                            )?;

                            current_cell_ref = None;
                        }
                        _ => {}
                    }
                }
                Ok(quick_xml::events::Event::Text(ref e)) if in_value => {
                    let text = crate::decode::decode_text_lossy(e);
                    current_cell_value.push_str(&text);
                }
                // quick-xml 0.40+ splits entity refs out of Text; without this
                // arm a cell value like "A &amp; B" or an in-cell "&#10;" break
                // would silently lose the entity.
                Ok(quick_xml::events::Event::GeneralRef(ref e)) if in_value => {
                    current_cell_value.push_str(&resolve_general_ref(e));
                }
                Ok(quick_xml::events::Event::End(ref e)) => match e.name().local_name().as_ref() {
                    "row" => {
                        sheet.end_row();
                        in_row = false;
                    }
                    "c" => {
                        // Collapse CR that re-entered via &#13;/&#xD; refs (Excel
                        // in-cell breaks). A CRLF pair arrives as two refs, so
                        // normalize the whole accumulated value once here.
                        let cell_value =
                            normalize_line_endings(Cow::Borrowed(&current_cell_value)).into_owned();
                        let mut cell = self.build_sheet_cell(
                            &cell_value,
                            current_cell_type.as_deref(),
                            current_cell_style,
                            current_cell_ref.as_deref(),
                            BuildSheetCellContext {
                                hyperlink_map,
                                comment_map,
                                is_header: sheet.in_first_row(),
                            },
                        )?;
                        Self::attach_rich_value_image(
                            &mut cell,
                            current_cell_vm,
                            current_cell_type.as_deref(),
                            rich_value_images,
                        );

                        sheet.place(
                            cell,
                            current_cell_ref.as_deref().and_then(Self::parse_cell_ref),
                        )?;

                        in_cell = false;
                        current_cell_ref = None;
                        current_cell_vm = None;
                    }
                    "v" | "t" => {
                        in_value = false;
                    }
                    _ => {}
                },
                Ok(quick_xml::events::Event::Eof) => break,
                Err(e) => return Err(e.into()),
                _ => {}
            }
            buf.clear();
        }

        Ok(sheet.finish())
    }

    fn parse_cell_attributes(
        e: &quick_xml::events::BytesStart<'_>,
        current_cell_type: &mut Option<String>,
        current_cell_ref: &mut Option<String>,
        current_cell_style: &mut Option<usize>,
        current_cell_vm: &mut Option<u32>,
    ) {
        for attr in e.attributes().flatten() {
            match attr.key.as_ref() {
                "t" => {
                    *current_cell_type = Some(attr.value.to_string());
                }
                "r" => {
                    *current_cell_ref = Some(attr.value.to_uppercase());
                }
                "s" => {
                    *current_cell_style = attr.value.parse().ok();
                }
                "vm" => {
                    *current_cell_vm = attr.value.parse().ok();
                }
                _ => {}
            }
        }
    }

    fn build_sheet_cell(
        &self,
        current_cell_value: &str,
        current_cell_type: Option<&str>,
        current_cell_style: Option<usize>,
        current_cell_ref: Option<&str>,
        context: BuildSheetCellContext<'_>,
    ) -> Result<Cell> {
        let value =
            self.resolve_cell_value(current_cell_value, current_cell_type, current_cell_style)?;

        let hyperlink_url = current_cell_ref
            .and_then(|r| context.hyperlink_map.get(r))
            .cloned();

        let mut text_run = TextRun::plain(&value);
        if let Some(url) = hyperlink_url {
            text_run.hyperlink = Some(url);
        }

        let mut runs = vec![text_run];

        if let Some(comment_text) = current_cell_ref.and_then(|r| context.comment_map.get(r)) {
            let mut comment_run = TextRun::plain(format!(" [Comment: {}]", comment_text));
            comment_run.style.italic = true;
            runs.push(comment_run);
        }

        Ok(Cell {
            content: vec![Paragraph {
                runs,
                ..Default::default()
            }],
            nested_tables: Vec::new(),
            col_span: 1,
            row_span: 1,
            alignment: CellAlignment::Left,
            vertical_alignment: Default::default(),
            is_header: context.is_header,
            background: None,
            background_image: None,
        })
    }

    /// Resolve a cell value based on its type and style.
    fn resolve_cell_value(
        &self,
        value: &str,
        cell_type: Option<&str>,
        style_index: Option<usize>,
    ) -> Result<String> {
        match cell_type {
            Some("s") => {
                // Shared string index
                if let Ok(idx) = value.parse::<usize>() {
                    self.shared_strings
                        .get(idx)
                        .map(str::to_string)
                        .ok_or_else(|| {
                            Error::InvalidData(format!("shared string index out of range: {idx}"))
                        })
                } else {
                    Ok(value.to_string())
                }
            }
            Some("b") => {
                // Boolean
                if value == "1" {
                    Ok("TRUE".to_string())
                } else {
                    Ok("FALSE".to_string())
                }
            }
            Some("e") => {
                // Error
                Ok(format!("#ERROR:{}", value))
            }
            Some("str") | Some("inlineStr") => {
                // Inline string
                Ok(value.to_string())
            }
            _ => {
                // Number or general - check for date format
                if let Some(style_idx) = style_index {
                    if let Some(num_fmt_id) = self.styles.get_num_fmt_id(style_idx) {
                        if self.styles.is_date_format(num_fmt_id) {
                            // Try to parse as date
                            if let Ok(serial) = value.parse::<f64>() {
                                if let Some(date_str) = Styles::serial_to_date(serial) {
                                    return Ok(date_str);
                                }
                            }
                        }
                    }
                }
                Ok(value.to_string())
            }
        }
    }

    /// Parse `<hyperlinks>` section from worksheet XML and resolve URLs via sheet rels.
    ///
    /// Returns a map of uppercase cell reference (e.g. "A1") to URL string.
    fn parse_hyperlinks(
        xml: &str,
        sheet_rels: &HashMap<String, (String, String)>,
    ) -> HashMap<String, String> {
        let mut hyperlinks = HashMap::new();
        let mut reader = crate::decode::reader_for(xml);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();
        let mut in_hyperlinks = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Start(ref e)) => {
                    match e.name().local_name().as_ref() {
                        "hyperlinks" => {
                            in_hyperlinks = true;
                        }
                        "hyperlink" if in_hyperlinks => {
                            Self::collect_hyperlink_attrs(e, sheet_rels, &mut hyperlinks);
                        }
                        _ => {}
                    }
                }
                Ok(quick_xml::events::Event::Empty(ref e))
                    if in_hyperlinks && e.name().local_name().as_ref() == "hyperlink" =>
                {
                    Self::collect_hyperlink_attrs(e, sheet_rels, &mut hyperlinks);
                }
                Ok(quick_xml::events::Event::End(ref e))
                    if e.name().local_name().as_ref() == "hyperlinks" =>
                {
                    break; // Done with hyperlinks section
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        hyperlinks
    }

    /// Extract cell ref and r:id from a `<hyperlink>` element and insert into map.
    fn collect_hyperlink_attrs(
        e: &quick_xml::events::BytesStart<'_>,
        sheet_rels: &HashMap<String, (String, String)>,
        hyperlinks: &mut HashMap<String, String>,
    ) {
        let mut cell_ref = String::new();
        let mut r_id = String::new();

        for attr in e.attributes().flatten() {
            match attr.key.local_name().as_ref() {
                "ref" => {
                    cell_ref = attr.value.to_uppercase();
                }
                "id" => {
                    r_id = attr.value.to_string();
                }
                _ => {}
            }
        }

        if !cell_ref.is_empty() && !r_id.is_empty() {
            if let Some((rel_type, target)) = sheet_rels.get(&r_id) {
                if rel_type.contains("hyperlink") {
                    hyperlinks.insert(cell_ref, target.clone());
                }
            }
        }
    }

    /// Parse comments XML and return a map of uppercase cell reference to comment text.
    ///
    /// The XML structure is:
    /// ```xml
    /// <comments>
    ///   <commentList>
    ///     <comment ref="A1" authorId="0">
    ///       <text><r><t>Comment text</t></r></text>
    ///     </comment>
    ///   </commentList>
    /// </comments>
    /// ```
    fn parse_comments_xml(xml: &str) -> HashMap<String, String> {
        let mut comments = HashMap::new();
        let mut reader = crate::decode::reader_for(xml);
        // Preserve whitespace inside <t>: quick-xml 0.40+ fragments a single <t>
        // into Text/GeneralRef events, and trimming each fragment would drop the
        // spaces adjacent to an entity ("Bad &bogus; comment" → "Bad&bogus;comment").
        // Inter-element indentation is already excluded by the `in_t` guard.
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();

        let mut in_comment = false;
        let mut in_text = false;
        let mut in_t = false;
        let mut current_ref = String::new();
        let mut current_text = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Start(ref e)) => {
                    let local = e.name().local_name();
                    match local.as_ref() {
                        "comment" => {
                            in_comment = true;
                            current_ref.clear();
                            current_text.clear();
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == "ref" {
                                    current_ref = attr.value.to_uppercase();
                                }
                            }
                        }
                        "text" if in_comment => {
                            in_text = true;
                        }
                        "t" if in_text => {
                            in_t = true;
                            // Separate consecutive <t> runs with a space. This must
                            // happen at the run boundary, not per Text event: under
                            // quick-xml 0.40+ a single <t> is fragmented into
                            // Text/GeneralRef events, and joining those with spaces
                            // would corrupt the text (e.g. "A &amp; B" → "A &  B").
                            if !current_text.is_empty() {
                                current_text.push(' ');
                            }
                        }
                        _ => {}
                    }
                }
                Ok(quick_xml::events::Event::Text(ref e)) if in_t => {
                    current_text.push_str(&crate::decode::decode_text_lossy(e));
                }
                Ok(quick_xml::events::Event::GeneralRef(ref e)) if in_t => {
                    current_text.push_str(&resolve_general_ref(e));
                }
                Ok(quick_xml::events::Event::End(ref e)) => {
                    let local = e.name().local_name();
                    match local.as_ref() {
                        "comment" => {
                            if !current_ref.is_empty() && !current_text.is_empty() {
                                let text = normalize_line_endings(Cow::Borrowed(&current_text))
                                    .into_owned();
                                comments.insert(current_ref.clone(), text);
                            }
                            in_comment = false;
                            in_text = false;
                            in_t = false;
                        }
                        "text" => {
                            in_text = false;
                        }
                        "t" => {
                            in_t = false;
                        }
                        _ => {}
                    }
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        comments
    }

    /// Find the comments file for a sheet via its relationships, read it, and parse.
    ///
    /// A missing comments part is treated as "no comments", but malformed
    /// content surfaces as `Error::Encoding` / other read errors so corruption
    /// is never silently dropped.
    fn find_and_parse_comments(
        container: &OoxmlContainer,
        sheet_rels: &HashMap<String, (String, String)>,
        sheet_dir: &str,
    ) -> Result<HashMap<String, String>> {
        // Look for a relationship whose type contains "comments"
        for (rel_type, target) in sheet_rels.values() {
            if !rel_type.contains("comments") {
                continue;
            }
            let comments_path = Self::resolve_relative_path(sheet_dir, target);
            if let Some(comments_xml) = container.read_xml_optional(&comments_path)? {
                return Ok(Self::parse_comments_xml(&comments_xml));
            }
        }
        Ok(HashMap::new())
    }

    /// Parse drawing images linked to a sheet.
    ///
    /// Follows the OOXML relationship chain:
    /// 1. sheet rels → find drawing relationship → drawing XML path
    /// 2. drawing rels → rId → media filename mapping
    /// 3. drawing XML → xdr:pic > xdr:blipFill > a:blip r:embed → rId
    fn parse_sheet_drawing_images(&self, sheet_path: &str) -> Result<Vec<Block>> {
        // Step 1: Parse sheet-level rels to find drawing target
        let sheet_rels = self.read_optional_relationships_for_part(sheet_path)?;

        // Find drawing relationships (Type contains "drawing")
        let sheet_dir = sheet_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let mut all_images = Vec::new();

        for (rel_type, target) in sheet_rels.values() {
            if !rel_type.contains("drawing") {
                continue;
            }

            // Resolve the drawing path relative to the sheet directory
            let drawing_path = Self::resolve_relative_path(sheet_dir, target);

            // Step 2: Parse drawing rels for rId → media filename mapping
            let drawing_dir = drawing_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            let drawing_rels = self
                .read_optional_relationships_for_part(&drawing_path)?
                .into_iter()
                .filter(|(_, (t, _))| t.contains("image"))
                .map(|(id, (_, tgt))| {
                    let resolved = Self::resolve_relative_path(drawing_dir, &tgt);
                    let filename = resolved.rsplit('/').next().unwrap_or(&resolved).to_string();
                    (id, filename)
                })
                .collect::<HashMap<String, String>>();

            if drawing_rels.is_empty() {
                continue;
            }

            // Step 3: Parse drawing XML for pic elements with a:blip references.
            // Missing drawing XML is OK; malformed bytes must surface.
            if let Some(xml) = self.container.read_xml_optional(&drawing_path)? {
                if let Ok(images) = Self::parse_drawing_images(&xml, &drawing_rels) {
                    all_images.extend(images);
                }
            }
        }

        Ok(all_images)
    }

    /// Build the _rels path for a given part path.
    /// e.g., "xl/worksheets/sheet1.xml" → "xl/worksheets/_rels/sheet1.xml.rels"
    #[cfg(test)]
    fn rels_path_for(part_path: &str) -> String {
        if let Some((dir, file)) = part_path.rsplit_once('/') {
            format!("{}/_rels/{}.rels", dir, file)
        } else {
            format!("_rels/{}.rels", part_path)
        }
    }

    fn read_optional_relationships_for_part(
        &self,
        part_path: &str,
    ) -> Result<HashMap<String, (String, String)>> {
        self.container
            .read_optional_relationships_for_part(part_path)
            .map(|rels| rels.into_type_targets_by_id())
    }

    /// Resolve "Place in Cell" (rich-value) images to a `vm` → media-filename map.
    ///
    /// Excel stores in-cell images as error cells (`t="e" vm="N"` with a
    /// `#VALUE!` placeholder) whose real content lives in a chain of
    /// workbook-level parts:
    ///
    /// 1. `xl/metadata.xml` — `vm` (1-based) → valueMetadata bk → rc[@v] →
    ///    futureMetadata\[XLRICHVALUE\] bk → `xlrd:rvb[@i]` = rich-value index
    /// 2. `xl/richData/rdrichvaluestructure.xml` — structure key order; the
    ///    `_rvRel:LocalImageIdentifier` key position selects the `<v>` slot
    /// 3. `xl/richData/rdrichvalue.xml` — rich value `<rv s="…">` with `<v>`
    ///    values; the selected slot is an index into richValueRel
    /// 4. `xl/richData/richValueRel.xml` + its rels — index → r:id → media path
    ///
    /// Any missing link in the chain yields an empty map (graceful
    /// degradation for foreign producers); malformed XML surfaces as an error.
    fn parse_rich_value_images(&self) -> Result<HashMap<u32, String>> {
        let wb_rels = self.read_optional_relationships_for_part("xl/workbook.xml")?;
        let part_by_type = |suffix: &str| -> Option<String> {
            wb_rels
                .values()
                .find(|(rel_type, _)| rel_type.ends_with(suffix))
                .map(|(_, target)| Self::resolve_relative_path("xl", target))
        };

        let (Some(metadata_path), Some(rv_path), Some(structure_path), Some(rel_path)) = (
            part_by_type("/sheetMetadata"),
            part_by_type("/rdRichValue"),
            part_by_type("/rdRichValueStructure"),
            part_by_type("/richValueRel"),
        ) else {
            return Ok(HashMap::new());
        };
        let (Some(metadata_xml), Some(rv_xml), Some(structure_xml), Some(rel_xml)) = (
            self.container.read_xml_optional(&metadata_path)?,
            self.container.read_xml_optional(&rv_path)?,
            self.container.read_xml_optional(&structure_path)?,
            self.container.read_xml_optional(&rel_path)?,
        ) else {
            return Ok(HashMap::new());
        };

        let vm_to_rv = Self::parse_value_metadata(&metadata_xml)?;
        let image_key_positions = Self::parse_rich_value_structures(&structure_xml)?;
        let rich_values = Self::parse_rich_values(&rv_xml)?;
        let rel_ids = Self::parse_rich_value_rel_ids(&rel_xml)?;
        let rel_targets = self
            .read_optional_relationships_for_part(&rel_path)?
            .into_iter()
            .map(|(id, (_, target))| (id, target))
            .collect::<HashMap<_, _>>();

        let mut result = HashMap::new();
        for (vm, rv_index) in vm_to_rv {
            let Some((structure_index, values)) = rich_values.get(rv_index) else {
                continue;
            };
            let Some(&Some(key_pos)) = image_key_positions.get(*structure_index) else {
                continue;
            };
            let Some(rel_index) = values.get(key_pos).and_then(|v| v.parse::<usize>().ok()) else {
                continue;
            };
            let Some(target) = rel_ids.get(rel_index).and_then(|id| rel_targets.get(id)) else {
                continue;
            };
            let filename = target.rsplit('/').next().unwrap_or(target).to_string();
            result.insert(vm, filename);
        }

        Ok(result)
    }

    /// Parse `xl/metadata.xml` into a `vm` (1-based) → rich-value-index map.
    fn parse_value_metadata(xml: &str) -> Result<HashMap<u32, usize>> {
        let mut reader = crate::decode::reader_for(xml);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();

        let mut metadata_type_names: Vec<String> = Vec::new();
        let mut future_rvb: Vec<usize> = Vec::new(); // futureMetadata bk order → rvb i
        let mut value_rcs: Vec<(u32, usize)> = Vec::new(); // valueMetadata bk order → (rc t, rc v)
        let mut in_future_xlrichvalue = false;
        let mut in_value_metadata = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Start(ref e))
                | Ok(quick_xml::events::Event::Empty(ref e)) => {
                    match e.name().local_name().as_ref() {
                        "metadataType" => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == "name" {
                                    metadata_type_names.push(attr.value.to_string());
                                }
                            }
                        }
                        "futureMetadata" => {
                            in_future_xlrichvalue = e.attributes().flatten().any(|attr| {
                                attr.key.local_name().as_ref() == "name"
                                    && attr.value.as_ref() == "XLRICHVALUE"
                            });
                        }
                        "rvb" if in_future_xlrichvalue => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == "i" {
                                    if let Ok(i) = attr.value.parse::<usize>() {
                                        future_rvb.push(i);
                                    }
                                }
                            }
                        }
                        "valueMetadata" => in_value_metadata = true,
                        "rc" if in_value_metadata => {
                            let mut t = None;
                            let mut v = None;
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    "t" => t = attr.value.parse().ok(),
                                    "v" => v = attr.value.parse().ok(),
                                    _ => {}
                                }
                            }
                            if let (Some(t), Some(v)) = (t, v) {
                                value_rcs.push((t, v));
                            }
                        }
                        _ => {}
                    }
                }
                Ok(quick_xml::events::Event::End(ref e)) => match e.name().local_name().as_ref() {
                    "futureMetadata" => in_future_xlrichvalue = false,
                    "valueMetadata" => in_value_metadata = false,
                    _ => {}
                },
                Ok(quick_xml::events::Event::Eof) => break,
                Err(e) => return Err(Error::xml_parse_with_context(e.to_string(), "metadata")),
                _ => {}
            }
            buf.clear();
        }

        // rc[@t] is a 1-based index into metadataTypes; only XLRICHVALUE
        // records reference rich values.
        let xlrichvalue_type = metadata_type_names
            .iter()
            .position(|n| n == "XLRICHVALUE")
            .map(|p| (p + 1) as u32);

        let mut map = HashMap::new();
        if let Some(rv_type) = xlrichvalue_type {
            for (idx, (t, v)) in value_rcs.iter().enumerate() {
                if *t == rv_type {
                    if let Some(&rv_index) = future_rvb.get(*v) {
                        map.insert((idx + 1) as u32, rv_index);
                    }
                }
            }
        }
        Ok(map)
    }

    /// Parse `rdrichvaluestructure.xml`: per structure (in order), the
    /// position of the `_rvRel:LocalImageIdentifier` key, if any.
    fn parse_rich_value_structures(xml: &str) -> Result<Vec<Option<usize>>> {
        let mut reader = crate::decode::reader_for(xml);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();

        let mut structures: Vec<Option<usize>> = Vec::new();
        let mut current_key_index = 0usize;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Start(ref e))
                | Ok(quick_xml::events::Event::Empty(ref e)) => {
                    match e.name().local_name().as_ref() {
                        "s" => {
                            structures.push(None);
                            current_key_index = 0;
                        }
                        "k" => {
                            let is_image_key = e.attributes().flatten().any(|attr| {
                                attr.key.local_name().as_ref() == "n"
                                    && attr.value.as_ref() == "_rvRel:LocalImageIdentifier"
                            });
                            if is_image_key {
                                if let Some(last) = structures.last_mut() {
                                    *last = Some(current_key_index);
                                }
                            }
                            current_key_index += 1;
                        }
                        _ => {}
                    }
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(e) => {
                    return Err(Error::xml_parse_with_context(
                        e.to_string(),
                        "rdrichvaluestructure",
                    ));
                }
                _ => {}
            }
            buf.clear();
        }

        Ok(structures)
    }

    /// Parse `rdrichvalue.xml`: ordered rich values as (structure index, v slots).
    fn parse_rich_values(xml: &str) -> Result<Vec<(usize, Vec<String>)>> {
        let mut reader = crate::decode::reader_for(xml);
        // Preserve intra-<v> whitespace so entity-adjacent spaces survive
        // quick-xml 0.40+ fragmentation; the `in_v` guard already excludes
        // inter-element indentation.
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();

        let mut values: Vec<(usize, Vec<String>)> = Vec::new();
        let mut in_v = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Start(ref e)) => {
                    match e.name().local_name().as_ref() {
                        "rv" => {
                            let mut s = 0usize;
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == "s" {
                                    s = attr.value.parse().unwrap_or(0);
                                }
                            }
                            values.push((s, Vec::new()));
                        }
                        "v" => {
                            in_v = true;
                            if let Some((_, slots)) = values.last_mut() {
                                slots.push(String::new());
                            }
                        }
                        _ => {}
                    }
                }
                Ok(quick_xml::events::Event::Text(ref e)) if in_v => {
                    if let Some(slot) = values.last_mut().and_then(|(_, s)| s.last_mut()) {
                        slot.push_str(&crate::decode::decode_text_lossy(e));
                    }
                }
                Ok(quick_xml::events::Event::GeneralRef(ref e)) if in_v => {
                    if let Some(slot) = values.last_mut().and_then(|(_, s)| s.last_mut()) {
                        slot.push_str(&resolve_general_ref(e));
                    }
                }
                Ok(quick_xml::events::Event::End(ref e))
                    if e.name().local_name().as_ref() == "v" =>
                {
                    in_v = false;
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(e) => return Err(Error::xml_parse_with_context(e.to_string(), "rdrichvalue")),
                _ => {}
            }
            buf.clear();
        }

        Ok(values)
    }

    /// Parse `richValueRel.xml`: ordered relationship IDs.
    fn parse_rich_value_rel_ids(xml: &str) -> Result<Vec<String>> {
        let mut reader = crate::decode::reader_for(xml);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();

        let mut ids = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Start(ref e))
                | Ok(quick_xml::events::Event::Empty(ref e))
                    if e.name().local_name().as_ref() == "rel" =>
                {
                    for attr in e.attributes().flatten() {
                        if attr.key.local_name().as_ref() == "id" {
                            ids.push(attr.value.to_string());
                        }
                    }
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(e) => return Err(Error::xml_parse_with_context(e.to_string(), "richValueRel")),
                _ => {}
            }
            buf.clear();
        }
        Ok(ids)
    }

    /// Attach a resolved rich-value image to a finished cell.
    ///
    /// The `#VALUE!` placeholder of an error-typed cell is what Excel shows
    /// without rich-value support; the image is the real content, so the
    /// placeholder runs are dropped.
    fn attach_rich_value_image(
        cell: &mut Cell,
        vm: Option<u32>,
        cell_type: Option<&str>,
        rich_value_images: &HashMap<u32, String>,
    ) {
        let Some(filename) = vm.and_then(|v| rich_value_images.get(&v)) else {
            return;
        };
        if cell_type == Some("e") {
            for para in &mut cell.content {
                para.runs.clear();
            }
        }
        let image = InlineImage {
            resource_id: filename.clone(),
            alt_text: None,
            width: None,
            height: None,
        };
        match cell.content.first_mut() {
            Some(para) => para.images.push(image),
            None => cell.content.push(Paragraph {
                images: vec![image],
                ..Default::default()
            }),
        }
    }

    /// Resolve a relative path (e.g., "../drawings/drawing1.xml") against a base directory.
    fn resolve_relative_path(base_dir: &str, relative: &str) -> String {
        if relative.starts_with('/') {
            return relative.trim_start_matches('/').to_string();
        }

        let mut parts: Vec<&str> = if base_dir.is_empty() {
            Vec::new()
        } else {
            base_dir.split('/').collect()
        };

        for segment in relative.split('/') {
            match segment {
                ".." => {
                    parts.pop();
                }
                "." | "" => {}
                s => parts.push(s),
            }
        }

        parts.join("/")
    }

    /// Parse a relationships XML file and return a map of id → (type, target).
    #[cfg(test)]
    fn parse_relationships(xml: &str) -> HashMap<String, (String, String)> {
        crate::container::parse_relationships_xml(xml, "worksheet relationships")
            .expect("worksheet relationships should parse in tests")
            .into_type_targets_by_id()
    }

    /// Parse drawing XML for pictures and picture-filled shapes and create Block::Image
    /// for each.
    ///
    /// A picture-filled shape is `xdr:sp > xdr:spPr > a:blipFill > a:blip[@r:embed]`; its
    /// text stays in the shape and is not read here.
    ///
    /// Structure: xdr:pic > xdr:nvPicPr > xdr:cNvPr[@name]
    ///            xdr:pic > xdr:blipFill > a:blip[@r:embed]
    ///            xdr:pic > xdr:spPr > a:xfrm > a:ext[@cx, @cy]
    fn parse_drawing_images(xml: &str, rels: &HashMap<String, String>) -> Result<Vec<Block>> {
        let mut images = Vec::new();
        let mut reader = crate::decode::reader_for(xml);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();

        // A picture is an `xdr:pic`; a shape filled with a picture is an `xdr:sp` whose
        // `spPr` holds an `a:blipFill`. Both show the image where they sit on the sheet.
        let mut in_pic = false;
        let mut is_shape = false;
        let mut in_nvpicpr = false;
        let mut in_blipfill = false;
        let mut in_sppr = false;
        let mut current_name: Option<String> = None;
        let mut current_rel_id: Option<String> = None;
        let mut current_width: Option<u32> = None;
        let mut current_height: Option<u32> = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Start(ref e)) => {
                    let local_name = e.name().local_name();
                    match local_name.as_ref() {
                        "pic" | "sp" if !in_pic => {
                            in_pic = true;
                            is_shape = local_name.as_ref() == "sp";
                            in_sppr = false;
                            in_blipfill = false;
                            current_name = None;
                            current_rel_id = None;
                            current_width = None;
                            current_height = None;
                        }
                        "nvPicPr" | "nvSpPr" if in_pic => {
                            in_nvpicpr = true;
                        }
                        "cNvPr" if in_nvpicpr => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == "name" {
                                    current_name = Some(attr.value.to_string());
                                }
                            }
                        }
                        "blipFill" if in_pic && in_sppr == is_shape => {
                            in_blipfill = true;
                        }
                        "blip" if in_blipfill => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == "embed" {
                                    current_rel_id = Some(attr.value.to_string());
                                }
                            }
                        }
                        "spPr" if in_pic => {
                            in_sppr = true;
                        }
                        "ext" if in_sppr => {
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    "cx" => {
                                        if let Ok(cx) = attr.value.parse::<u32>() {
                                            current_width = Some(cx);
                                        }
                                    }
                                    "cy" => {
                                        if let Ok(cy) = attr.value.parse::<u32>() {
                                            current_height = Some(cy);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(quick_xml::events::Event::Empty(ref e)) => {
                    let local_name = e.name().local_name();
                    match local_name.as_ref() {
                        "cNvPr" if in_nvpicpr => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == "name" {
                                    current_name = Some(attr.value.to_string());
                                }
                            }
                        }
                        "blip" if in_blipfill => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == "embed" {
                                    current_rel_id = Some(attr.value.to_string());
                                }
                            }
                        }
                        "ext" if in_sppr => {
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    "cx" => {
                                        if let Ok(cx) = attr.value.parse::<u32>() {
                                            current_width = Some(cx);
                                        }
                                    }
                                    "cy" => {
                                        if let Ok(cy) = attr.value.parse::<u32>() {
                                            current_height = Some(cy);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(quick_xml::events::Event::End(ref e)) => {
                    let local_name = e.name().local_name();
                    match local_name.as_ref() {
                        "pic" | "sp" if in_pic && (local_name.as_ref() == "sp") == is_shape => {
                            if let Some(rel_id) = current_rel_id.take() {
                                if let Some(filename) = rels.get(&rel_id) {
                                    images.push(Block::Image {
                                        resource_id: filename.clone(),
                                        alt_text: current_name.take(),
                                        width: current_width.take(),
                                        height: current_height.take(),
                                    });
                                }
                            }
                            in_pic = false;
                        }
                        "nvPicPr" | "nvSpPr" => {
                            in_nvpicpr = false;
                        }
                        "blipFill" => {
                            in_blipfill = false;
                        }
                        "spPr" => {
                            in_sppr = false;
                        }
                        _ => {}
                    }
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(e) => return Err(e.into()),
                _ => {}
            }
            buf.clear();
        }

        Ok(images)
    }

    /// Extract resources (images, media) from the workbook.
    ///
    /// Every part under `xl/media/` is listed, keyed by file name, and annotated with what
    /// the drawings' pictures say about it: a picture's SVG original or HD Photo layer is a
    /// companion of the image the picture shows (see [`crate::drawing`]).
    fn extract_resources(&self, doc: &mut Document) -> Result<()> {
        for file in self.container.list_files() {
            if !file.starts_with("xl/media/") {
                continue;
            }
            if let Ok(data) = self.container.read_binary(&file) {
                let resource = Resource::from_part(&file, data);
                if let Some(name) = resource.filename.clone() {
                    doc.resources.insert(name, resource);
                }
            }
        }

        // Companion file → (role, primary file), kept only while no picture shows it.
        let mut companions: HashMap<String, (ResourceRole, String)> = HashMap::new();
        let mut shown: std::collections::HashSet<String> = std::collections::HashSet::new();
        for drawing in self.container.list_files_with_prefix("xl/drawings/") {
            if !drawing.ends_with(".xml") || drawing.contains("/_rels/") {
                continue;
            }
            let Some(xml) = self.container.read_xml_optional(&drawing)? else {
                continue;
            };
            let pictures = crate::drawing::scan_pictures(&xml);
            let rels = self
                .container
                .read_optional_relationships_for_part(&drawing)?;
            let file_of = |rel_id: &str| -> Option<String> {
                let rel = rels.get(rel_id)?;
                let path = OoxmlContainer::resolve_path(&drawing, &rel.target);
                Some(path.rsplit('/').next().unwrap_or(&path).to_string())
            };
            for rel_id in &pictures.referenced {
                let Some(name) = file_of(rel_id) else {
                    continue;
                };
                match pictures.companions.get(rel_id) {
                    Some((role, primary)) => {
                        if let Some(primary) = file_of(primary) {
                            companions.entry(name).or_insert((*role, primary));
                        }
                    }
                    None => {
                        if let Some(resource) = doc.resources.get_mut(&name) {
                            if resource.alt_text.is_none() {
                                resource.alt_text = pictures.alt_texts.get(rel_id).cloned();
                            }
                        }
                        shown.insert(name);
                    }
                }
            }
        }
        for (name, (role, primary)) in companions {
            if shown.contains(&name) {
                continue;
            }
            if let Some(resource) = doc.resources.get_mut(&name) {
                resource.role = role;
                resource.companion_of = Some(primary);
            }
        }

        Ok(())
    }

    /// Get a reference to the container.
    pub fn container(&self) -> &OoxmlContainer {
        &self.container
    }

    /// Get the number of sheets.
    pub fn sheet_count(&self) -> usize {
        self.sheets.len()
    }

    /// Get sheet names.
    pub fn sheet_names(&self) -> Vec<&str> {
        self.sheets.iter().map(|s| s.name.as_str()).collect()
    }
}

/// The relationship id of a worksheet's background picture (`<picture r:id="…"/>`).
fn sheet_picture_rel(xml: &str) -> Option<String> {
    let mut reader = crate::decode::reader_for(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Empty(e)) | Ok(quick_xml::events::Event::Start(e))
                if e.name().local_name().as_ref() == "picture" =>
            {
                return e
                    .attributes()
                    .flatten()
                    .find(|a| a.key.local_name().as_ref() == "id")
                    .map(|a| crate::decode::attr_value(&a));
            }
            Ok(quick_xml::events::Event::Eof) | Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }
}

#[cfg(test)]
#[path = "../../tests/unit/xlsx/parser.rs"]
mod tests;
