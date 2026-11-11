//! PPTX parser implementation.

use super::bullets::InheritedBullets;
use crate::charts;
use crate::container::OoxmlContainer;
use crate::error::Result;
use crate::model::{
    Block, Cell, Document, HeadingLevel, ListInfo, ListType, Metadata, Paragraph, Resource,
    ResourceRole, Row, Section, Table, TextRun, TextStyle,
};
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

/// Slide info from presentation.xml.
#[derive(Debug, Clone)]
pub(super) struct SlideInfo {
    pub(super) rel_id: String,
}

/// Parser for PPTX (PowerPoint) presentations.
pub struct PptxParser {
    pub(super) container: OoxmlContainer,
    pub(super) slides: Vec<SlideInfo>,
    pub(super) relationships: HashMap<String, String>,
}

impl PptxParser {
    /// Open a PPTX file for parsing.
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
        // Parse presentation relationships
        let relationships = Self::parse_presentation_rels(&container)?;

        // Parse presentation for slide info
        let slides = Self::parse_presentation(&container)?;

        Ok(Self {
            container,
            slides,
            relationships,
        })
    }

    /// Parse presentation relationships.
    fn parse_presentation_rels(container: &OoxmlContainer) -> Result<HashMap<String, String>> {
        Ok(container
            .read_required_relationships_for_part("ppt/presentation.xml")?
            .into_targets_by_id())
    }

    /// Parse presentation.xml for slide info.
    fn parse_presentation(container: &OoxmlContainer) -> Result<Vec<SlideInfo>> {
        let mut slides = Vec::new();
        let xml = container.read_xml("ppt/presentation.xml")?;

        let mut reader = crate::decode::reader_for(&xml);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(plain_libs::xml::events::Event::Empty(e)) | Ok(plain_libs::xml::events::Event::Start(e)) => {
                    let name = e.name();
                    let local_name = name.local_name();
                    if local_name.as_ref() == "sldId" {
                        let mut rel_id = String::new();

                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                key if key.ends_with("id") && key != "id" && key.len() > 2 => {
                                    rel_id = attr.value.to_string();
                                }
                                _ => {}
                            }
                        }

                        if !rel_id.is_empty() {
                            slides.push(SlideInfo { rel_id });
                        }
                    }
                }
                Ok(plain_libs::xml::events::Event::Eof) => break,
                Err(e) => return Err(e.into()),
                _ => {}
            }
            buf.clear();
        }

        Ok(slides)
    }

    /// Parse the presentation and return a Document model.
    pub fn parse(&mut self) -> Result<Document> {
        let mut doc = Document::new();
        doc.format = crate::detect::FormatType::Pptx;

        // Parse metadata
        doc.metadata = self.parse_metadata()?;

        // Extract resources (images, media) and add to document
        let resources = self.extract_resources()?;
        for resource in resources {
            if let Some(ref filename) = resource.filename {
                doc.add_resource(filename.clone(), resource);
            }
        }

        // Parse each slide as a section
        for (idx, slide) in self.slides.clone().iter().enumerate() {
            let section = self.parse_slide_as_section(idx, slide)?;
            doc.add_section(section);
        }

        Ok(doc)
    }

    /// Stream sections (slides) one at a time, calling `f` for each event.
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
        let section_count = self.slides.len();

        // Extract resources once; reuse for both image_map and ResourceExtracted.
        let resources = self.extract_resources()?;
        let image_map: std::collections::HashMap<String, String> = resources
            .iter()
            .filter_map(|r| r.filename.as_ref().map(|name| (name.clone(), name.clone())))
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

        for (idx, slide) in self.slides.clone().iter().enumerate() {
            let section_result = self.parse_slide_as_section(idx, slide);

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
            for resource in resources {
                let name = resource
                    .filename
                    .unwrap_or_else(|| "resource.bin".to_string());
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

    /// Parse a single slide into a Section.
    fn parse_slide_as_section(&self, idx: usize, slide: &SlideInfo) -> Result<Section> {
        let mut section = Section::new(idx);
        section.name = Some(format!("Slide {}", idx + 1));

        if let Some(slide_path) = self.slide_path(slide) {
            let slide_full_rels = self
                .container
                .read_optional_relationships_for_part(&slide_path)?;
            let inherited_phs = self.build_inherited_phs(&slide_path, &slide_full_rels)?;
            let inherited_bullets = self.build_inherited_bullets(&slide_path, &slide_full_rels)?;
            let slide_rels = slide_full_rels.into_targets_by_id();

            if let Some(xml) = self.container.read_xml_optional(&slide_path)? {
                // The slide's own background picture. A background a slide takes from its
                // layout or master is template decoration, like the other layout and
                // master media, and is not referenced.
                section.background_image = crate::drawing::background_picture(&xml)
                    .and_then(|rel_id| slide_rels.get(&rel_id))
                    .map(|target| media_name(target));
                let blocks = self.parse_slide_content_with_rels(
                    &xml,
                    &slide_rels,
                    &slide_path,
                    &inherited_phs,
                    &inherited_bullets,
                )?;
                for block in blocks {
                    section.add_block(block);
                }
            }

            let notes_path = slide_path
                .replace("slides/slide", "notesSlides/notesSlide")
                .replace("slides\\slide", "notesSlides\\notesSlide");
            if let Some(xml) = self.container.read_xml_optional(&notes_path)? {
                let notes_rels = self.parse_slide_relationships(&notes_path)?;
                let notes = self.parse_notes_with_rels(&xml, &notes_rels)?;
                if !notes.is_empty() {
                    section.notes = Some(notes);
                }
            }
        }

        Ok(section)
    }

    /// Parse relationships for a specific slide/notes file.
    fn parse_slide_relationships(&self, slide_path: &str) -> Result<HashMap<String, String>> {
        self.container
            .read_optional_relationships_for_part(slide_path)
            .map(|rels| rels.into_targets_by_id())
    }

    /// Parse metadata from docProps/core.xml.
    fn parse_metadata(&self) -> Result<Metadata> {
        // Use shared metadata parsing from container
        let mut meta = self.container.parse_core_metadata()?;
        // Set slide count
        meta.page_count = Some(self.slides.len() as u32);
        Ok(meta)
    }

    /// Parse slide XML into content blocks with relationship map for hyperlinks, images, and charts.
    fn parse_slide_content_with_rels(
        &self,
        xml: &str,
        rels: &HashMap<String, String>,
        slide_path: &str,
        inherited_phs: &HashMap<String, Vec<Paragraph>>,
        inherited_bullets: &InheritedBullets,
    ) -> Result<Vec<Block>> {
        let mut blocks = Vec::new();

        // Parse text content first (title, headings usually come before tables)
        let paragraphs = self.parse_text_content_excluding_tables_with_rels(
            xml,
            rels,
            inherited_phs,
            inherited_bullets,
        )?;
        for para in paragraphs {
            blocks.push(Block::Paragraph(para));
        }

        // Parse tables after text content
        let tables = self.parse_tables_with_rels(xml, rels)?;
        for table in tables {
            blocks.push(Block::Table(table));
        }

        // Parse charts and convert to tables for RAG-ready output
        let chart_tables = self.parse_charts(rels, slide_path)?;
        for table in chart_tables {
            blocks.push(Block::Table(table));
        }

        // Parse images (p:pic elements)
        let images = self.parse_images(xml, rels)?;
        for image in images {
            blocks.push(image);
        }

        Ok(blocks)
    }

    /// Parse images from slide XML.
    /// Images are in <p:pic> elements with <a:blip r:embed="rIdN"> referencing relationships.
    fn parse_images(&self, xml: &str, rels: &HashMap<String, String>) -> Result<Vec<Block>> {
        let mut images = Vec::new();
        let mut reader = crate::decode::reader_for(xml);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        // A picture is a `p:pic`; a shape filled with a picture is a `p:sp` whose `spPr`
        // holds an `a:blipFill`. Both show the image where they sit on the slide.
        let mut in_pic = false;
        let mut is_shape = false;
        let mut in_nvpicpr = false;
        let mut in_blipfill = false;
        let mut in_sppr = false;
        let mut current_descr: Option<String> = None;
        let mut current_rel_id: Option<String> = None;
        let mut current_width: Option<u32> = None;
        let mut current_height: Option<u32> = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(plain_libs::xml::events::Event::Start(ref e)) => {
                    let local_name = e.name().local_name();
                    match local_name.as_ref() {
                        // p:pic - picture element
                        "pic" | "sp" if !in_pic => {
                            in_pic = true;
                            is_shape = local_name.as_ref() == "sp";
                            in_sppr = false;
                            in_blipfill = false;
                            current_descr = None;
                            current_rel_id = None;
                            current_width = None;
                            current_height = None;
                        }
                        // p:nvPicPr - non-visual picture properties (contains name)
                        "nvPicPr" | "nvSpPr" if in_pic => {
                            in_nvpicpr = true;
                        }
                        // p:cNvPr - common non-visual properties (has name attribute)
                        "cNvPr" if in_nvpicpr => {
                            if let Some(descr) = picture_description(e) {
                                current_descr = Some(descr);
                            }
                        }
                        // p:blipFill - blip fill (contains the image reference)
                        "blipFill" if in_pic && in_sppr == is_shape => {
                            in_blipfill = true;
                        }
                        // a:blip - the actual image reference
                        "blip" if in_blipfill => {
                            for attr in e.attributes().flatten() {
                                // r:embed attribute contains the relationship ID
                                if attr.key.local_name().as_ref() == "embed" {
                                    current_rel_id = Some(attr.value.to_string());
                                }
                            }
                        }
                        // p:spPr - shape properties (contains size)
                        "spPr" if in_pic => {
                            in_sppr = true;
                        }
                        // a:ext - extent (size)
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
                Ok(plain_libs::xml::events::Event::Empty(ref e)) => {
                    let local_name = e.name().local_name();
                    match local_name.as_ref() {
                        // Handle self-closing cNvPr
                        "cNvPr" if in_nvpicpr => {
                            if let Some(descr) = picture_description(e) {
                                current_descr = Some(descr);
                            }
                        }
                        // Handle self-closing blip
                        "blip" if in_blipfill => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == "embed" {
                                    current_rel_id = Some(attr.value.to_string());
                                }
                            }
                        }
                        // Handle self-closing ext
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
                Ok(plain_libs::xml::events::Event::End(ref e)) => {
                    let local_name = e.name().local_name();
                    match local_name.as_ref() {
                        "pic" | "sp" if in_pic && (local_name.as_ref() == "sp") == is_shape => {
                            // Create image block if we have a valid relationship
                            if let Some(rel_id) = current_rel_id.take() {
                                if let Some(target) = rels.get(&rel_id) {
                                    // Extract filename from target path (e.g., "../media/image1.png" -> "image1.png")
                                    let filename =
                                        target.rsplit('/').next().unwrap_or(target).to_string();

                                    images.push(Block::Image {
                                        resource_id: filename,
                                        alt_text: current_descr.take(),
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
                Ok(plain_libs::xml::events::Event::Eof) => break,
                Err(e) => return Err(e.into()),
                _ => {}
            }
            buf.clear();
        }

        Ok(images)
    }

    /// Parse charts referenced in slide relationships and convert to tables for RAG-ready output.
    /// Chart data is extracted from ppt/charts/chartN.xml files.
    fn parse_charts(&self, rels: &HashMap<String, String>, slide_path: &str) -> Result<Vec<Table>> {
        let mut tables = Vec::new();

        // Find chart relationships (target contains "chart")
        for target in rels.values() {
            if !target.contains("chart") {
                continue;
            }

            // Resolve chart path relative to slide
            // Relationship target is like "../charts/chart1.xml"
            let chart_path = if let Some(stripped) = target.strip_prefix("../") {
                // Relative path from slide directory
                if let Some(last_slash) = slide_path.rfind('/') {
                    let slide_dir = &slide_path[..last_slash];
                    if let Some(parent_slash) = slide_dir.rfind('/') {
                        let parent_dir = &slide_dir[..parent_slash];
                        format!("{}/{}", parent_dir, stripped)
                    } else {
                        stripped.to_string()
                    }
                } else {
                    stripped.to_string()
                }
            } else if let Some(stripped) = target.strip_prefix('/') {
                stripped.to_string()
            } else {
                format!("ppt/{}", target)
            };

            // Read and parse chart XML
            let chart_xml = self.container.read_xml(&chart_path)?;
            let chart_data = charts::parse_chart_xml(&chart_xml)?;
            if !chart_data.is_empty() {
                let mut table = chart_data.to_table();
                // Add chart title as caption if available
                if let Some(ref title) = chart_data.title {
                    if !title.is_empty() {
                        // Update first header cell to include chart title
                        if let Some(first_row) = table.rows.first_mut() {
                            if let Some(first_cell) = first_row.cells.first_mut() {
                                let original = first_cell.plain_text();
                                first_cell.content.clear();
                                first_cell.content.push(Paragraph::with_text(format!(
                                    "{} ({})",
                                    original, title
                                )));
                            }
                        }
                    }
                }
                tables.push(table);
            }
        }

        Ok(tables)
    }

    /// Parse notes slide XML into paragraphs.

    #[cfg(test)]
    fn parse_notes(&self, xml: &str) -> Result<Vec<Paragraph>> {
        self.parse_notes_with_rels(xml, &HashMap::new())
    }

    /// Parse notes slide XML into paragraphs with relationship map.
    fn parse_notes_with_rels(
        &self,
        xml: &str,
        rels: &HashMap<String, String>,
    ) -> Result<Vec<Paragraph>> {
        self.parse_text_content_with_rels(xml, rels)
    }

    /// Parse all tables from slide XML with relationship map for hyperlinks.
    fn parse_tables_with_rels(
        &self,
        xml: &str,
        rels: &HashMap<String, String>,
    ) -> Result<Vec<Table>> {
        let mut tables = Vec::new();
        let mut reader = crate::decode::reader_for(xml);
        // Don't trim text - preserve whitespace from xml:space="preserve" elements
        reader.config_mut().trim_text(false);

        let mut buf = Vec::new();
        let mut in_table = false;
        let mut in_row = false;
        let mut in_cell = false;
        let mut in_txbody = false;
        let mut in_paragraph = false;
        let mut in_tc_pr = false;
        let mut in_ppr = false;
        let mut in_bu_blip = false;
        let mut current_level: u8 = 0;
        let mut current_bullet: Option<String> = None;

        let mut current_table = Table::new();
        let mut current_row = Row::new();
        let mut current_cell = Cell::new();
        // The current `a:tc` is a position another cell's merge covers — it has no cell of
        // its own in the model (see `Table::cell_columns`).
        let mut cell_covered = false;
        // The current row holds a position a vertical merge from above covers. Such a row
        // stays even when it reads as empty: dropping it would leave the merge's
        // `row_span` counting a row that is not there.
        let mut row_under_merge = false;
        let mut current_paragraphs: Vec<Paragraph> = Vec::new();
        let mut runs = RunReader::default();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(plain_libs::xml::events::Event::Start(ref e))
                    if in_paragraph && runs.element(e, false, rels) => {}
                Ok(plain_libs::xml::events::Event::Start(ref e)) => {
                    let local_name = e.name().local_name();
                    match local_name.as_ref() {
                        // a:tbl - table
                        "tbl" => {
                            in_table = true;
                            current_table = Table::new();
                        }
                        // a:tr - table row
                        "tr" if in_table => {
                            in_row = true;
                            row_under_merge = false;
                            current_row = Row::new();
                        }
                        // a:tc - table cell
                        "tc" if in_row => {
                            in_cell = true;
                            current_cell = Cell::new();
                            cell_covered = read_table_cell_merge(e, &mut current_cell);
                            row_under_merge |= cell_covered && is_vertically_covered(e);
                            current_paragraphs.clear();
                        }
                        // a:tcPr - cell properties; its fill may be a picture
                        "tcPr" if in_cell => {
                            in_tc_pr = true;
                        }
                        "blip" if in_tc_pr => {
                            if let Some(file) = blip_file(e, rels) {
                                current_cell.background_image = Some(file);
                            }
                        }
                        // a:txBody - text body in cell
                        "txBody" if in_cell => {
                            in_txbody = true;
                        }
                        // a:pPr / a:buBlip - the paragraph's own picture bullet
                        "pPr" if in_paragraph && !runs.in_run() => {
                            in_ppr = true;
                            current_level = paragraph_level(e);
                        }
                        "buBlip" if in_ppr => {
                            in_bu_blip = true;
                        }
                        "blip" if in_bu_blip => {
                            if let Some(file) = blip_file(e, rels) {
                                current_bullet = Some(file);
                            }
                        }
                        // a:p - paragraph
                        "p" if in_txbody => {
                            in_paragraph = true;
                            current_bullet = None;
                            current_level = 0;
                            in_ppr = false;
                            runs.start_paragraph();
                        }
                        _ => {}
                    }
                }
                Ok(plain_libs::xml::events::Event::Empty(ref e))
                    if in_paragraph && runs.element(e, true, rels) => {}
                Ok(plain_libs::xml::events::Event::Empty(ref e)) => {
                    let local_name = e.name().local_name();
                    match local_name.as_ref() {
                        // A self-closing a:tc: an empty cell, or a covered position.
                        "tc" if in_row => {
                            let mut cell = Cell::new();
                            let covered = read_table_cell_merge(e, &mut cell);
                            row_under_merge |= covered && is_vertically_covered(e);
                            if !covered {
                                current_row.add_cell(cell);
                            }
                        }
                        "blip" if in_tc_pr => {
                            if let Some(file) = blip_file(e, rels) {
                                current_cell.background_image = Some(file);
                            }
                        }
                        "pPr" if in_paragraph && !runs.in_run() => {
                            current_level = paragraph_level(e);
                        }
                        "blip" if in_bu_blip => {
                            if let Some(file) = blip_file(e, rels) {
                                current_bullet = Some(file);
                            }
                        }
                        _ => {}
                    }
                }
                Ok(plain_libs::xml::events::Event::Text(ref e)) if runs.in_text() => {
                    runs.push_text(&crate::decode::decode_text_lossy(e));
                }
                Ok(plain_libs::xml::events::Event::GeneralRef(ref e)) if runs.in_text() => {
                    runs.push_text(&crate::decode::resolve_general_ref(e));
                }
                Ok(plain_libs::xml::events::Event::End(ref e)) if in_paragraph && runs.end(e) => {}
                Ok(plain_libs::xml::events::Event::End(ref e)) => {
                    let local_name = e.name().local_name();
                    match local_name.as_ref() {
                        "p" if in_txbody => {
                            let runs = runs.finish();
                            if !runs.is_empty() {
                                let list_info = current_bullet.take().map(|image| ListInfo {
                                    list_type: ListType::Bullet,
                                    level: current_level,
                                    number: None,
                                    marker_image: Some(image),
                                });
                                current_paragraphs.push(Paragraph {
                                    runs,
                                    list_info,
                                    ..Default::default()
                                });
                            }
                            in_paragraph = false;
                            in_ppr = false;
                        }
                        "pPr" => {
                            in_ppr = false;
                        }
                        "buBlip" => {
                            in_bu_blip = false;
                        }
                        "tcPr" => {
                            in_tc_pr = false;
                        }
                        "txBody" => {
                            in_txbody = false;
                        }
                        "tc" => {
                            if !cell_covered {
                                current_cell.content = current_paragraphs.clone();
                                current_row.add_cell(current_cell.clone());
                            }
                            in_cell = false;
                        }
                        "tr" => {
                            if !current_row.is_empty() || row_under_merge {
                                // Mark first row as header
                                if current_table.is_empty() {
                                    current_row.is_header = true;
                                }
                                current_table.add_row(current_row.clone());
                            }
                            in_row = false;
                        }
                        "tbl" => {
                            if !current_table.is_empty() {
                                tables.push(current_table.clone());
                            }
                            in_table = false;
                        }
                        _ => {}
                    }
                }
                Ok(plain_libs::xml::events::Event::Eof) => break,
                Err(e) => return Err(e.into()),
                _ => {}
            }
            buf.clear();
        }

        Ok(tables)
    }

    /// Parse text content excluding tables with relationship map for hyperlinks.
    /// `inherited_phs` maps placeholder key → fallback paragraphs from layout/master.
    fn parse_text_content_excluding_tables_with_rels(
        &self,
        xml: &str,
        rels: &HashMap<String, String>,
        inherited_phs: &HashMap<String, Vec<Paragraph>>,
        inherited_bullets: &InheritedBullets,
    ) -> Result<Vec<Paragraph>> {
        let mut paragraphs = Vec::new();
        let mut reader = crate::decode::reader_for(xml);
        // Don't trim text - preserve whitespace from xml:space="preserve" elements
        reader.config_mut().trim_text(false);

        let mut buf = Vec::new();
        let mut in_table = false;
        let mut table_depth = 0;
        let mut in_shape = false;
        let mut in_txbody = false;
        let mut in_paragraph = false;
        let mut runs = RunReader::default();
        let mut current_heading: HeadingLevel = HeadingLevel::None;
        // Placeholder inheritance tracking
        let mut current_ph_key: Option<String> = None;
        let mut shape_para_start: usize = 0; // paragraphs.len() when current shape started
                                             // Picture bullets (`a:buBlip`): the image of the paragraph's own bullet, and the
                                             // ones the shape's list style sets per level (`a:lstStyle/a:lvlNpPr`).
        let mut in_ppr = false;
        let mut in_bu_blip = false;
        let mut in_lst_style = false;
        let mut lst_style_level: Option<u8> = None;
        let mut shape_bullets: HashMap<u8, Option<String>> = HashMap::new();
        let mut current_ph_type: Option<String> = None;
        let mut current_ph_idx: Option<String> = None;
        let mut current_level: u8 = 0;
        let mut current_bullet: Option<String> = None;
        let mut bullet_overridden = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(plain_libs::xml::events::Event::Start(ref e))
                    if in_paragraph && !in_table && runs.element(e, false, rels) => {}
                Ok(plain_libs::xml::events::Event::Start(ref e)) => {
                    let local_name = e.name().local_name();
                    match local_name.as_ref() {
                        // Track table depth to skip table content
                        "tbl" => {
                            in_table = true;
                            table_depth += 1;
                        }
                        // p:sp - shape (also matches inner shapes inside p:grpSp groups,
                        // because quick_xml's flat event stream processes nested elements
                        // the same as top-level ones by local name)
                        "sp" if !in_table => {
                            in_shape = true;
                            current_heading = HeadingLevel::None;
                            current_ph_key = None;
                            current_ph_type = None;
                            current_ph_idx = None;
                            shape_para_start = paragraphs.len();
                            shape_bullets.clear();
                        }
                        "lstStyle" if in_shape && !in_table => {
                            in_lst_style = true;
                        }
                        name if in_lst_style && list_style_level(name).is_some() => {
                            lst_style_level = list_style_level(name);
                        }
                        // a:pPr - paragraph properties (level, bullet)
                        "pPr" if in_paragraph && !in_table && !runs.in_run() => {
                            in_ppr = true;
                            current_level = paragraph_level(e);
                        }
                        "buBlip" if in_ppr || lst_style_level.is_some() => {
                            in_bu_blip = true;
                        }
                        "buNone" | "buChar" | "buAutoNum" if in_ppr => {
                            bullet_overridden = true;
                        }
                        "buNone" | "buChar" | "buAutoNum" if lst_style_level.is_some() => {
                            if let Some(level) = lst_style_level {
                                shape_bullets.insert(level, None);
                            }
                        }
                        "blip" if in_bu_blip => {
                            record_bullet(
                                e,
                                rels,
                                in_ppr,
                                lst_style_level,
                                &mut current_bullet,
                                &mut shape_bullets,
                            );
                        }
                        // p:txBody - text body in shape
                        "txBody" if in_shape && !in_table => {
                            in_txbody = true;
                        }
                        // a:p - paragraph (only if not in table, but in shape's txBody)
                        "p" if !in_table && in_txbody => {
                            in_paragraph = true;
                            runs.start_paragraph();
                            in_ppr = false;
                            current_level = 0;
                            current_bullet = None;
                            bullet_overridden = false;
                        }
                        // p:ph - placeholder type (for heading detection and inheritance key)
                        "ph" if in_shape && !in_table => {
                            let placeholder = Placeholder::of(e);
                            current_heading = placeholder.heading();
                            current_ph_key = Some(placeholder.key());
                            current_ph_type = placeholder.kind;
                            current_ph_idx = placeholder.idx;
                        }
                        _ => {}
                    }
                }
                Ok(plain_libs::xml::events::Event::Empty(ref e))
                    if in_paragraph && !in_table && runs.element(e, true, rels) => {}
                Ok(plain_libs::xml::events::Event::Empty(ref e)) => {
                    let local_name = e.name().local_name();
                    match local_name.as_ref() {
                        "pPr" if in_paragraph && !in_table && !runs.in_run() => {
                            current_level = paragraph_level(e);
                        }
                        "buNone" | "buChar" | "buAutoNum" if in_ppr => {
                            bullet_overridden = true;
                        }
                        "buNone" | "buChar" | "buAutoNum" if lst_style_level.is_some() => {
                            if let Some(level) = lst_style_level {
                                shape_bullets.insert(level, None);
                            }
                        }
                        "blip" if in_bu_blip => {
                            record_bullet(
                                e,
                                rels,
                                in_ppr,
                                lst_style_level,
                                &mut current_bullet,
                                &mut shape_bullets,
                            );
                        }
                        // p:ph - placeholder type (self-closing)
                        "ph" if in_shape && !in_table => {
                            let placeholder = Placeholder::of(e);
                            current_heading = placeholder.heading();
                            current_ph_key = Some(placeholder.key());
                            current_ph_type = placeholder.kind;
                            current_ph_idx = placeholder.idx;
                        }
                        _ => {}
                    }
                }
                Ok(plain_libs::xml::events::Event::Text(ref e)) if runs.in_text() && !in_table => {
                    runs.push_text(&crate::decode::decode_text_lossy(e));
                }
                Ok(plain_libs::xml::events::Event::GeneralRef(ref e)) if runs.in_text() && !in_table => {
                    runs.push_text(&crate::decode::resolve_general_ref(e));
                }
                Ok(plain_libs::xml::events::Event::End(ref e))
                    if in_paragraph && !in_table && runs.end(e) => {}
                Ok(plain_libs::xml::events::Event::End(ref e)) => {
                    let local_name = e.name().local_name();
                    match local_name.as_ref() {
                        "tbl" => {
                            table_depth -= 1;
                            if table_depth == 0 {
                                in_table = false;
                            }
                        }
                        "pPr" => {
                            in_ppr = false;
                        }
                        "buBlip" => {
                            in_bu_blip = false;
                        }
                        "lstStyle" => {
                            in_lst_style = false;
                            lst_style_level = None;
                        }
                        name if in_lst_style && list_style_level(name).is_some() => {
                            lst_style_level = None;
                        }
                        "p" if !in_table => {
                            let runs = runs.finish();
                            if !runs.is_empty() {
                                // The paragraph's own bullet, else the shape's list style,
                                // else what the layout and master give the placeholder.
                                let marker_image = current_bullet.clone().or_else(|| {
                                    if bullet_overridden {
                                        return None;
                                    }
                                    match shape_bullets.get(&current_level) {
                                        Some(own) => own.clone(),
                                        None => inherited_bullets
                                            .levels(
                                                current_ph_type.as_deref(),
                                                current_ph_idx.as_deref(),
                                            )
                                            .and_then(|levels| levels.get(&current_level))
                                            .cloned()
                                            .flatten()
                                            .map(|path| media_name(&path)),
                                    }
                                });
                                let list_info = marker_image.map(|image| ListInfo {
                                    list_type: ListType::Bullet,
                                    level: current_level,
                                    number: None,
                                    marker_image: Some(image),
                                });
                                paragraphs.push(Paragraph {
                                    runs,
                                    heading: current_heading,
                                    list_info,
                                    ..Default::default()
                                });
                            }
                            in_paragraph = false;
                            in_ppr = false;
                        }
                        "txBody" if !in_table => {
                            in_txbody = false;
                        }
                        "sp" if !in_table => {
                            // If this shape had a placeholder but produced no text,
                            // inherit from layout/master
                            if paragraphs.len() == shape_para_start {
                                if let Some(ref ph_key) = current_ph_key {
                                    if let Some(fallback) = inherited_phs.get(ph_key) {
                                        paragraphs.extend_from_slice(fallback);
                                    }
                                }
                            }
                            in_shape = false;
                            current_heading = HeadingLevel::None;
                            current_ph_key = None;
                        }
                        _ => {}
                    }
                }
                Ok(plain_libs::xml::events::Event::Eof) => break,
                Err(e) => return Err(e.into()),
                _ => {}
            }
            buf.clear();
        }

        Ok(paragraphs)
    }

    /// Parse text content from slide or notes XML.
    /// Text is found in: p:sp/p:txBody/a:p/a:r/a:t

    #[cfg(test)]
    fn parse_text_content(&self, xml: &str) -> Result<Vec<Paragraph>> {
        self.parse_text_content_with_rels(xml, &HashMap::new())
    }

    /// Parse text content from slide or notes XML with relationship map for hyperlinks.
    fn parse_text_content_with_rels(
        &self,
        xml: &str,
        rels: &HashMap<String, String>,
    ) -> Result<Vec<Paragraph>> {
        let mut paragraphs = Vec::new();
        let mut reader = crate::decode::reader_for(xml);
        // Don't trim text - preserve whitespace from xml:space="preserve" elements
        reader.config_mut().trim_text(false);

        let mut buf = Vec::new();
        let mut in_paragraph = false;
        let mut runs = RunReader::default();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(plain_libs::xml::events::Event::Start(ref e)) => {
                    if e.name().local_name().as_ref() == "p" {
                        in_paragraph = true;
                        runs.start_paragraph();
                    } else if in_paragraph {
                        runs.element(e, false, rels);
                    }
                }
                Ok(plain_libs::xml::events::Event::Empty(ref e)) if in_paragraph => {
                    runs.element(e, true, rels);
                }
                Ok(plain_libs::xml::events::Event::Text(ref e)) if runs.in_text() => {
                    runs.push_text(&crate::decode::decode_text_lossy(e));
                }
                Ok(plain_libs::xml::events::Event::GeneralRef(ref e)) if runs.in_text() => {
                    runs.push_text(&crate::decode::resolve_general_ref(e));
                }
                Ok(plain_libs::xml::events::Event::End(ref e)) => {
                    if e.name().local_name().as_ref() == "p" {
                        let runs = runs.finish();
                        if !runs.is_empty() {
                            paragraphs.push(Paragraph {
                                runs,
                                ..Default::default()
                            });
                        }
                        in_paragraph = false;
                    } else {
                        runs.end(e);
                    }
                }
                Ok(plain_libs::xml::events::Event::Eof) => break,
                Err(e) => return Err(e.into()),
                _ => {}
            }
            buf.clear();
        }

        Ok(paragraphs)
    }

    /// Build a map of placeholder key → fallback paragraphs from layout and master XMLs.
    /// Layout takes precedence over master; slide-defined text takes precedence over both.
    ///
    /// The layout, its relationships and the master are optional: an absent part is
    /// skipped. One that is present and unreadable is an error, as for every other
    /// optional part.
    fn build_inherited_phs(
        &self,
        slide_path: &str,
        slide_full_rels: &crate::container::Relationships,
    ) -> Result<HashMap<String, Vec<Paragraph>>> {
        const SLIDE_LAYOUT_TYPE: &str =
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
        const SLIDE_MASTER_TYPE: &str =
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";

        let mut inherited: HashMap<String, Vec<Paragraph>> = HashMap::new();

        // Try to find the layout from slide relationships
        let layout_path = slide_full_rels
            .get_by_type(SLIDE_LAYOUT_TYPE)
            .first()
            .map(|rel| OoxmlContainer::resolve_path(slide_path, &rel.target));

        let Some(layout_path) = layout_path else {
            return Ok(inherited);
        };

        // Parse master first (lower priority), then overlay layout
        let layout_full_rels = self
            .container
            .read_optional_relationships_for_part(&layout_path)?;

        if let Some(master_rel) = layout_full_rels.get_by_type(SLIDE_MASTER_TYPE).first() {
            let master_path = OoxmlContainer::resolve_path(&layout_path, &master_rel.target);
            if let Some(master_xml) = self.container.read_xml_optional(&master_path)? {
                inherited = parse_placeholder_texts_from_xml(&master_xml)?;
            }
        }

        if let Some(layout_xml) = self.container.read_xml_optional(&layout_path)? {
            // Layout overrides master
            for (key, paras) in parse_placeholder_texts_from_xml(&layout_xml)? {
                inherited.insert(key, paras);
            }
        }

        // Remove presentational placeholders — these are dynamic at display time
        // and would inject noise (footer text, slide numbers, dates) into every slide.
        const PRESENTATIONAL: &[&str] = &["dt", "sldNum", "ftr", "hdr", "sldImg"];
        inherited.retain(|k, _| !PRESENTATIONAL.contains(&k.as_str()));

        Ok(inherited)
    }

    /// The picture bullets the slide's placeholders inherit from its layout and master.
    fn build_inherited_bullets(
        &self,
        slide_path: &str,
        slide_full_rels: &crate::container::Relationships,
    ) -> Result<InheritedBullets> {
        const SLIDE_LAYOUT_TYPE: &str =
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
        const SLIDE_MASTER_TYPE: &str =
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";

        let Some(layout_rel) = slide_full_rels
            .get_by_type(SLIDE_LAYOUT_TYPE)
            .first()
            .copied()
        else {
            return Ok(InheritedBullets::default());
        };
        let layout_path = OoxmlContainer::resolve_path(slide_path, &layout_rel.target);
        let layout_rels = self
            .container
            .read_optional_relationships_for_part(&layout_path)?;
        let master_path = layout_rels
            .get_by_type(SLIDE_MASTER_TYPE)
            .first()
            .map(|rel| OoxmlContainer::resolve_path(&layout_path, &rel.target));

        let layout_xml = self.container.read_xml_optional(&layout_path)?;
        let layout_targets = layout_rels.into_targets_by_id();
        let (master_xml, master_targets) = match &master_path {
            Some(path) => (
                self.container.read_xml_optional(path)?,
                self.container
                    .read_optional_relationships_for_part(path)?
                    .into_targets_by_id(),
            ),
            None => (None, HashMap::new()),
        };

        Ok(InheritedBullets::new(
            Some((&layout_path, &layout_targets)),
            layout_xml.as_deref(),
            master_path.as_deref().map(|p| (p, &master_targets)),
            master_xml.as_deref(),
        ))
    }

    /// The media a slide draws through bullets it inherits from its layout or master:
    /// package path of each picture bullet that some paragraph of the slide ends up with.
    fn inherited_bullet_media(&self, slide_path: &str) -> Result<Vec<String>> {
        let slide_full_rels = self
            .container
            .read_optional_relationships_for_part(slide_path)?;
        let bullets = self.build_inherited_bullets(slide_path, &slide_full_rels)?;
        let Some(xml) = self.container.read_xml_optional(slide_path)? else {
            return Ok(Vec::new());
        };
        let rels = slide_full_rels.into_targets_by_id();
        let paragraphs = self.parse_text_content_excluding_tables_with_rels(
            &xml,
            &rels,
            &HashMap::new(),
            &bullets,
        )?;
        let mut paths: Vec<String> = Vec::new();
        for file in paragraphs
            .iter()
            .filter_map(|p| p.list_info.as_ref()?.marker_image.as_deref())
        {
            if let Some(path) = bullets.path_of(file) {
                if !paths.iter().any(|p| p == path) {
                    paths.push(path.to_string());
                }
            }
        }
        Ok(paths)
    }

    /// Extract resources (images, media) from the presentation.
    ///
    /// Lists the media the slides reference, keyed by file name, the way a Word document
    /// lists its image relationships: media used only by a slide layout or master, or by
    /// nothing at all, is not part of what the slides show. A picture's SVG original or
    /// HD Photo layer is listed as a companion of the image the picture shows (see
    /// [`crate::drawing`]); a file that some slide shows as a picture stays a primary.
    pub fn extract_resources(&self) -> Result<Vec<Resource>> {
        let mut resources: Vec<Resource> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        // Companion file → (role, primary file), kept only while no slide shows it.
        let mut companions: HashMap<String, (ResourceRole, String)> = HashMap::new();
        let mut shown: std::collections::HashSet<String> = std::collections::HashSet::new();

        for slide in &self.slides {
            let Some(slide_path) = self.slide_path(slide) else {
                continue;
            };
            let rels = self
                .container
                .read_optional_relationships_for_part(&slide_path)?;
            let pictures = self
                .container
                .read_xml_optional(&slide_path)?
                .map(|xml| crate::drawing::scan_pictures(&xml))
                .unwrap_or_default();

            let file_of = |rel_id: &str| -> Option<(String, String)> {
                let rel = rels.get(rel_id)?;
                let path = OoxmlContainer::resolve_path(&slide_path, &rel.target);
                let name = path.rsplit('/').next().unwrap_or(&path).to_string();
                Some((path, name))
            };

            let mut ids: Vec<&String> = rels.by_id.keys().collect();
            ids.sort();
            for rel_id in ids {
                let rel = &rels.by_id[rel_id];
                let is_media = ["/image", "/hdphoto", "/video", "/audio", "/media"]
                    .iter()
                    .any(|kind| rel.rel_type.ends_with(kind));
                if !is_media || rel.external {
                    continue;
                }
                let Some((path, name)) = file_of(rel_id) else {
                    continue;
                };

                match pictures.companions.get(rel_id.as_str()) {
                    Some((role, primary)) => {
                        if let Some((_, primary_name)) = file_of(primary) {
                            companions
                                .entry(name.clone())
                                .or_insert((*role, primary_name));
                        }
                    }
                    None => {
                        shown.insert(name.clone());
                    }
                }

                let at = match index.get(&name) {
                    Some(&at) => at,
                    None => {
                        let Ok(data) = self.container.read_binary(&path) else {
                            continue;
                        };
                        index.insert(name.clone(), resources.len());
                        resources.push(Resource::from_part(&path, data));
                        resources.len() - 1
                    }
                };
                let resource = &mut resources[at];
                if resource.alt_text.is_none() {
                    resource.alt_text = pictures.alt_texts.get(rel_id.as_str()).cloned();
                }
            }

            // A picture bullet the slide takes from its layout or master is drawn by the
            // slide's paragraphs, so its image is listed with the slide's media.
            for path in self.inherited_bullet_media(&slide_path)? {
                let name = media_name(&path);
                shown.insert(name.clone());
                if let std::collections::hash_map::Entry::Vacant(slot) = index.entry(name) {
                    let Ok(data) = self.container.read_binary(&path) else {
                        continue;
                    };
                    slot.insert(resources.len());
                    resources.push(Resource::from_part(&path, data));
                }
            }
        }

        for resource in &mut resources {
            let name = resource.filename.as_deref().unwrap_or_default();
            if shown.contains(name) {
                continue;
            }
            if let Some((role, primary)) = companions.get(name) {
                resource.role = *role;
                resource.companion_of = Some(primary.clone());
                resource.alt_text = None;
            }
        }

        Ok(resources)
    }

    /// The package path of a slide.
    fn slide_path(&self, slide: &SlideInfo) -> Option<String> {
        let target = self.relationships.get(&slide.rel_id)?;
        Some(match target.strip_prefix('/') {
            Some(stripped) => stripped.to_string(),
            None => format!("ppt/{target}"),
        })
    }

    /// Get a reference to the container.
    pub fn container(&self) -> &OoxmlContainer {
        &self.container
    }

    /// Get the number of slides.
    pub fn slide_count(&self) -> usize {
        self.slides.len()
    }
}

/// Parse placeholder texts from a layout or master XML.
/// Returns ph_key → Vec<Paragraph> for non-empty placeholder shapes.
/// ph_key = ph type (e.g. "title") if set, else "idx:<N>".
fn parse_placeholder_texts_from_xml(xml: &str) -> Result<HashMap<String, Vec<Paragraph>>> {
    let mut result: HashMap<String, Vec<Paragraph>> = HashMap::new();

    let mut reader = crate::decode::reader_for(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    // A layout's and master's text links nowhere a slide's reader resolves: their links
    // are their own parts' relationships.
    let no_links = HashMap::new();
    let mut in_shape = false;
    let mut in_table = false;
    let mut table_depth = 0u32;
    let mut in_txbody = false;
    let mut in_paragraph = false;
    let mut runs = RunReader::default();
    let mut current_ph_key: Option<String> = None;
    let mut current_heading = HeadingLevel::None;
    let mut shape_paragraphs: Vec<Paragraph> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(plain_libs::xml::events::Event::Start(ref e))
                if in_paragraph && !in_table && runs.element(e, false, &no_links) => {}
            Ok(plain_libs::xml::events::Event::Start(ref e)) => match e.name().local_name().as_ref() {
                "tbl" => {
                    in_table = true;
                    table_depth += 1;
                }
                "sp" if !in_table => {
                    in_shape = true;
                    current_ph_key = None;
                    current_heading = HeadingLevel::None;
                    shape_paragraphs.clear();
                }
                "txBody" if in_shape && !in_table => {
                    in_txbody = true;
                }
                "p" if in_txbody && !in_table => {
                    in_paragraph = true;
                    runs.start_paragraph();
                }
                "ph" if in_shape && !in_table => {
                    let placeholder = Placeholder::of(e);
                    current_heading = placeholder.heading();
                    current_ph_key = Some(placeholder.key());
                }
                _ => {}
            },
            Ok(plain_libs::xml::events::Event::Empty(ref e))
                if in_paragraph && !in_table && runs.element(e, true, &no_links) => {}
            Ok(plain_libs::xml::events::Event::Empty(ref e)) => {
                if e.name().local_name().as_ref() == "ph" && in_shape && !in_table {
                    let placeholder = Placeholder::of(e);
                    current_heading = placeholder.heading();
                    current_ph_key = Some(placeholder.key());
                }
            }
            Ok(plain_libs::xml::events::Event::Text(ref e)) if runs.in_text() && !in_table => {
                runs.push_text(&crate::decode::decode_text_lossy(e));
            }
            Ok(plain_libs::xml::events::Event::GeneralRef(ref e)) if runs.in_text() && !in_table => {
                runs.push_text(&crate::decode::resolve_general_ref(e));
            }
            Ok(plain_libs::xml::events::Event::End(ref e))
                if in_paragraph && !in_table && runs.end(e) => {}
            Ok(plain_libs::xml::events::Event::End(ref e)) => match e.name().local_name().as_ref() {
                "tbl" => {
                    table_depth -= 1;
                    if table_depth == 0 {
                        in_table = false;
                    }
                }
                "p" if !in_table => {
                    let runs = runs.finish();
                    if !runs.is_empty() {
                        shape_paragraphs.push(Paragraph {
                            runs,
                            heading: current_heading,
                            ..Default::default()
                        });
                    }
                    in_paragraph = false;
                }
                "txBody" if !in_table => {
                    in_txbody = false;
                }
                "sp" if !in_table => {
                    // Only store non-empty shapes with a placeholder key
                    if let Some(ph_key) = current_ph_key.take() {
                        if !shape_paragraphs.is_empty() {
                            result.insert(ph_key, shape_paragraphs.clone());
                        }
                    }
                    in_shape = false;
                    current_heading = HeadingLevel::None;
                }
                _ => {}
            },
            Ok(plain_libs::xml::events::Event::Eof) => break,
            Err(e) => return Err(e.into()),
            _ => {}
        }
        buf.clear();
    }

    Ok(result)
}

/// A picture's description (`p:cNvPr descr`) — its alt text — if it has a non-blank one.
/// The shape's `name` ("Picture 3") is an editing label, not a description.
/// The list level a paragraph-properties element (`a:pPr`) sets (`lvl`, 0-based).
fn paragraph_level(e: &plain_libs::xml::events::BytesStart<'_>) -> u8 {
    e.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == "lvl")
        .and_then(|a| a.value.parse::<u8>().ok())
        .unwrap_or(0)
}

