use crate::audio_tags::file::FileType;
use crate::audio_tags::id3::v2::FrameId;
use crate::audio_tags::tag::ItemKey;

use std::collections::TryReserveError;
use std::fmt::{Debug, Display, Formatter};

use crate::audio_tags::ogg::page::PageError;

pub type Result<T> = std::result::Result<T, LoftyError>;

#[derive(Debug)]
#[non_exhaustive]
pub enum ErrorKind {
    // File format related errors
    UnknownFormat,

    // File data related errors
    TooMuchData,

    // TODO: Should probably have context
    SizeMismatch,

    FileDecoding(FileDecodingError),

    FileEncoding(FileEncodingError),

    // Picture related errors
    NotAPicture,

    UnsupportedPicture,

    // Tag related errors
    UnsupportedTag,

    FakeTag,

    TextDecode(&'static str),

    BadTimestamp(&'static str),

    Id3v2(Id3v2Error),

    BadAtom(&'static str),

    AtomMismatch,

    // Conversions for external errors
    OggPage(PageError),

    StringFromUtf8(std::string::FromUtf8Error),

    StrFromUtf8(std::str::Utf8Error),

    Io(std::io::Error),

    Fmt(std::fmt::Error),

    Alloc(TryReserveError),

    Infallible(std::convert::Infallible),
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Id3v2ErrorKind {
    // Header
    BadId3v2Version(u8, u8),

    V2Compression,

    BadExtendedHeaderSize,

    // Frame
    BadFrameId(Vec<u8>),

    UnsupportedFrameId(ItemKey),

    BadFrameLength,

    EmptyFrame(FrameId<'static>),

    MissingDataLengthIndicator,

    InvalidUnsynchronisation,

    V2InvalidTextEncoding,

    BadPictureFormat(String),

    BadSyncText,

    MissingUfidOwner,

    BadRva2ChannelType,

    BadTimestampFormat,

    // Compression
    #[cfg(feature = "media")]
    Decompression(crate::compression::DecompressError),
    #[cfg(not(feature = "media"))]
    CompressedFrameEncountered,

    // Writing
    InvalidEncryptionMethodSymbol(u8),

    BadFrame(String, &'static str),

    InvalidLanguage([u8; 3]),
}

impl Display for Id3v2ErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            // Header
            Self::BadId3v2Version(major, minor) => write!(
                f,
                "Found an invalid version (v{major}.{minor}), expected any major revision in: (2, \
				 3, 4)"
            ),
            Self::V2Compression => write!(f, "Encountered a compressed ID3v2.2 tag"),
            Self::BadExtendedHeaderSize => {
                write!(f, "Found an extended header with an invalid size")
            }

            // Frame
            Self::BadFrameId(frame_id) => write!(f, "Failed to parse a frame ID: 0x{frame_id:x?}"),
            Self::UnsupportedFrameId(item_key) => {
                write!(f, "Unsupported frame ID for item key {item_key:?}")
            }
            Self::BadFrameLength => write!(
                f,
                "Frame isn't long enough to extract the necessary information"
            ),
            Self::EmptyFrame(id) => write!(f, "Frame `{id}` is empty"),
            Self::MissingDataLengthIndicator => write!(
                f,
                "Encountered an encrypted frame without a data length indicator"
            ),
            Self::InvalidUnsynchronisation => write!(f, "Encountered an invalid unsynchronisation"),
            Self::V2InvalidTextEncoding => {
                write!(f, "ID3v2.2 only supports Latin-1 and UTF-16 encodings")
            }
            Self::BadPictureFormat(format) => {
                write!(f, "Picture: Found unexpected format \"{format}\"")
            }
            Self::BadSyncText => write!(f, "Encountered invalid data in SYLT frame"),
            Self::MissingUfidOwner => write!(f, "Missing owner in UFID frame"),
            Self::BadRva2ChannelType => write!(f, "Encountered invalid channel type in RVA2 frame"),
            Self::BadTimestampFormat => write!(
                f,
                "Encountered an invalid timestamp format in a synchronized frame"
            ),

            // Compression
            #[cfg(feature = "media")]
            Self::Decompression(err) => write!(f, "Failed to decompress frame: {err}"),
            #[cfg(not(feature = "media"))]
            Self::CompressedFrameEncountered => write!(
                f,
                "Encountered a compressed ID3v2 frame, support is disabled"
            ),

            // Writing
            Self::InvalidEncryptionMethodSymbol(symbol) => write!(
                f,
                "Attempted to write an encrypted frame with an invalid method symbol ({symbol})"
            ),
            Self::BadFrame(frame_id, frame_value) => write!(
                f,
                "Attempted to write an invalid frame. ID: \"{frame_id}\", Value: \"{frame_value}\"",
            ),
            Self::InvalidLanguage(lang) => write!(
                f,
                "Invalid frame language found: {lang:?} (expected 3 ascii characters)"
            ),
        }
    }
}

pub struct Id3v2Error {
    kind: Id3v2ErrorKind,
}

impl Id3v2Error {
    #[must_use]
    pub const fn new(kind: Id3v2ErrorKind) -> Self {
        Self { kind }
    }

    pub fn kind(&self) -> &Id3v2ErrorKind {
        &self.kind
    }
}

impl Debug for Id3v2Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ID3v2: {:?}", self.kind)
    }
}

impl Display for Id3v2Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ID3v2: {}", self.kind)
    }
}

pub struct FileDecodingError {
    format: Option<FileType>,
    description: &'static str,
}

impl FileDecodingError {
    #[must_use]
    pub const fn new(format: FileType, description: &'static str) -> Self {
        Self {
            format: Some(format),
            description,
        }
    }

    pub fn from_description(description: &'static str) -> Self {
        Self {
            format: None,
            description,
        }
    }

