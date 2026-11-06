//! Error types for the undoc library.

use std::io;
use thiserror::Error;

/// Result type alias for undoc operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Stable classification of an [`Error`], for consumers that must branch on the
/// *reason* a call failed rather than match on its message text.
///
/// Every [`Error`] variant maps onto one of these, so the mapping needs no judgement
/// and stays obvious as the error type grows. The discriminants are explicit and part
/// of the public contract: they cross the C-ABI boundary as `undoc_last_error_kind`
/// return values, so **existing values must never be renumbered** — a new failure
/// reason takes the next free number instead. Treat an unrecognised value as a
/// generic failure rather than as an error.
///
/// Values `100..=199` are reserved for FFI-boundary reasons that have no core `Error`
/// counterpart (null arguments, caught panics, output that cannot cross the ABI); see the
/// `UNDOC_ERROR_*` constants in the `ffi` module. Reasons added after `13` come from
/// undoc's own band, `300..=399`.
///
/// This enum is `#[non_exhaustive]`: match it with a `_ =>` arm and treat an unfamiliar
/// reason as a generic failure. That is the same contract the C, C# and Python surfaces
/// document, and it is what lets a later release name a new reason without breaking
/// callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[repr(i32)]
pub enum ErrorKind {
    /// A failure with no more specific classification.
    ///
    /// [`Error::kind`] never returns this — no core variant maps to it. It exists so
    /// that bindings and consumers have a value for a failure that did not come from
    /// this library and therefore carries no classification. It must not be confused
    /// with success, which is the absence of an error (`0`).
    Other = 1,
    /// [`Error::Io`]
    Io = 2,
    /// [`Error::UnknownFormat`]
    UnknownFormat = 3,
    /// [`Error::UnsupportedFormat`]
    UnsupportedFormat = 4,
    /// [`Error::ZipArchive`]
    ZipArchive = 5,
    /// [`Error::XmlParse`] and [`Error::XmlParseWithContext`] — the same reason,
    /// differing only in whether location context was available.
    XmlParse = 6,
    /// [`Error::InvalidData`]
    InvalidData = 7,
    /// [`Error::MissingComponent`]
    MissingComponent = 8,
    /// [`Error::Encoding`]
    Encoding = 9,
    /// [`Error::StyleNotFound`]
    StyleNotFound = 10,
    /// [`Error::ResourceNotFound`]
    ResourceNotFound = 11,
    /// [`Error::Encrypted`]
    Encrypted = 12,
    /// [`Error::Render`]
    Render = 13,
    /// [`Error::SectionOutOfRange`] — undoc's own band
    /// ([`unparser_shared::kind::library_band`]`(1)`).
    SectionOutOfRange = 300,
}

/// Errors that can occur during document processing.
#[derive(Error, Debug)]
pub enum Error {
    /// I/O error during file operations.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// The file format could not be determined.
    #[error("Unknown file format")]
    UnknownFormat,

    /// The file format is recognized but not supported.
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    /// Error reading ZIP archive.
    #[error("ZIP archive error: {0}")]
    ZipArchive(String),

    /// Error parsing XML content.
    #[error("XML parse error in {location}: {message}")]
    XmlParseWithContext {
        /// The error message
        message: String,
        /// Location context (file path, element name, etc.)
        location: String,
    },

    /// Error parsing XML content (legacy, no context).
    #[error("XML parse error: {0}")]
    XmlParse(String),

    /// Invalid or malformed data in the document.
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// A required document component is missing.
    #[error("Missing component: {0}")]
    MissingComponent(String),

    /// Error during text encoding conversion.
    #[error("Encoding error: {0}")]
    Encoding(String),

    /// A referenced style was not found.
    #[error("Style not found: {0}")]
    StyleNotFound(String),

    /// A referenced resource was not found.
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    /// The document is encrypted and cannot be processed.
    #[error("Document is encrypted")]
    Encrypted,

    /// Error during rendering.
    #[error("Render error: {0}")]
    Render(String),

