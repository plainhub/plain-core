//! Format detection for Office Open XML documents.

use crate::container::decode_xml_bytes;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use std::fs::File;
#[cfg(not(target_arch = "wasm32"))]
use std::io::BufReader;
use std::io::{Read, Seek};
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

/// ZIP file magic bytes: PK\x03\x04
const ZIP_MAGIC: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];

/// Compound File Binary (OLE2) magic bytes: D0 CF 11 E0 A1 B1 1A E1.
///
/// Two kinds of file arrive with this header: the legacy binary Office formats
/// (.doc/.xls/.ppt) and an OOXML document protected with ECMA-376 encryption, whose ZIP
/// package is wrapped inside a CFB container. The header alone cannot tell them apart —
/// [`classify_cfb_container`] walks the directory, because the two answers send a caller
/// in opposite directions: one looks for a converter, the other for a password.
pub(crate) const CFB_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// Stream that marks a CFB container as an ECMA-376 encrypted OOXML package.
///
/// Such a file also carries `EncryptionInfo` (the key derivation data), but the payload
/// stream is the one that says "there is a document in here, behind a password".
const ENCRYPTED_PACKAGE_STREAM: &str = "/EncryptedPackage";

/// Content type for DOCX main document part.
const DOCX_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

/// Content type for XLSX workbook part.
const XLSX_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";

/// Content type for PPTX presentation part.
const PPTX_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml";

/// Detected Office document format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatType {
    /// Microsoft Word document (.docx)
    #[default]
    Docx,
    /// Microsoft Excel workbook (.xlsx)
    Xlsx,
    /// Microsoft PowerPoint presentation (.pptx)
    Pptx,
    /// Microsoft Word 97-2003 binary document (.doc)
    Doc,
    /// Microsoft Excel 97-2003 binary workbook (.xls)
    Xls,
    /// Microsoft PowerPoint 97-2003 binary presentation (.ppt)
    Ppt,
}

impl FormatType {
    /// Every format this library knows.
    ///
    /// The enumeration lives here so that consumers do not have to keep their own copy.
    /// A consumer deciding whether to hand a file to this library otherwise hardcodes the
    /// extension list on its side, and that copy goes quietly stale the moment the library
    /// learns a new format -- with nothing to notice the drift. Adding a variant without
    /// adding it here is caught by `all_variants_are_listed` below.
    ///
    /// Each format's parser sits behind a cargo feature of the same name, so a build may
    /// parse fewer than this; [`FormatType::is_enabled`] says which ones this build parses.
    pub const ALL: [FormatType; 6] = [
        FormatType::Docx,
        FormatType::Xlsx,
        FormatType::Pptx,
        FormatType::Doc,
        FormatType::Xls,
        FormatType::Ppt,
    ];

    /// Whether this build parses the format.
    ///
    /// Each format's parser is behind a cargo feature named after it (`docx`, `xlsx`, `pptx`,
    /// `doc`, `xls`, `ppt`; all on by default). A build without one still recognises the
    /// format, and [`parse_bytes`](crate::parse_bytes) rejects it with
    /// [`Error::UnsupportedFormat`]. A caller filtering files by extension asks this rather
    /// than assuming every entry of [`FormatType::ALL`] is parseable.
    pub const fn is_enabled(self) -> bool {
        match self {
            FormatType::Docx => cfg!(feature = "docx"),
            FormatType::Xlsx => cfg!(feature = "xlsx"),
            FormatType::Pptx => cfg!(feature = "pptx"),
            FormatType::Doc => cfg!(feature = "doc"),
            FormatType::Xls => cfg!(feature = "xls"),
            FormatType::Ppt => cfg!(feature = "ppt"),
        }
    }

    /// Returns the file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            FormatType::Docx => "docx",
            FormatType::Xlsx => "xlsx",
            FormatType::Pptx => "pptx",
            FormatType::Doc => "doc",
            FormatType::Xls => "xls",
            FormatType::Ppt => "ppt",
        }
    }

    /// Returns a human-readable name for this format.
    pub fn name(&self) -> &'static str {
        match self {
            FormatType::Docx => "Word Document",
            FormatType::Xlsx => "Excel Workbook",
            FormatType::Pptx => "PowerPoint Presentation",
            FormatType::Doc => "Word 97-2003 Document",
            FormatType::Xls => "Excel 97-2003 Workbook",
            FormatType::Ppt => "PowerPoint 97-2003 Presentation",
        }
    }
}

