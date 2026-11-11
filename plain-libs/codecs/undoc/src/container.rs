//! ZIP container abstraction for OOXML documents.

use crate::decode::{normalize_line_endings, resolve_general_ref};
use crate::error::{Error, Result};
use crate::inflate::{is_inflation_limit, InflateGuard};
use crate::model::Metadata;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::fs::File;
#[cfg(not(target_arch = "wasm32"))]
use std::io::BufReader;
use std::io::{Cursor, Read, Seek};
use std::path::Path;

/// Classify a failure to open an entry inside the archive.
///
/// Only a genuinely absent entry is a missing component — that is the case
/// [`OoxmlContainer::read_xml_optional`] turns into `Ok(None)`. Every other reason (a
/// damaged container, one that needs a password) has to keep its own classification,
/// or the optional-part path silently degrades a broken file into a valid-empty
/// result — exactly what that method promises not to do.
fn entry_error(path: &str, err: plain_libs::zip::result::ZipError) -> Error {
    match err {
        plain_libs::zip::result::ZipError::FileNotFound => Error::MissingComponent(path.to_string()),
        other => Error::from(other),
    }
}

/// The bytes of the entry `path`, read from its decompressing reader `file` through
/// [`InflateGuard`]: an entry that expands past what a document's data does (a
/// decompression bomb) is a damaged archive, not an allocation that ends the process.
pub(crate) fn read_entry(path: &str, file: impl Read, compressed_size: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    match InflateGuard::new(file, compressed_size).read_to_end(&mut bytes) {
        Ok(_) => Ok(bytes),
        Err(e) if is_inflation_limit(&e) => Err(Error::ZipArchive(format!("{path}: {e}"))),
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelationshipPolicy {
    Required,
    Optional,
}

/// A relationship entry from a .rels file.
#[derive(Debug, Clone)]
pub struct Relationship {
    /// Relationship ID (e.g., "rId1")
    pub id: String,
    /// Relationship type URI
    pub rel_type: String,
    /// Target path (relative or absolute)
    pub target: String,
    /// Whether the target is external
    pub external: bool,
}

/// Collection of relationships parsed from a .rels file.
#[derive(Debug, Clone, Default)]
pub struct Relationships {
    /// Map from relationship ID to relationship data
    pub by_id: HashMap<String, Relationship>,
    /// Map from relationship type to list of relationships
    pub by_type: HashMap<String, Vec<Relationship>>,
}

impl Relationships {
    /// Create a new empty relationships collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a relationship by ID.
    pub fn get(&self, id: &str) -> Option<&Relationship> {
        self.by_id.get(id)
    }

    /// Get relationships by type.
    pub fn get_by_type(&self, rel_type: &str) -> Vec<&Relationship> {
        self.by_type
            .get(rel_type)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Add a relationship.
    pub fn add(&mut self, rel: Relationship) {
        self.by_type
            .entry(rel.rel_type.clone())
            .or_default()
            .push(rel.clone());
        self.by_id.insert(rel.id.clone(), rel);
    }

    /// Consume the collection into an ID → target map.
    pub fn into_targets_by_id(self) -> HashMap<String, String> {
        self.by_id
            .into_iter()
            .map(|(id, rel)| (id, rel.target))
            .collect()
    }

    /// Consume the collection into an ID → (type, target) map.
    pub fn into_type_targets_by_id(self) -> HashMap<String, (String, String)> {
        self.by_id
            .into_iter()
            .map(|(id, rel)| (id, (rel.rel_type, rel.target)))
            .collect()
    }
}

/// Restate the XML declaration's encoding as UTF-8 after a decode.
///
/// Once the bytes have been decoded into a Rust `String` the content *is* UTF-8, so a
/// declaration still naming the source encoding contradicts the string carrying it.
/// Callers get the part as text and may re-serialize or re-read it; leaving the stale
/// name in place hands them a document that describes itself wrongly.
///
/// Whatever the declared value was, it is no longer the truth — so the value is replaced
/// rather than matched against a list of spellings (`UTF-16`, `utf-16le`, `UTF-16BE`, …).
fn fix_xml_encoding_declaration(content: &str) -> String {
    if !content.starts_with("<?xml") {
        return content.to_string();
    }
    let Some(end_decl) = content.find("?>") else {
        return content.to_string();
    };
    let (decl, rest) = content.split_at(end_decl + 2);

    let Some(value_start) = decl.find("encoding=") else {
        return content.to_string();
    };
    let after = &decl[value_start + "encoding=".len()..];
    let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
        return content.to_string();
    };
    let Some(value_len) = after[1..].find(quote) else {
        return content.to_string();
    };

    format!(
        "{}encoding={quote}UTF-8{quote}{}{}",
        &decl[..value_start],
        &after[1 + value_len + 1..],
        rest
    )
}

/// OOXML container abstraction over a ZIP archive.
///
/// Provides methods to read XML files, binary data, and relationships
/// from an Office Open XML document.
pub struct OoxmlContainer {
    archive: RefCell<plain_libs::zip::ZipArchive<Cursor<Vec<u8>>>>,
    /// Cached package-level relationships (used in Phase 2+)
    inflated_bytes: Cell<u64>,
}

/// Decode XML bytes handling different encodings (UTF-8, UTF-16 LE/BE).
///
/// OOXML files are typically UTF-8 encoded, but some (especially older
/// or non-standard documents) may use UTF-16 encoding.
///
/// A BOM decides the encoding outright. Without one, UTF-16 is detected *before* UTF-8 is
/// attempted, because BOM-less UTF-16 of ASCII text is a valid UTF-8 byte sequence: every
/// second byte is NUL, and NUL is a legal UTF-8 encoding of U+0000. Trying UTF-8 first
/// would therefore succeed and yield a string whose element names are interleaved with
/// NUL — no XML reader matches anything in it, and the part parses to nothing at all. The
/// detection cannot misfire on real UTF-8 XML, since XML 1.0 forbids U+0000 in content.
pub fn decode_xml_bytes(bytes: &[u8]) -> Result<String> {
    // Check for BOM (Byte Order Mark)
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        // UTF-8 BOM: EF BB BF - skip BOM and decode as UTF-8
        return String::from_utf8(bytes[3..].to_vec())
            .map_err(|e| Error::Encoding(format!("UTF-8 XML part has invalid bytes: {e}")));
    }

    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        // UTF-16 LE BOM: FF FE
        let content = decode_utf16_le(&bytes[2..])?;
        // Fix XML declaration encoding to UTF-8 since we've already converted
        return Ok(fix_xml_encoding_declaration(&content));
    }

    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        // UTF-16 BE BOM: FE FF
        let content = decode_utf16_be(&bytes[2..])?;
        // Fix XML declaration encoding to UTF-8 since we've already converted
        return Ok(fix_xml_encoding_declaration(&content));
    }

    // No BOM. XML 1.0 requires one on UTF-16 entities, so this is already non-conformant
    // input — but it is recoverable, and the NUL pattern says which half of the code unit
    // holds the ASCII byte.
    if bytes.len() >= 4 && bytes[1] == 0 && bytes[3] == 0 {
        let content = decode_utf16_le(bytes)?;
        return Ok(fix_xml_encoding_declaration(&content));
    }
    if bytes.len() >= 4 && bytes[0] == 0 && bytes[2] == 0 {
        let content = decode_utf16_be(bytes)?;
        return Ok(fix_xml_encoding_declaration(&content));
    }

    String::from_utf8(bytes.to_vec())
        .map_err(|_| Error::Encoding("XML part is not valid UTF-8 or UTF-16".to_string()))
}

