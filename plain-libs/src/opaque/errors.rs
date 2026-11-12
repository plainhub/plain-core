// Copyright (c) Meta Platforms, Inc. and affiliates; adapted under the retained source licenses.
use std::fmt;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidLoginError,
    SerializationError,
    ReflectedValueError,
    SizeError {
        name: &'static str,
        len: usize,
        actual_len: usize,
    },
    LibraryError,
}
impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLoginError => f.write_str("Error in validating credentials"),
            Self::SerializationError => f.write_str("Error with serializing / deserializing protocol messages"),
            Self::ReflectedValueError => f.write_str("This error occurs when the client detects that the server has reflected the OPRF value (beta == alpha)"),
            Self::SizeError {name, len, actual_len} => write!(f, "Invalid length for `{name}`: expected {len}, actual {actual_len}"),
            Self::LibraryError => f.write_str("Internal error encountered"),
        }
    }
}
impl std::error::Error for ProtocolError {}
