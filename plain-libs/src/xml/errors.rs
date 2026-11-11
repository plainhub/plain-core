use crate::xml::encoding::EncodingError;
use crate::xml::escape::EscapeError;
use crate::xml::events::attributes::AttrError;
use crate::xml::name::{NamespaceError, QName};
use std::fmt;
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::sync::Arc;
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SyntaxError {
    InvalidBangMarkup,
    UnclosedPI,
    UnclosedXmlDecl,
    UnclosedComment,
    UnclosedDoctype,
    UnclosedCData,
    UnclosedTag,
    UnclosedSingleQuotedAttributeValue,
    UnclosedDoubleQuotedAttributeValue,
}
impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidBangMarkup => f.write_str("unknown or missed symbol in markup"),
            Self::UnclosedPI => {
                f.write_str("processing instruction not closed: `?>` not found before end of input")
            }
            Self::UnclosedXmlDecl => {
                f.write_str("XML declaration not closed: `?>` not found before end of input")
            }
            Self::UnclosedComment => {
                f.write_str("comment not closed: `-->` not found before end of input")
            }
            Self::UnclosedDoctype => {
                f.write_str("DOCTYPE not closed: `>` not found before end of input")
            }
            Self::UnclosedCData => {
                f.write_str("CDATA not closed: `]]>` not found before end of input")
            }
            Self::UnclosedTag => f.write_str("tag not closed: `>` not found before end of input"),
            Self::UnclosedSingleQuotedAttributeValue => {
                f.write_str("attribute value not closed: `'` not found before end of input")
            }
            Self::UnclosedDoubleQuotedAttributeValue => {
                f.write_str("attribute value not closed: `\"` not found before end of input")
            }
        }
    }
}
impl std::error::Error for SyntaxError {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IllFormedError {
    MissingDeclVersion(Option<String>),
    UnknownVersion,
    MissingDoctypeName,
    MissingEndTag(String),
    UnmatchedEndTag(String),
    MismatchedEndTag { expected: String, found: String },
    DoubleHyphenInComment,
    UnclosedReference,
}
impl fmt::Display for IllFormedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::MissingDeclVersion(None) => {
                f.write_str("an XML declaration does not contain `version` attribute")
            }
            Self::MissingDeclVersion(Some(attr)) => {
                write!(
                    f,
                    "an XML declaration must start with `version` attribute, but in starts with `{}`",
                    attr
                )
            }
            Self::UnknownVersion => {
                f.write_str("unknown XML version: either 1.0 or 1.1 is expected")
            }
            Self::MissingDoctypeName => {
                f.write_str("`<!DOCTYPE>` declaration does not contain a name of a document type")
            }
            Self::MissingEndTag(tag) => write!(
                f,
                "start tag not closed: `</{}>` not found before end of input",
                tag,
            ),
            Self::UnmatchedEndTag(tag) => {
                write!(f, "close tag `</{}>` does not match any open tag", tag)
            }
            Self::MismatchedEndTag { expected, found } => write!(
                f,
                "expected `</{}>`, but `</{}>` was found",
                expected, found,
            ),
            Self::DoubleHyphenInComment => {
                f.write_str("forbidden string `--` was found in a comment")
            }
            Self::UnclosedReference => f.write_str(
                "entity or character reference not closed: `;` not found before end of input",
            ),
        }
    }
}
impl std::error::Error for IllFormedError {}
#[derive(Clone, Debug)]
pub enum Error {
    Io(Arc<IoError>),
    Syntax(SyntaxError),
    IllFormed(IllFormedError),
    InvalidAttr(AttrError),
    Encoding(EncodingError),
    Escape(EscapeError),
    Namespace(NamespaceError),
}
impl Error {
    pub(crate) fn missed_end(name: QName) -> Self {
        IllFormedError::MissingEndTag(name.as_ref().to_string()).into()
    }
}
impl From<IoError> for Error {
    #[inline]
    fn from(error: IoError) -> Error {
        match error.kind() {
            IoErrorKind::InvalidData => match error.downcast::<EncodingError>() {
                Ok(err) => Self::Encoding(err),
                Err(err) => Self::Io(Arc::new(err)),
            },
            _ => Self::Io(Arc::new(error)),
        }
    }
}
impl From<SyntaxError> for Error {
    #[inline]
    fn from(error: SyntaxError) -> Self {
        Self::Syntax(error)
    }
}
impl From<IllFormedError> for Error {
    #[inline]
    fn from(error: IllFormedError) -> Self {
        Self::IllFormed(error)
    }
}
impl From<EncodingError> for Error {
    #[inline]
    fn from(error: EncodingError) -> Error {
        Self::Encoding(error)
    }
}
impl From<std::str::Utf8Error> for Error {
    #[inline]
    fn from(error: std::str::Utf8Error) -> Error {
        Self::Encoding(EncodingError::Utf8(error))
    }
}
impl From<EscapeError> for Error {
    #[inline]
    fn from(error: EscapeError) -> Error {
        Self::Escape(error)
    }
}
impl From<AttrError> for Error {
    #[inline]
    fn from(error: AttrError) -> Self {
        Self::InvalidAttr(error)
    }
}
impl From<NamespaceError> for Error {
    #[inline]
    fn from(error: NamespaceError) -> Self {
        Self::Namespace(error)
    }
}
pub type Result<T> = std::result::Result<T, Error>;
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::Syntax(e) => write!(f, "syntax error: {}", e),
            Self::IllFormed(e) => write!(f, "ill-formed document: {}", e),
            Self::InvalidAttr(e) => write!(f, "error while parsing attribute: {}", e),
            Self::Encoding(e) => e.fmt(f),
            Self::Escape(e) => e.fmt(f),
            Self::Namespace(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Syntax(e) => Some(e),
            Self::IllFormed(e) => Some(e),
            Self::InvalidAttr(e) => Some(e),
            Self::Encoding(e) => Some(e),
            Self::Escape(e) => Some(e),
            Self::Namespace(e) => Some(e),
        }
    }
}