/// Decode UTF-16 Little Endian bytes to String.
fn decode_utf16_le(bytes: &[u8]) -> Result<String> {
    if !bytes.len().is_multiple_of(2) {
        return Err(Error::Encoding(
            "UTF-16 little-endian XML has an odd byte length".to_string(),
        ));
    }

    let u16_iter = (0..bytes.len())
        .step_by(2)
        .map(|i| u16::from_le_bytes([bytes[i], bytes[i + 1]]));

    char::decode_utf16(u16_iter)
        .collect::<std::result::Result<String, _>>()
        .map_err(|e| Error::Encoding(format!("UTF-16 little-endian XML is malformed: {e}")))
}

/// Decode UTF-16 Big Endian bytes to String.
fn decode_utf16_be(bytes: &[u8]) -> Result<String> {
    if !bytes.len().is_multiple_of(2) {
        return Err(Error::Encoding(
            "UTF-16 big-endian XML has an odd byte length".to_string(),
        ));
    }

    let u16_iter = (0..bytes.len())
        .step_by(2)
        .map(|i| u16::from_be_bytes([bytes[i], bytes[i + 1]]));

    char::decode_utf16(u16_iter)
        .collect::<std::result::Result<String, _>>()
        .map_err(|e| Error::Encoding(format!("UTF-16 big-endian XML is malformed: {e}")))
}

