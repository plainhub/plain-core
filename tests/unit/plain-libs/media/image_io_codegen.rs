extern crate plain_libs;
use std::path::Path;
use plain_libs::serde_json::Value;
use plain_libs::utils::source_reader::SourceReader;
#[unsafe(no_mangle)]
pub fn thumbnail(path: String, width: u32, height: u32) -> Result<Option<Vec<u8>>, String> {
    THUMBNAIL_BODY
}
#[unsafe(no_mangle)]
pub fn convert(path: String, output: String) -> Result<Value, String> {
    CONVERSION_BODY
}
#[unsafe(no_mangle)]
pub fn file_dimensions(reader: std::fs::File) -> Option<(u32, u32)> {
    FILE_DIMENSIONS_BODY
}
#[unsafe(no_mangle)]
pub fn source_dimensions(reader: &mut SourceReader) -> Option<(u32, u32)> {
    SOURCE_DIMENSIONS_EXPRESSION
}
