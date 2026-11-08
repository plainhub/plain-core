#[cfg(any(feature = "docx", feature = "pptx"))]
mod charts;
#[cfg(any(feature = "doc", feature = "xls", feature = "ppt"))]
mod codepage;
pub mod container;
mod decode;
pub mod detect;
#[cfg(any(feature = "docx", feature = "xlsx", feature = "pptx"))]
mod drawing;
pub mod error;
pub mod model;
#[cfg(any(feature = "xlsx", feature = "xls"))]
mod sheet;
#[cfg(test)]
pub mod streaming;
#[cfg(any(feature = "doc", feature = "xls", feature = "ppt"))]
mod summary;
mod xmlns;

#[cfg(any(feature = "docx", feature = "doc"))]
mod fields;

#[cfg(feature = "docx")]
pub mod docx;

#[cfg(feature = "xlsx")]
pub mod xlsx;

#[cfg(feature = "pptx")]
pub mod pptx;

#[cfg(feature = "doc")]
pub mod doc;

#[cfg(feature = "xls")]
pub mod xls;

#[cfg(feature = "ppt")]
pub mod ppt;

pub mod render;

mod inflate;

// Re-exports
pub use container::{OoxmlContainer, Relationship, Relationships};
pub use detect::detect_format_from_bytes;
#[cfg(not(target_arch = "wasm32"))]
pub use detect::detect_format_from_path;
pub use detect::FormatType;
pub use error::{Error, ErrorKind, Result};
pub use model::{
    Block, Cell, CellAlignment, Document, HeadingLevel, ListInfo, ListType, Metadata, Paragraph,
    Resource, ResourceRole, ResourceType, Row, Section, Table, TextAlignment, TextRun, TextStyle,
};
pub use render::SectionMarkerStyle;
#[cfg(all(test, not(target_arch = "wasm32")))]
pub use streaming::{parse_file_streaming, ParseEvent, SectionStreamOptions};

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

/// Parse a document file and return a Document model.
///
/// This function auto-detects the file format and uses the appropriate parser.
///
/// # Example
///
/// ```no_run
/// use undoc::parse_file;
///
/// let doc = parse_file("document.docx")?;
/// println!("Sections: {}", doc.sections.len());
/// # Ok::<(), undoc::Error>(())
/// ```
#[cfg(not(target_arch = "wasm32"))]
pub fn parse_file(path: impl AsRef<Path>) -> Result<Document> {
    let input = std::fs::File::open(path)?;
    let bytes = limits::read_input(input)?;
    parse_bytes(&bytes)
}

/// Parse a document from bytes.
///
/// # Example
///
/// ```no_run
/// use undoc::parse_bytes;
///
/// let data = std::fs::read("document.docx")?;
/// let doc = parse_bytes(&data)?;
/// # Ok::<(), undoc::Error>(())
/// ```
pub fn parse_bytes(data: &[u8]) -> Result<Document> {
    limits::check_input(data.len())?;
    let format = detect_format_from_bytes(data)?;

    match format {
        #[cfg(feature = "docx")]
        FormatType::Docx => {
            let mut parser = docx::DocxParser::from_bytes(data.to_vec())?;
            parser.parse()
        }
        #[cfg(feature = "xlsx")]
        FormatType::Xlsx => {
            let mut parser = xlsx::XlsxParser::from_bytes(data.to_vec())?;
            parser.parse()
        }
        #[cfg(feature = "pptx")]
        FormatType::Pptx => {
            let mut parser = pptx::PptxParser::from_bytes(data.to_vec())?;
            parser.parse()
        }
        #[cfg(feature = "doc")]
        FormatType::Doc => {
            let mut parser = doc::DocParser::from_bytes(data.to_vec())?;
            parser.parse()
        }
        #[cfg(feature = "xls")]
        FormatType::Xls => {
            let mut parser = xls::XlsParser::from_bytes(data.to_vec())?;
            parser.parse()
        }
        #[cfg(feature = "ppt")]
        FormatType::Ppt => {
            let mut parser = ppt::PptParser::from_bytes(data.to_vec())?;
            parser.parse()
        }
        #[cfg(not(all(
            feature = "docx",
            feature = "xlsx",
            feature = "pptx",
            feature = "doc",
            feature = "xls",
            feature = "ppt"
        )))]
        _ => Err(Error::UnsupportedFormat(format!("{:?}", format))),
    }
}

#[cfg(test)]
mod csv;
mod grid;
mod markdown;

pub mod limits;

#[cfg(test)]
#[path = "../tests/unit/cfb_fixture.rs"]
mod cfb_fixture;