/// The 0-based level of a list-style level element (`a:lvl1pPr` … `a:lvl9pPr`).
fn list_style_level(name: &str) -> Option<u8> {
    let digit = name.strip_prefix("lvl")?.strip_suffix("pPr")?;
    digit
        .parse::<u8>()
        .ok()
        .filter(|n| (1..=9).contains(n))
        .map(|n| n - 1)
}

/// Record the picture of an `a:buBlip`'s `a:blip`: the file name of the image it embeds,
/// on the paragraph when the bullet is the paragraph's own, else on the shape's list style
/// level it sits in.
fn record_bullet(
    blip: &plain_libs::xml::events::BytesStart<'_>,
    rels: &HashMap<String, String>,
    in_paragraph_properties: bool,
    list_style_level: Option<u8>,
    paragraph_bullet: &mut Option<String>,
    shape_bullets: &mut HashMap<u8, Option<String>>,
) {
    let Some(target) = blip
        .attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == "embed")
        .and_then(|a| rels.get(a.value.as_ref()))
    else {
        return;
    };
    let file = media_name(target);
    if in_paragraph_properties {
        *paragraph_bullet = Some(file);
    } else if let Some(level) = list_style_level {
        shape_bullets.insert(level, Some(file));
    }
}

/// The media file an `a:blip`'s `r:embed` names, through the part's relationships.
fn blip_file(
    blip: &plain_libs::xml::events::BytesStart<'_>,
    rels: &HashMap<String, String>,
) -> Option<String> {
    blip.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == "embed")
        .and_then(|a| rels.get(a.value.as_ref()))
        .map(|target| media_name(target))
}