/// The names of an archive's entries.
///
/// `zip` reports each name as a result because it decodes it: as UTF-8, and otherwise as
/// CP437. CP437 maps every byte, so decoding cannot fail — a name it could not decode would
/// not be one a part could be read by either.
fn entry_names<R: Read + Seek>(
    archive: &plain_libs::zip::ZipArchive<R>,
) -> impl Iterator<Item = std::borrow::Cow<'_, str>> {
    archive.file_names().filter_map(|name| name.ok())
}

impl OoxmlContainer {
    fn rels_path_for_part(part_path: &str) -> String {
        if part_path.is_empty() || part_path == "/" {
            "_rels/.rels".to_string()
        } else {
            let path = Path::new(part_path);
            let parent = path.parent().unwrap_or(Path::new(""));
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            format!("{}/_rels/{}.rels", parent.display(), filename)
        }
    }

    /// Open an OOXML container from a file path.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use undoc::container::OoxmlContainer;
    ///
    /// let container = OoxmlContainer::open("document.docx")?;
    /// # Ok::<(), undoc::Error>(())
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path.as_ref())?;
        let mut reader = BufReader::new(file);
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;
        Self::from_bytes(data)
    }

    /// Create an OOXML container from a byte vector.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        crate::limits::check_input(data.len())?;
        let cursor = Cursor::new(data);
        let mut archive = plain_libs::zip::ZipArchive::new(cursor)?;
        if archive.len() > crate::limits::MAX_PARTS {
            return Err(Error::InvalidData(
                "Document archive entry limit exceeded".into(),
            ));
        }
        let mut total = 0u64;
        for index in 0..archive.len() {
            let entry = archive.by_index_raw(index)?;
            total = total.saturating_add(entry.size());
            if entry.size() > crate::limits::MAX_PART_BYTES
                || total > crate::limits::MAX_DOCUMENT_BYTES
            {
                return Err(Error::InvalidData(
                    "Document archive size limit exceeded".into(),
                ));
            }
        }
        Ok(Self {
            archive: RefCell::new(archive),
            inflated_bytes: Cell::new(0),
        })
    }

    /// Create an OOXML container from a reader.
    pub fn from_reader<R: Read + Seek>(mut reader: R) -> Result<Self> {
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;
        Self::from_bytes(data)
    }

    /// Read an XML file from the archive as a string.
    ///
    /// Handles different encodings:
    /// - UTF-8 (with or without BOM)
    /// - UTF-16 LE (with BOM: FF FE)
    /// - UTF-16 BE (with BOM: FE FF)
    pub fn read_xml(&self, path: &str) -> Result<String> {
        let mut archive = self.archive.borrow_mut();
        let file = archive.by_name(path).map_err(|e| entry_error(path, e))?;
        let compressed = file.compressed_size();
        let bytes = read_entry(path, file, compressed)?;
        self.record_inflated(bytes.len())?;

        // Detect encoding from BOM
        let content = decode_xml_bytes(&bytes)?;
        crate::limits::check_xml(&content).map_err(|error| match error {
            Error::XmlParse(message) => Error::XmlParseWithContext {
                location: path.to_owned(),
                message,
            },
            error => error,
        })?;
        // Readers match elements by their written prefix; give them the conventional one
        // whatever prefix the writer chose (see `xmlns`).
        Ok(crate::xmlns::canonical_prefixes(content))
    }

    /// Read an optional XML part.
    ///
    /// Returns `Ok(None)` when the part is absent (`Error::MissingComponent`),
    /// but surfaces any other error — including `Error::Encoding` for malformed
    /// byte content — so that corrupted optional parts are never silently
    /// degraded into a valid-empty result.
    pub fn read_xml_optional(&self, path: &str) -> Result<Option<String>> {
        match self.read_xml(path) {
            Ok(content) => Ok(Some(content)),
            Err(Error::MissingComponent(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Read a binary file from the archive.
    pub fn read_binary(&self, path: &str) -> Result<Vec<u8>> {
        let mut archive = self.archive.borrow_mut();
        let file = archive.by_name(path).map_err(|e| entry_error(path, e))?;
        let compressed = file.compressed_size();
        let bytes = read_entry(path, file, compressed)?;
        self.record_inflated(bytes.len())?;
        Ok(bytes)
    }

    fn record_inflated(&self, size: usize) -> Result<()> {
        let total = self.inflated_bytes.get().saturating_add(size as u64);
        if total > crate::limits::MAX_DOCUMENT_BYTES {
            return Err(Error::InvalidData(
                "Document decompression budget exceeded".into(),
            ));
        }
        self.inflated_bytes.set(total);
        Ok(())
    }

    /// Check if a file exists in the archive.
    pub fn exists(&self, path: &str) -> bool {
        let archive = self.archive.borrow();
        let result = entry_names(&archive).any(|n| n == path);
        result
    }

    /// List all files in the archive.
    pub fn list_files(&self) -> Vec<String> {
        let archive = self.archive.borrow();
        entry_names(&archive).map(String::from).collect()
    }

    /// List files matching a prefix.
    pub fn list_files_with_prefix(&self, prefix: &str) -> Vec<String> {
        let archive = self.archive.borrow();
        entry_names(&archive)
            .filter(|n| n.starts_with(prefix))
            .map(String::from)
            .collect()
    }

    /// Read and parse relationships from a .rels file.
    ///
    /// Legacy helper: treats missing relationship parts as optional.
    pub fn read_relationships(&self, part_path: &str) -> Result<Relationships> {
        self.read_optional_relationships_for_part(part_path)
    }

    /// Read relationships for a required OOXML part.
    pub fn read_required_relationships_for_part(&self, part_path: &str) -> Result<Relationships> {
        let rels_path = Self::rels_path_for_part(part_path);
        self.read_relationships_at_path(&rels_path, RelationshipPolicy::Required)
    }

    /// Read relationships for an optional OOXML part.
    pub fn read_optional_relationships_for_part(&self, part_path: &str) -> Result<Relationships> {
        let rels_path = Self::rels_path_for_part(part_path);
        self.read_relationships_at_path(&rels_path, RelationshipPolicy::Optional)
    }

    fn read_relationships_at_path(
        &self,
        rels_path: &str,
        policy: RelationshipPolicy,
    ) -> Result<Relationships> {
        let content = match self.read_xml(rels_path) {
            Ok(content) => content,
            Err(Error::MissingComponent(_)) if policy == RelationshipPolicy::Optional => {
                return Ok(Relationships::new());
            }
            Err(err) => return Err(err),
        };

        parse_relationships_xml(&content, rels_path)
    }

    /// Read package-level relationships (_rels/.rels).
    pub fn read_package_relationships(&self) -> Result<Relationships> {
        self.read_relationships_at_path("_rels/.rels", RelationshipPolicy::Required)
    }

    /// Parse core metadata from docProps/core.xml.
    ///
    /// This is common to all OOXML formats (DOCX, XLSX, PPTX).
    pub fn parse_core_metadata(&self) -> Result<Metadata> {
        let mut meta = Metadata::default();

        match self.read_xml("docProps/core.xml") {
            Ok(xml) => {
                let mut reader = crate::decode::reader_for(&xml);
                reader.config_mut().trim_text(false);

                let mut buf = Vec::new();
                let mut current_element: Option<String> = None;
                // Accumulate an element's text across events. quick-xml 0.40+ splits
                // a value like "Tom &amp; Jerry" into Text/GeneralRef/Text, so the
                // value must be built up and flushed at the element's End rather than
                // assigned per Text event (which would drop all but the last fragment).
                let mut current_text = String::new();

                loop {
                    match reader.read_event_into(&mut buf) {
                        Ok(plain_libs::xml::events::Event::Start(e)) => {
                            let name = e.name();
                            current_element = Some(name.local_name().as_ref().to_string());
                            current_text.clear();
                        }
                        Ok(plain_libs::xml::events::Event::Text(e)) if current_element.is_some() => {
                            current_text.push_str(&metadata_text_or_raw(&e, "docProps/core.xml")?);
                        }
                        Ok(plain_libs::xml::events::Event::GeneralRef(e))
                            if current_element.is_some() =>
                        {
                            current_text.push_str(&resolve_general_ref(&e));
                        }
                        Ok(plain_libs::xml::events::Event::End(_)) => {
                            if let Some(ref elem) = current_element {
                                let text = normalize_line_endings(Cow::Borrowed(&current_text))
                                    .into_owned();
                                match elem.as_str() {
                                    "title" => meta.title = Some(text),
                                    "creator" => meta.author = Some(text),
                                    "subject" => meta.subject = Some(text),
                                    "description" => meta.description = Some(text),
                                    "keywords" => {
                                        meta.keywords = text
                                            .split([',', ';'])
                                            .map(|s| s.trim().to_string())
                                            .filter(|s| !s.is_empty())
                                            .collect();
                                    }
                                    "created" => meta.created = Some(text),
                                    "modified" => meta.modified = Some(text),
                                    "lastModifiedBy" => meta.last_modified_by = Some(text),
                                    _ => {}
                                }
                            }
                            current_element = None;
                            current_text.clear();
                        }
                        Ok(plain_libs::xml::events::Event::Eof) => break,
                        Err(e) => {
                            return Err(Error::xml_parse_with_context(
                                e.to_string(),
                                "docProps/core.xml",
                            ));
                        }
                        _ => {}
                    }
                    buf.clear();
                }
            }
            Err(Error::MissingComponent(_)) => {}
            Err(err) => return Err(err),
        }

        // Enrich with app.xml metadata
        self.parse_app_metadata(&mut meta)?;

        Ok(meta)
    }

    /// Parse extended metadata from docProps/app.xml.
    ///
    /// Extracts Application, Pages, Words, and Slides properties.
    /// Only sets `page_count` and `word_count` if not already populated.
    fn parse_app_metadata(&self, meta: &mut Metadata) -> Result<()> {
        let xml = match self.read_xml("docProps/app.xml") {
            Ok(xml) => xml,
            Err(Error::MissingComponent(_)) => return Ok(()),
            Err(err) => return Err(err),
        };

        let mut reader = crate::decode::reader_for(&xml);
        reader.config_mut().trim_text(false);

        let mut buf = Vec::new();
        let mut current_element: Option<String> = None;
        // Accumulate per element and flush at End — see parse_core_metadata for
        // why per-Text assignment is unsafe under quick-xml 0.40+.
        let mut current_text = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(plain_libs::xml::events::Event::Start(e)) => {
                    let name = e.name();
                    current_element = Some(name.local_name().as_ref().to_string());
                    current_text.clear();
                }
                Ok(plain_libs::xml::events::Event::Text(e)) if current_element.is_some() => {
                    current_text.push_str(&metadata_text_or_raw(&e, "docProps/app.xml")?);
                }
                Ok(plain_libs::xml::events::Event::GeneralRef(e)) if current_element.is_some() => {
                    current_text.push_str(&resolve_general_ref(&e));
                }
                Ok(plain_libs::xml::events::Event::End(_)) => {
                    if let Some(ref elem) = current_element {
                        let text =
                            normalize_line_endings(Cow::Borrowed(&current_text)).into_owned();
                        match elem.as_str() {
                            "Application" => meta.application = Some(text),
                            "Pages" if meta.page_count.is_none() => {
                                meta.page_count = text.trim().parse::<u32>().ok();
                            }
                            "Words" if meta.word_count.is_none() => {
                                meta.word_count = text.trim().parse::<u32>().ok();
                            }
                            "Slides" if meta.page_count.is_none() => {
                                meta.page_count = text.trim().parse::<u32>().ok();
                            }
                            _ => {}
                        }
                    }
                    current_element = None;
                    current_text.clear();
                }
                Ok(plain_libs::xml::events::Event::Eof) => break,
                Err(e) => {
                    return Err(Error::xml_parse_with_context(
                        e.to_string(),
                        "docProps/app.xml",
                    ));
                }
                _ => {}
            }
            buf.clear();
        }

        Ok(())
    }

    /// Resolve a relative path from a base path.
    pub fn resolve_path(base: &str, relative: &str) -> String {
        if let Some(stripped) = relative.strip_prefix('/') {
            return stripped.to_string();
        }

        let base_path = Path::new(base);
        let base_dir = base_path.parent().unwrap_or(Path::new(""));

        let mut result = base_dir.to_path_buf();
        for component in Path::new(relative).components() {
            match component {
                std::path::Component::ParentDir => {
                    result.pop();
                }
                std::path::Component::Normal(c) => {
                    result.push(c);
                }
                _ => {}
            }
        }

        result.to_string_lossy().replace('\\', "/")
    }
}

fn metadata_text_or_raw(text: &plain_libs::xml::events::BytesText<'_>, location: &str) -> Result<String> {
    crate::decode::decode_text_strict(text, location)
}

fn parse_relationship_element(
    e: &plain_libs::xml::events::BytesStart<'_>,
    location: &str,
) -> Result<Relationship> {
    let mut id = String::new();
    let mut rel_type = String::new();
    let mut target = String::new();
    let mut external = false;

    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            "Id" => id = attr.value.as_ref().to_string(),
            "Type" => rel_type = attr.value.as_ref().to_string(),
            "Target" => target = attr.value.as_ref().to_string(),
            "TargetMode" => external = attr.value.as_ref().eq_ignore_ascii_case("external"),
            _ => {}
        }
    }

    let mut missing = Vec::new();
    if id.is_empty() {
        missing.push("Id");
    }
    if rel_type.is_empty() {
        missing.push("Type");
    }
    if target.is_empty() {
        missing.push("Target");
    }

    if !missing.is_empty() {
        return Err(Error::xml_parse_with_context(
            format!(
                "Relationship element missing required attribute(s): {}",
                missing.join(", ")
            ),
            location,
        ));
    }

    Ok(Relationship {
        id,
        rel_type,
        target,
        external,
    })
}

pub(crate) fn parse_relationships_xml(content: &str, location: &str) -> Result<Relationships> {
    if content.trim().is_empty() {
        return Err(Error::xml_parse_with_context(
            "relationship file is empty",
            location,
        ));
    }

    let mut rels = Relationships::new();
    let mut reader = crate::decode::reader_for(content);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(plain_libs::xml::events::Event::Empty(e)) | Ok(plain_libs::xml::events::Event::Start(e))
                if e.name().local_name().as_ref() == "Relationship" =>
            {
                rels.add(parse_relationship_element(&e, location)?);
            }
            Ok(plain_libs::xml::events::Event::Eof) => break,
            Err(e) => return Err(Error::xml_parse_with_context(e.to_string(), location)),
            _ => {}
        }
        buf.clear();
    }

    Ok(rels)
}

impl std::fmt::Debug for OoxmlContainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OoxmlContainer")
            .field("files", &self.list_files().len())
            .finish()
    }
}

#[cfg(test)]
#[path = "../tests/unit/container.rs"]
mod tests;
