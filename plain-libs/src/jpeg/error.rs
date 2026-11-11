use crate::jpeg::decoder::ColorTransform;
use core::result;
use std::error::Error as StdError;
use std::fmt;
use std::io::Error as IoError;
use std::string::String;
pub type Result<T> = result::Result<T, Error>;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UnsupportedFeature {
    Hierarchical,
    ArithmeticEntropyCoding,
    SamplePrecision(u8),
    ComponentCount(u8),
    DNL,
    NonIntegerSubsamplingRatio,
    ColorTransform(ColorTransform),
}
#[derive(Debug)]
pub enum Error {
    Format(String),
    Unsupported(UnsupportedFeature),
    Io(IoError),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Format(ref desc) => write!(f, "invalid JPEG format: {}", desc),
            Error::Unsupported(ref feat) => write!(f, "unsupported JPEG feature: {:?}", feat),
            Error::Io(ref err) => err.fmt(f),
        }
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match *self {
            Error::Io(ref err) => Some(err),
            _ => None,
        }
    }
}
impl From<IoError> for Error {
    fn from(err: IoError) -> Error {
        Error::Io(err)
    }
}