/// The file name of a media part: the last segment of a relationship target.
fn media_name(target: &str) -> String {
    target.rsplit('/').next().unwrap_or(target).to_string()
}

fn picture_description(e: &plain_libs::xml::events::BytesStart<'_>) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == "descr")
        .map(|a| crate::decode::attr_value(&a))
        .filter(|d| !d.trim().is_empty())
}

/// Read a DrawingML table cell's merge attributes into `cell`, returning whether the
/// cell is a covered position.
///
/// DrawingML writes every grid position as an `a:tc`: the owner of a merge carries
/// `gridSpan`/`rowSpan`, and the positions it covers are present with `hMerge="1"` or
/// `vMerge="1"`. The model records the merge once, on the owner, and gives covered
/// positions no cell.
fn read_table_cell_merge(e: &plain_libs::xml::events::BytesStart<'_>, cell: &mut Cell) -> bool {
    let mut covered = false;
    for attr in e.attributes().flatten() {
        let value = attr.value.as_ref();
        match attr.key.local_name().as_ref() {
            "gridSpan" => cell.col_span = value.parse().unwrap_or(1).max(1),
            "rowSpan" => cell.row_span = value.parse().unwrap_or(1).max(1),
            "hMerge" | "vMerge" => covered |= is_true(value),
            _ => {}
        }
    }
    covered
}

