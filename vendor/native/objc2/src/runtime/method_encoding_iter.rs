//! Utility for parsing an Objective-C method type encoding.
//!
//! TODO: Move this to `objc2-encode` when more stable.
use core::fmt;
use core::num::ParseIntError;
use std::error::Error;

use crate::encode::{Encoding, EncodingBox, ParseError};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MethodEncodingIter<'a> {
    s: &'a str,
}

impl<'a> MethodEncodingIter<'a> {
    pub(crate) fn new(s: &'a str) -> Self {
        Self { s }
    }

    pub(crate) fn extract_return(
        &mut self,
    ) -> Result<(EncodingBox, Option<isize>), EncodingParseError> {
        // TODO: Verify stack layout
        self.next().ok_or(EncodingParseError::MissingReturn)?
    }

    pub(crate) fn verify_receiver(&mut self) -> Result<(), EncodingParseError> {
        // TODO: Verify stack layout
        let (enc, _stack_layout) = self.next().ok_or(EncodingParseError::MissingReceiver)??;
        if !Encoding::Object.equivalent_to_box(&enc) {
            return Err(EncodingParseError::InvalidReceiver(enc));
        }
        Ok(())
    }

    pub(crate) fn verify_sel(&mut self) -> Result<(), EncodingParseError> {
        let (enc, _stack_layout) = self.next().ok_or(EncodingParseError::MissingSel)??;
        if !Encoding::Sel.equivalent_to_box(&enc) {
            return Err(EncodingParseError::InvalidSel(enc));
        }
        Ok(())
    }

    fn extract_encoding(&mut self) -> Result<(EncodingBox, Option<isize>), EncodingParseError> {
        // See also the following other approaches:
        // objrs: https://gitlab.com/objrs/objrs/-/blob/b4f6598696b3fa622e6fddce7aff281770b0a8c2/src/test.rs
        // libobjc2: https://github.com/gnustep/libobjc2/blob/v2.1/encoding2.c
        // objc4: https://github.com/apple-oss-distributions/objc4/blob/objc4-841.13/runtime/objc-typeencoding.mm

        let encoding = EncodingBox::from_start_of_str(&mut self.s)?;
        let stack_layout = parse_stack_layout(&mut self.s)?;

        Ok((encoding, stack_layout))
    }
}

impl Iterator for MethodEncodingIter<'_> {
    type Item = Result<(EncodingBox, Option<isize>), EncodingParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.s.is_empty() {
            return None;
        }
        Some(self.extract_encoding())
    }
}

// TODO: Is `isize` correct here?
fn parse_stack_layout(s: &mut &str) -> Result<Option<isize>, ParseIntError> {
    let rest = s.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '+');
    let stack_layout = &s[..s.len() - rest.len()];
    *s = rest;

    if stack_layout.is_empty() {
        return Ok(None);
    }
    stack_layout.parse().map(Some)
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub(crate) enum EncodingParseError {
    ParseError(ParseError),
    InvalidStackLayoutInteger,
    MissingReturn,
    MissingReceiver,
    MissingSel,
    InvalidReceiver(EncodingBox),
    InvalidSel(EncodingBox),
}

impl From<ParseError> for EncodingParseError {
    fn from(e: ParseError) -> Self {
        Self::ParseError(e)
    }
}

impl From<ParseIntError> for EncodingParseError {
    fn from(_: ParseIntError) -> Self {
        Self::InvalidStackLayoutInteger
    }
}

impl fmt::Display for EncodingParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !matches!(self, Self::ParseError(_)) {
            write!(f, "failed parsing encoding: ")?;
        }

        match self {
            Self::ParseError(e) => write!(f, "{e}")?,
            Self::InvalidStackLayoutInteger => write!(f, "invalid integer for stack layout")?,
            Self::MissingReturn => write!(f, "return type must be present")?,
            Self::MissingReceiver => write!(f, "receiver type must be present")?,
            Self::MissingSel => write!(f, "selector type must be present")?,
            Self::InvalidReceiver(enc) => {
                write!(f, "receiver encoding must be '@', but it was '{enc}'")?;
            }
            Self::InvalidSel(enc) => {
                write!(f, "selector encoding must be '@', but it was '{enc}'")?;
            }
        }
        write!(f, ". This is likely a bug, please report it!")
    }
}

impl Error for EncodingParseError {}

#[cfg(test)]
#[path = "../../../../../tests/native/objc2/unit/runtime/method_encoding_iter.rs"]
mod tests;
