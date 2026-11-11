#![forbid(unsafe_code)]
pub mod de;
pub mod ser;
pub use crate::serde_urlencoded::de::{Deserializer, from_bytes, from_reader, from_str};
pub use crate::serde_urlencoded::ser::{Serializer, to_string};

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/serde_urlencoded.rs"]
mod tests;
