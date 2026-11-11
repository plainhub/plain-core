pub use crate::zip::compression::{CompressionMethod, SUPPORTED_COMPRESSION_METHODS};
pub use crate::zip::datetime::DateTime;
pub use crate::zip::format::aes::AesMode;
pub use crate::zip::format::system::System;
pub use crate::zip::format::{ZIP64_BYTES_THR, ZIP64_ENTRY_THR};
pub use crate::zip::read::HasZipMetadata;
pub use crate::zip::read::{ZipArchive, ZipReadOptions};
pub use crate::zip::write::ZipWriter;
mod compression;
mod cp437;
mod crc32;
mod datetime;
pub mod extra_fields;
mod format;
mod path;
pub mod read;
pub mod result;
mod types;
pub mod write;
mod zipcrypto;
pub use extra_fields::ExtraField;
pub mod unstable;

mod windows_path;

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/zip.rs"]
mod tests;
