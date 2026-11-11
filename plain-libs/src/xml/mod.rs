// Adapted and pruned from the upstream source identified in SOURCE.json.
#![forbid(unsafe_code)]
pub mod encoding;
pub mod errors;
pub mod escape;
pub mod events;
pub mod name;
pub mod parser;
pub mod reader;
pub mod utils;
pub use crate::xml::errors::{Error, Result};
pub use crate::xml::reader::{NsReader, Reader};
use std::borrow::Cow;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XmlVersion {
    Implicit1_0,
    Explicit1_0,
    Explicit1_1,
}
impl XmlVersion {
    pub(crate) fn normalize_attribute_value<'input, 'entity, F>(
        &self,
        value: &'input str,
        depth: usize,
        resolve_entity: F,
    ) -> std::result::Result<Cow<'input, str>, escape::EscapeError>
    where
        F: FnMut(&str) -> Option<&'entity str>,
    {
        match self {
            Self::Explicit1_1 => {
                escape::normalize_xml11_attribute_value(value, depth, resolve_entity)
            }
            _ => escape::normalize_xml10_attribute_value(value, depth, resolve_entity),
        }
    }
}
impl Default for XmlVersion {
    #[inline]
    fn default() -> Self {
        Self::Implicit1_0
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/xml.rs"]
mod tests;