/// Whether a covered `a:tc` is covered from a row above (`vMerge`).
fn is_vertically_covered(e: &plain_libs::xml::events::BytesStart<'_>) -> bool {
    e.attributes()
        .flatten()
        .any(|a| a.key.local_name().as_ref() == "vMerge" && is_true(a.value.as_ref()))
}

fn is_true(value: &str) -> bool {
    value == "1" || value == "true"
}

/// Apply the formatting an `a:rPr` sets on its run -- bold, italic, underline, strikethrough --
/// to `style`. One reader for every place a run is read: a table cell's runs once kept only
/// bold and italic, and lost their underline and strikethrough.
fn apply_run_properties(e: &plain_libs::xml::events::BytesStart<'_>, style: &mut TextStyle) {
    for attr in e.attributes().flatten() {
        let val = crate::decode::attr_value(&attr);
        match attr.key.local_name().as_ref() {
            "b" => style.bold = val != "0" && val != "false",
            "i" => style.italic = val != "0" && val != "false",
            "u" => style.underline = val != "none",
            "strike" => style.strikethrough = val != "noStrike" && val != "0" && val != "false",
            _ => {}
        }
    }
}

/// The runs of the paragraph being read. Every reader of a deck's text -- a shape's, a table
/// cell's, the notes', the placeholders' a layout and master give -- reads a paragraph's runs
/// through this one, so what one of them understands of a run they all do. (Three copies
/// had once kept apart a table cell's formatting; none of the four read a line break or a
/// field.)
#[derive(Default)]
struct RunReader {
    runs: Vec<TextRun>,
    in_run: bool,
    in_text: bool,
    text: String,
    style: TextStyle,
    hyperlink: Option<String>,
}