    pub fn format(&self) -> Option<FileType> {
        self.format
    }

    pub fn description(&self) -> &str {
        self.description
    }
}

impl Debug for FileDecodingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(format) = self.format {
            write!(f, "{:?}: {:?}", format, self.description)
        } else {
            write!(f, "{:?}", self.description)
        }
    }
}

impl Display for FileDecodingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(format) = self.format {
            write!(f, "{:?}: {}", format, self.description)
        } else {
            write!(f, "{}", self.description)
        }
    }
}

pub struct FileEncodingError {
    format: Option<FileType>,
    description: &'static str,
}

impl FileEncodingError {
    #[must_use]
    pub const fn new(format: FileType, description: &'static str) -> Self {
        Self {
            format: Some(format),
            description,
        }
    }

    pub fn from_description(description: &'static str) -> Self {
        Self {
            format: None,
            description,
        }
    }

    pub fn format(&self) -> Option<FileType> {
        self.format
    }

    pub fn description(&self) -> &str {
        self.description
    }
}

impl Debug for FileEncodingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(format) = self.format {
            write!(f, "{:?}: {:?}", format, self.description)
        } else {
            write!(f, "{:?}", self.description)
        }
    }
}

impl Display for FileEncodingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(format) = self.format {
            write!(f, "{:?}: {:?}", format, self.description)
        } else {
            write!(f, "{}", self.description)
        }
    }
}

pub struct LoftyError {
    pub(crate) kind: ErrorKind,
}

impl LoftyError {
    #[must_use]
    pub const fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

impl std::error::Error for LoftyError {}

impl Debug for LoftyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

impl From<Id3v2Error> for LoftyError {
    fn from(input: Id3v2Error) -> Self {
        Self {
            kind: ErrorKind::Id3v2(input),
        }
    }
}

impl From<FileDecodingError> for LoftyError {
    fn from(input: FileDecodingError) -> Self {
        Self {
            kind: ErrorKind::FileDecoding(input),
        }
    }
}

impl From<FileEncodingError> for LoftyError {
    fn from(input: FileEncodingError) -> Self {
        Self {
            kind: ErrorKind::FileEncoding(input),
        }
    }
}

impl From<PageError> for LoftyError {
    fn from(input: PageError) -> Self {
        Self {
            kind: ErrorKind::OggPage(input),
        }
    }
}

impl From<std::io::Error> for LoftyError {
    fn from(input: std::io::Error) -> Self {
        Self {
            kind: ErrorKind::Io(input),
        }
    }
}

impl From<std::fmt::Error> for LoftyError {
    fn from(input: std::fmt::Error) -> Self {
        Self {
            kind: ErrorKind::Fmt(input),
        }
    }
}

impl From<std::string::FromUtf8Error> for LoftyError {
    fn from(input: std::string::FromUtf8Error) -> Self {
        Self {
            kind: ErrorKind::StringFromUtf8(input),
        }
    }
}

impl From<std::str::Utf8Error> for LoftyError {
    fn from(input: std::str::Utf8Error) -> Self {
        Self {
            kind: ErrorKind::StrFromUtf8(input),
        }
    }
}

impl From<std::collections::TryReserveError> for LoftyError {
    fn from(input: TryReserveError) -> Self {
        Self {
            kind: ErrorKind::Alloc(input),
        }
    }
}

impl From<std::convert::Infallible> for LoftyError {
    fn from(input: std::convert::Infallible) -> Self {
        Self {
            kind: ErrorKind::Infallible(input),
        }
    }
}

impl Display for LoftyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            // Conversions
            ErrorKind::OggPage(ref err) => write!(f, "{err}"),
            ErrorKind::StringFromUtf8(ref err) => write!(f, "{err}"),
            ErrorKind::StrFromUtf8(ref err) => write!(f, "{err}"),
            ErrorKind::Io(ref err) => write!(f, "{err}"),
            ErrorKind::Fmt(ref err) => write!(f, "{err}"),
            ErrorKind::Alloc(ref err) => write!(f, "{err}"),

            ErrorKind::UnknownFormat => {
                write!(f, "No format could be determined from the provided file")
            }
            ErrorKind::NotAPicture => write!(f, "Picture: Encountered invalid data"),
            ErrorKind::UnsupportedPicture => {
                write!(f, "Picture: attempted to write an unsupported picture")
            }
            ErrorKind::UnsupportedTag => write!(
                f,
                "Attempted to write a tag to a format that does not support it"
            ),
            ErrorKind::FakeTag => write!(f, "Reading: Expected a tag, found invalid data"),
            ErrorKind::TextDecode(message) => write!(f, "Text decoding: {message}"),
            ErrorKind::BadTimestamp(message) => {
                write!(f, "Encountered an invalid timestamp: {message}")
            }
            ErrorKind::Id3v2(ref id3v2_err) => write!(f, "{id3v2_err}"),
            ErrorKind::BadAtom(message) => write!(f, "MP4 Atom: {message}"),
            ErrorKind::AtomMismatch => write!(
                f,
                "MP4 Atom: Attempted to use `Atom::merge()` with mismatching identifiers"
            ),

            // Files
            ErrorKind::TooMuchData => write!(
                f,
                "Attempted to read/write an abnormally large amount of data"
            ),
            ErrorKind::SizeMismatch => write!(
                f,
                "Encountered an invalid item size, either too big or too small to be valid"
            ),
            ErrorKind::FileDecoding(ref file_decode_err) => write!(f, "{file_decode_err}"),
            ErrorKind::FileEncoding(ref file_encode_err) => write!(f, "{file_encode_err}"),

            ErrorKind::Infallible(_) => write!(f, "A expected condition was not upheld"),
        }
    }
}
