use crate::image::color::ExtendedColorType;
use crate::image::{ImageFormat, metadata::Cicp};
use std::collections::TryReserveError;
use std::error::Error;
use std::{fmt, io};
#[derive(Debug)]
pub enum ImageError {
    Decoding(DecodingError),
    Encoding(EncodingError),
    Parameter(ParameterError),
    Limits(LimitError),
    Unsupported(UnsupportedError),
    IoError(io::Error),
}
#[derive(Debug)]
pub struct UnsupportedError {
    format: ImageFormatHint,
    kind: UnsupportedErrorKind,
}
#[derive(Clone, Debug, Hash, PartialEq)]
#[non_exhaustive]
pub enum UnsupportedErrorKind {
    Color(ExtendedColorType),
    ColorLayout(ExtendedColorType),
    ColorspaceCicp(Cicp),
    Format(ImageFormatHint),
    GenericFeature(String),
}
#[derive(Debug)]
pub struct EncodingError {
    format: ImageFormatHint,
    underlying: Option<Box<dyn Error + Send + Sync>>,
}
#[derive(Debug)]
pub struct ParameterError {
    kind: ParameterErrorKind,
    underlying: Option<Box<dyn Error + Send + Sync>>,
}
#[derive(Clone, Debug, Hash, PartialEq)]
#[non_exhaustive]
pub enum ParameterErrorKind {
    DimensionMismatch,
    FailedAlready,
    RgbCicpRequired(Cicp),
    Generic(String),
    NoMoreData,
    CicpMismatch { expected: Cicp, found: Cicp },
}
#[derive(Debug)]
pub struct DecodingError {
    format: ImageFormatHint,
    underlying: Option<Box<dyn Error + Send + Sync>>,
}
#[derive(Debug)]
pub struct LimitError {
    kind: LimitErrorKind,
}
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[non_exhaustive]
pub enum LimitErrorKind {
    DimensionError,
    InsufficientMemory,
    Unsupported {
        limits: crate::image::Limits,
        supported: crate::image::LimitSupport,
    },
}
#[derive(Clone, Debug, Hash, PartialEq)]
#[non_exhaustive]
pub enum ImageFormatHint {
    Exact(ImageFormat),
    Name(String),
    PathExtension(std::path::PathBuf),
    Unknown,
}
impl UnsupportedError {
    #[must_use]
    pub fn from_format_and_kind(format: ImageFormatHint, kind: UnsupportedErrorKind) -> Self {
        UnsupportedError { format, kind }
    }
    #[must_use]
    pub fn kind(&self) -> UnsupportedErrorKind {
        self.kind.clone()
    }
    #[must_use]
    pub fn format_hint(&self) -> ImageFormatHint {
        self.format.clone()
    }
}
impl DecodingError {
    pub fn new(format: ImageFormatHint, err: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        DecodingError {
            format,
            underlying: Some(err.into()),
        }
    }
    #[must_use]
    pub fn from_format_hint(format: ImageFormatHint) -> Self {
        DecodingError {
            format,
            underlying: None,
        }
    }
    #[must_use]
    pub fn format_hint(&self) -> ImageFormatHint {
        self.format.clone()
    }
}
impl EncodingError {
    pub fn new(format: ImageFormatHint, err: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        EncodingError {
            format,
            underlying: Some(err.into()),
        }
    }
    #[must_use]
    pub fn from_format_hint(format: ImageFormatHint) -> Self {
        EncodingError {
            format,
            underlying: None,
        }
    }
    #[must_use]
    pub fn format_hint(&self) -> ImageFormatHint {
        self.format.clone()
    }
}
impl ParameterError {
    #[must_use]
    pub fn from_kind(kind: ParameterErrorKind) -> Self {
        ParameterError {
            kind,
            underlying: None,
        }
    }
    #[must_use]
    pub fn kind(&self) -> ParameterErrorKind {
        self.kind.clone()
    }
}
impl LimitError {
    #[must_use]
    pub fn from_kind(kind: LimitErrorKind) -> Self {
        LimitError { kind }
    }
    #[must_use]
    pub fn kind(&self) -> LimitErrorKind {
        self.kind.clone()
    }
}
impl From<LimitErrorKind> for LimitError {
    fn from(kind: LimitErrorKind) -> Self {
        Self { kind }
    }
}
impl From<io::Error> for ImageError {
    fn from(err: io::Error) -> ImageError {
        ImageError::IoError(err)
    }
}
impl From<TryReserveError> for ImageError {
    fn from(_: TryReserveError) -> ImageError {
        ImageError::Limits(LimitErrorKind::InsufficientMemory.into())
    }
}
impl From<ImageFormat> for ImageFormatHint {
    fn from(format: ImageFormat) -> Self {
        ImageFormatHint::Exact(format)
    }
}
impl From<&'_ std::path::Path> for ImageFormatHint {
    fn from(path: &'_ std::path::Path) -> Self {
        match path.extension() {
            Some(ext) => ImageFormatHint::PathExtension(ext.into()),
            None => ImageFormatHint::Unknown,
        }
    }
}
impl From<ImageFormatHint> for UnsupportedError {
    fn from(hint: ImageFormatHint) -> Self {
        UnsupportedError {
            format: hint.clone(),
            kind: UnsupportedErrorKind::Format(hint),
        }
    }
}
pub type ImageResult<T> = Result<T, ImageError>;
impl fmt::Display for ImageError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            ImageError::IoError(err) => err.fmt(fmt),
            ImageError::Decoding(err) => err.fmt(fmt),
            ImageError::Encoding(err) => err.fmt(fmt),
            ImageError::Parameter(err) => err.fmt(fmt),
            ImageError::Limits(err) => err.fmt(fmt),
            ImageError::Unsupported(err) => err.fmt(fmt),
        }
    }
}
impl Error for ImageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ImageError::IoError(err) => err.source(),
            ImageError::Decoding(err) => err.source(),
            ImageError::Encoding(err) => err.source(),
            ImageError::Parameter(err) => err.source(),
            ImageError::Limits(err) => err.source(),
            ImageError::Unsupported(err) => err.source(),
        }
    }
}
impl fmt::Display for UnsupportedError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match &self.kind {
            UnsupportedErrorKind::Format(ImageFormatHint::Unknown) => {
                write!(fmt, "The image format could not be determined",)
            }
            UnsupportedErrorKind::Format(format @ ImageFormatHint::PathExtension(_)) => write!(
                fmt,
                "The file extension {format} was not recognized as an image format",
            ),
            UnsupportedErrorKind::Format(format) => {
                write!(fmt, "The image format {format} is not supported",)
            }
            UnsupportedErrorKind::Color(color) => write!(
                fmt,
                "The encoder or decoder for {} does not support the color type `{:?}`",
                self.format, color,
            ),
            UnsupportedErrorKind::ColorLayout(layout) => write!(
                fmt,
                "Converting with the texel memory layout {layout:?} is not supported",
            ),
            UnsupportedErrorKind::ColorspaceCicp(color) => write!(
                fmt,
                "The colorimetric interpretation of a CICP color space is not supported for `{color:?}`",
            ),
            UnsupportedErrorKind::GenericFeature(message) => match &self.format {
                ImageFormatHint::Unknown => write!(
                    fmt,
                    "The decoder does not support the format feature {message}",
                ),
                other => write!(
                    fmt,
                    "The decoder for {other} does not support the format features {message}",
                ),
            },
        }
    }
}
impl Error for UnsupportedError {}
impl fmt::Display for ParameterError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match &self.kind {
            ParameterErrorKind::DimensionMismatch => write!(
                fmt,
                "The Image's dimensions are either too \
                 small or too large"
            ),
            ParameterErrorKind::FailedAlready => write!(
                fmt,
                "The end the image stream has been reached due to a previous error"
            ),
            ParameterErrorKind::RgbCicpRequired(cicp) => {
                write!(fmt, "The CICP {cicp:?} can not be used for RGB images",)
            }
            ParameterErrorKind::Generic(message) => {
                write!(fmt, "The parameter is malformed: {message}",)
            }
            ParameterErrorKind::NoMoreData => write!(fmt, "The end of the image has been reached",),
            ParameterErrorKind::CicpMismatch { expected, found } => {
                write!(
                    fmt,
                    "The color space {found:?} does not match the expected {expected:?}",
                )
            }
        }?;
        if let Some(underlying) = &self.underlying {
            write!(fmt, "\n{underlying}")?;
        }
        Ok(())
    }
}
impl Error for ParameterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.underlying {
            None => None,
            Some(source) => Some(&**source),
        }
    }
}
impl fmt::Display for EncodingError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match &self.underlying {
            Some(underlying) => write!(
                fmt,
                "Format error encoding {}:\n{}",
                self.format, underlying,
            ),
            None => write!(fmt, "Format error encoding {}", self.format,),
        }
    }
}
impl Error for EncodingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.underlying {
            None => None,
            Some(source) => Some(&**source),
        }
    }
}
impl fmt::Display for DecodingError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match &self.underlying {
            None => match self.format {
                ImageFormatHint::Unknown => write!(fmt, "Format error"),
                _ => write!(fmt, "Format error decoding {}", self.format),
            },
            Some(underlying) => {
                write!(fmt, "Format error decoding {}: {}", self.format, underlying)
            }
        }
    }
}
impl Error for DecodingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.underlying {
            None => None,
            Some(source) => Some(&**source),
        }
    }
}
impl fmt::Display for LimitError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self.kind {
            LimitErrorKind::InsufficientMemory => write!(fmt, "Memory limit exceeded"),
            LimitErrorKind::DimensionError => write!(fmt, "Image size exceeds limit"),
            LimitErrorKind::Unsupported { .. } => {
                write!(
                    fmt,
                    "The following strict limits are specified but not supported by the opertation: "
                )?;
                Ok(())
            }
        }
    }
}
impl Error for LimitError {}
impl fmt::Display for ImageFormatHint {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            ImageFormatHint::Exact(format) => write!(fmt, "{format:?}"),
            ImageFormatHint::Name(name) => write!(fmt, "`{name}`"),
            ImageFormatHint::PathExtension(ext) => write!(fmt, "`.{ext:?}`"),
            ImageFormatHint::Unknown => write!(fmt, "`Unknown`"),
        }
    }
}
#[derive(Clone)]
pub struct TryFromExtendedColorError {
    pub(crate) was: ExtendedColorType,
}
impl fmt::Debug for TryFromExtendedColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}
impl fmt::Display for TryFromExtendedColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "The pixel layout {:?} is not supported as a buffer ColorType",
            self.was
        )
    }
}
impl Error for TryFromExtendedColorError {}
impl From<TryFromExtendedColorError> for ImageError {
    fn from(err: TryFromExtendedColorError) -> ImageError {
        ImageError::Unsupported(UnsupportedError::from_format_and_kind(
            ImageFormatHint::Unknown,
            UnsupportedErrorKind::Color(err.was),
        ))
    }
}