impl RunReader {
    /// Start reading a new paragraph.
    fn start_paragraph(&mut self) {
        *self = Self::default();
    }

    /// Whether a run or a field is open: an `a:pPr` inside one is not the paragraph's.
    fn in_run(&self) -> bool {
        self.in_run
    }

    /// Whether text read now is a run's.
    fn in_text(&self) -> bool {
        self.in_text
    }

    /// Read an element opened -- or self-closed, when `empty` -- inside the paragraph, and say
    /// whether it was one of the runs'.
    fn element(
        &mut self,
        e: &plain_libs::xml::events::BytesStart<'_>,
        empty: bool,
        rels: &HashMap<String, String>,
    ) -> bool {
        match e.name().local_name().as_ref() {
            // A field (`a:fld` -- a slide number, a date) is a run whose text the application
            // keeps up to date; the value it last showed is its `a:t`.
            "r" | "fld" if !empty => {
                self.in_run = true;
                self.in_text = false;
                self.text.clear();
                self.style = TextStyle::default();
                self.hyperlink = None;
            }
            "t" if self.in_run && !empty => self.in_text = true,
            "rPr" if self.in_run => apply_run_properties(e, &mut self.style),
            "hlinkClick" if self.in_run => {
                if let Some(url) = link_address(e, rels) {
                    self.hyperlink = Some(url);
                }
            }
            // A line break inside the paragraph ends the line of the run before it.
            "br" if !self.in_run => match self.runs.last_mut() {
                Some(run) => run.line_break = true,
                None => self.runs.push(TextRun {
                    text: String::new(),
                    line_break: true,
                    ..TextRun::default()
                }),
            },
            _ => return false,
        }
        true
    }