impl std::fmt::Display for FormatType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Detect the format type from a file path.
///
/// This function reads the file, verifies it's a valid ZIP archive,
/// and inspects the `[Content_Types].xml` to determine the specific format.
///
/// # Example
///
/// ```no_run
/// use undoc::detect::detect_format_from_path;
///
/// let format = detect_format_from_path("document.docx")?;
/// println!("Detected format: {}", format);
/// # Ok::<(), undoc::Error>(())
/// ```
#[cfg(not(target_arch = "wasm32"))]
pub fn detect_format_from_path(path: impl AsRef<Path>) -> Result<FormatType> {
    let file = File::open(path.as_ref())?;
    let reader = BufReader::new(file);
    detect_format_from_reader(reader)
}

/// Detect the format type from a byte slice.
///
/// # Example
///
/// ```no_run
/// use undoc::detect::detect_format_from_bytes;
///
/// let data = std::fs::read("document.docx")?;
/// let format = detect_format_from_bytes(&data)?;
/// # Ok::<(), undoc::Error>(())
/// ```
pub fn detect_format_from_bytes(data: &[u8]) -> Result<FormatType> {
    detect_format_from_reader(std::io::Cursor::new(data))
}

/// Classify the container by its leading bytes.
///
/// Runs before the ZIP layer gets involved, so that a file we can *recognise* but not
/// open is reported as such instead of surfacing as a damaged archive — which would send
/// the caller off to repair a file that is not broken.
fn classify_container_magic<R: Read + Seek>(reader: &mut R) -> Result<Option<FormatType>> {
    let mut head = [0u8; CFB_MAGIC.len()];
    let mut filled = 0;
    while filled < head.len() {
        match reader.read(&mut head[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    reader.seek(std::io::SeekFrom::Start(0))?;

    if filled == CFB_MAGIC.len() && head == CFB_MAGIC {
        let format = classify_cfb_container(reader)?;
        reader.seek(std::io::SeekFrom::Start(0))?;
        return Ok(Some(format));
    }

    if is_bare_biff(&head[..filled]) {
        return Ok(Some(FormatType::Xls));
    }

    if filled < ZIP_MAGIC.len() || head[..ZIP_MAGIC.len()] != ZIP_MAGIC {
        return Err(Error::UnknownFormat);
    }

    Ok(None)
}

/// Say which kind of CFB container this is, having established that it is one.
///
/// An ECMA-376 encrypted OOXML document is a CFB wrapper around the encrypted ZIP
/// package; a legacy binary Office file is a CFB holding its own well-known streams. The
/// header is identical, so only the directory distinguishes them — and the distinction is
/// the whole point: `Encrypted` tells the caller to supply a password, while
/// `UnsupportedFormat` tells them to convert the file. Reporting the disjunction leaves
/// them to guess.
///
/// A Word binary document is answered with its format; the binary formats not yet read, and
/// an unrecognised CFB, are reported as unsupported — naming the format where the directory
/// shows which it is, which is what the header proved.
fn classify_cfb_container<R: Read + Seek>(reader: &mut R) -> Result<FormatType> {
    let container = match cfb::CompoundFile::open(reader) {
        Ok(container) => container,
        // A CFB header whose directory will not parse. Still an Office container, still
        // unopenable; guessing which kind would claim more than was established.
        Err(_) => {
            return Err(Error::UnsupportedFormat(
                "OLE/CFB container whose directory could not be read — a legacy binary \
                 Office format (.doc/.xls/.ppt) or an ECMA-376 encrypted document"
                    .to_string(),
            ));
        }
    };

    if container.exists(ENCRYPTED_PACKAGE_STREAM) {
        return Err(Error::Encrypted);
    }

    if container.exists("/WordDocument") {
        return Ok(FormatType::Doc);
    }
    // `Book` is the workbook stream of Excel 5.0/95 (BIFF5/BIFF7).
    if container.exists("/Workbook") || container.exists("/Book") {
        return Ok(FormatType::Xls);
    }
    if container.exists("/PowerPoint Document") {
        return Ok(FormatType::Ppt);
    }

    Err(Error::UnsupportedFormat(
        "OLE/CFB container that is not a recognised Office document or encrypted OOXML package"
            .to_string(),
    ))
}

/// Whether `head` begins a bare BIFF record stream — how Excel 2.x–4.0 saved a workbook, with
/// no container around it: a `BOF` record of BIFF2, BIFF3 or BIFF4, its expected length, and
/// a substream type a workbook can start with.
fn is_bare_biff(head: &[u8]) -> bool {
    let [k0, k1, s0, s1, _, _, t0, t1, ..] = *head else {
        return false;
    };
    let (kind, size, substream) = (
        u16::from_le_bytes([k0, k1]),
        u16::from_le_bytes([s0, s1]),
        u16::from_le_bytes([t0, t1]),
    );
    let expected_size = match kind {
        0x0009 => 4,
        0x0209 | 0x0409 => 6,
        _ => return false,
    };
    // Worksheet, chart, macro sheet, BIFF4 workbook.
    size == expected_size && matches!(substream, 0x0010 | 0x0020 | 0x0040 | 0x0100)
}

/// Detect the format type from a reader.
///
/// The leading bytes are inspected before the ZIP layer is involved, so the reader is
/// rewound to absolute position 0 rather than continued from wherever the caller left
/// it, and the container must *begin* with a recognised signature — a ZIP archive with
/// data prepended to it (a self-extracting stub, say) is reported as
/// [`Error::UnknownFormat`] instead of being recovered from its central directory.
pub fn detect_format_from_reader<R: Read + Seek>(reader: R) -> Result<FormatType> {
    let mut reader = reader;
    if let Some(format) = classify_container_magic(&mut reader)? {
        return Ok(format);
    }

    let mut archive = zip::ZipArchive::new(reader)?;

    // Try to read [Content_Types].xml
    let content_types = match archive.by_name("[Content_Types].xml") {
        Ok(file) => {
            let compressed = file.compressed_size();
            let bytes = crate::container::read_entry("[Content_Types].xml", file, compressed)?;
            decode_xml_bytes(&bytes)?
        }
        // Only an absent part is a missing component. A container that is damaged or
        // needs a password must say so — reporting either as "the part isn't there"
        // points the caller at the wrong problem.
        Err(zip::result::ZipError::FileNotFound) => {
            return Err(Error::MissingComponent("[Content_Types].xml".to_string()));
        }
        Err(e) => return Err(Error::from(e)),
    };

    // Check content types to determine format
    if content_types.contains(DOCX_CONTENT_TYPE) {
        Ok(FormatType::Docx)
    } else if content_types.contains(XLSX_CONTENT_TYPE) {
        Ok(FormatType::Xlsx)
    } else if content_types.contains(PPTX_CONTENT_TYPE) {
        Ok(FormatType::Pptx)
    } else {
        // Fallback: check for format-specific folders
        detect_by_folder_structure(&mut archive)
    }
}

/// Fallback detection by checking folder structure.
fn detect_by_folder_structure<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<FormatType> {
    let names = archive
        .file_names()
        .map(|name| name.map(String::from))
        .collect::<zip::result::ZipResult<Vec<String>>>()?;

    // Check for format-specific paths
    let has_word = names.iter().any(|n| n.starts_with("word/"));
    let has_xl = names.iter().any(|n| n.starts_with("xl/"));
    let has_ppt = names.iter().any(|n| n.starts_with("ppt/"));

    match (has_word, has_xl, has_ppt) {
        (true, false, false) => Ok(FormatType::Docx),
        (false, true, false) => Ok(FormatType::Xlsx),
        (false, false, true) => Ok(FormatType::Pptx),
        _ => Err(Error::UnknownFormat),
    }
}

/// Check if data starts with ZIP magic bytes.
pub fn is_zip_file(data: &[u8]) -> bool {
    data.len() >= 4 && data[..4] == ZIP_MAGIC
}

#[cfg(test)]
#[path = "../tests/unit/detect.rs"]
mod tests;