    /// A section was asked for by an index the document does not have.
    #[error("section {index} is out of range: the document has {count} sections")]
    SectionOutOfRange {
        /// The index asked for (0-based).
        index: usize,
        /// How many sections the document has.
        count: usize,
    },
}

impl From<zip::result::ZipError> for Error {
    /// Preserve the reason the container could not be read.
    ///
    /// The ZIP layer already distinguishes a damaged archive from an unsupported one,
    /// from a missing entry, from one that needs a password. Collapsing all of them
    /// into a single variant would throw that away here, at the one place it is still
    /// known — and no amount of work further up could recover it.
    fn from(err: zip::result::ZipError) -> Self {
        use zip::result::ZipError;

        match err {
            ZipError::Io(e) => Error::Io(e),
            ZipError::InvalidPassword => Error::Encrypted,
            // The ZIP spec has no dedicated "encrypted" status: a password-protected
            // entry surfaces as an unsupported archive with this specific message.
            ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED) => Error::Encrypted,
            ZipError::UnsupportedArchive(what) => Error::UnsupportedFormat(what.to_string()),
            ZipError::FileNotFound => {
                Error::MissingComponent("entry not present in archive".to_string())
            }
            ZipError::InvalidArchive(what) => Error::ZipArchive(what.to_string()),
            // `ZipError` is not `#[non_exhaustive]` today, but treat an unfamiliar
            // variant as a damaged container rather than failing to compile a
            // consumer's build on a dependency bump.
            other => Error::ZipArchive(other.to_string()),
        }
    }
}

impl From<quick_xml::Error> for Error {
    /// Preserve whether the XML failed to *arrive*, to *decode*, or to *parse*.
    ///
    /// These are three different problems for whoever has to act on the failure, and
    /// only this conversion still knows which one happened.
    fn from(err: quick_xml::Error) -> Self {
        match err {
            quick_xml::Error::Io(e) => Error::Io(io::Error::new(e.kind(), e.to_string())),
            quick_xml::Error::Encoding(e) => Error::Encoding(e.to_string()),
            other => Error::XmlParse(other.to_string()),
        }
    }
}

impl Error {
    /// Create an XML parse error with context information.
    ///
    /// # Arguments
    /// * `message` - The error message
    /// * `location` - Context such as file path or element being parsed
    ///
    /// # Example
    /// ```ignore
    /// Error::xml_parse_with_context("Invalid element", "word/document.xml")
    /// ```
    pub fn xml_parse_with_context(message: impl Into<String>, location: impl Into<String>) -> Self {
        Error::XmlParseWithContext {
            message: message.into(),
            location: location.into(),
        }
    }

    /// Classify this error into a stable [`ErrorKind`].
    ///
    /// Lets a caller branch on *why* an operation failed without matching on the
    /// message text. The message stays the human-readable diagnostic; the kind is
    /// additive.
    ///
    /// # Example
    /// ```
    /// use undoc::{Error, ErrorKind};
    ///
    /// let err = Error::UnknownFormat;
    /// assert_eq!(err.kind(), ErrorKind::UnknownFormat);
    /// ```
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::Io(_) => ErrorKind::Io,
            Error::UnknownFormat => ErrorKind::UnknownFormat,
            Error::UnsupportedFormat(_) => ErrorKind::UnsupportedFormat,
            Error::ZipArchive(_) => ErrorKind::ZipArchive,
            Error::XmlParseWithContext { .. } => ErrorKind::XmlParse,
            Error::XmlParse(_) => ErrorKind::XmlParse,
            Error::InvalidData(_) => ErrorKind::InvalidData,
            Error::MissingComponent(_) => ErrorKind::MissingComponent,
            Error::Encoding(_) => ErrorKind::Encoding,
            Error::StyleNotFound(_) => ErrorKind::StyleNotFound,
            Error::ResourceNotFound(_) => ErrorKind::ResourceNotFound,
            Error::Encrypted => ErrorKind::Encrypted,
            Error::Render(_) => ErrorKind::Render,
            Error::SectionOutOfRange { .. } => ErrorKind::SectionOutOfRange,
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/error.rs"]
mod tests;