    /// Text inside the open `a:t`.
    fn push_text(&mut self, text: &str) {
        self.text.push_str(text);
    }

    /// Read an element's end, and say whether it was one of the runs'.
    fn end(&mut self, e: &plain_libs::xml::events::BytesEnd<'_>) -> bool {
        match e.name().local_name().as_ref() {
            "t" if self.in_run => self.in_text = false,
            "r" | "fld" if self.in_run => {
                if !self.text.is_empty() {
                    let hyperlink = self.hyperlink.take();
                    let style = std::mem::take(&mut self.style);
                    self.runs.push(TextRun {
                        text: std::mem::take(&mut self.text),
                        style: if hyperlink.is_some() {
                            style.in_link()
                        } else {
                            style
                        },
                        hyperlink,
                        ..TextRun::default()
                    });
                }
                self.in_run = false;
                self.in_text = false;
                self.hyperlink = None;
            }
            _ => return false,
        }
        true
    }

    /// The paragraph's runs, read to its end: none when it holds no text (a line break
    /// alone is no content), and no break after its last line.
    fn finish(&mut self) -> Vec<TextRun> {
        let mut runs = std::mem::take(&mut self.runs);
        if runs.iter().all(|run| run.text.is_empty()) {
            return Vec::new();
        }
        while runs.last().is_some_and(|run| run.text.is_empty()) {
            runs.pop();
        }
        if let Some(last) = runs.last_mut() {
            last.line_break = false;
        }
        runs
    }
}

/// A shape's placeholder (`p:ph`): its type and index.
struct Placeholder {
    kind: Option<String>,
    idx: Option<String>,
}

impl Placeholder {
    fn of(e: &plain_libs::xml::events::BytesStart<'_>) -> Self {
        let mut kind = None;
        let mut idx = None;
        for attr in e.attributes().flatten() {
            match attr.key.local_name().as_ref() {
                "type" => kind = Some(crate::decode::attr_value(&attr)).filter(|t| !t.is_empty()),
                "idx" => idx = Some(crate::decode::attr_value(&attr)),
                _ => {}
            }
        }
        Self { kind, idx }
    }

    /// The heading a placeholder of this type makes of its text.
    fn heading(&self) -> HeadingLevel {
        match self.kind.as_deref() {
            Some("title" | "ctrTitle") => HeadingLevel::H1,
            Some("subTitle") => HeadingLevel::H2,
            _ => HeadingLevel::None,
        }
    }

    /// The key a layout's and master's text for this placeholder is kept under: its type, or
    /// its index for one that has none.
    fn key(&self) -> String {
        match &self.kind {
            Some(kind) => kind.clone(),
            None => format!("idx:{}", self.idx.as_deref().unwrap_or("0")),
        }
    }
}

/// The address an `a:hlinkClick` links its text to: the target of the relationship it names.
/// A jump to another slide of the deck (`ppaction://hlinksldjump`, `ppaction://hlinkshowjump`)
/// leads to no address the output can point at, and is no link.
fn link_address(
    e: &plain_libs::xml::events::BytesStart<'_>,
    rels: &HashMap<String, String>,
) -> Option<String> {
    let mut id = None;
    let mut action = None;
    for attr in e.attributes().flatten() {
        match attr.key.local_name().as_ref() {
            "id" => id = Some(crate::decode::attr_value(&attr)),
            "action" => action = Some(crate::decode::attr_value(&attr)),
            _ => {}
        }
    }
    let jumps_within = action.is_some_and(|a| {
        a.starts_with("ppaction://hlinksldjump") || a.starts_with("ppaction://hlinkshowjump")
    });
    if jumps_within {
        return None;
    }
    rels.get(&id?).cloned()
}

#[cfg(test)]
#[path = "../../tests/unit/pptx/parser.rs"]
mod tests;
