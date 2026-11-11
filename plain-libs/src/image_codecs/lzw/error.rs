#[must_use = "Contains a status with potential error information"]
#[derive(Debug)]
pub struct BufferResult {
    pub consumed_in: usize,
    pub consumed_out: usize,
    pub status: Result<LzwStatus, LzwError>,
}
#[must_use = "Contains a status with potential error information"]
#[derive(Debug)]
pub struct VectorResult {
    pub consumed_in: usize,
    pub consumed_out: usize,
    pub status: Result<LzwStatus, LzwError>,
}
#[must_use = "Contains a status with potential error information"]
#[derive(Debug)]
pub struct StreamResult {
    pub bytes_read: usize,
    pub bytes_written: usize,
    pub status: std::io::Result<()>,
}
#[derive(Debug, Clone, Copy)]
pub enum LzwStatus {
    Ok,
    NoProgress,
    Done,
}
#[derive(Debug, Clone, Copy)]
pub enum LzwError {
    InvalidCode,
}
impl core::fmt::Display for LzwError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            LzwError::InvalidCode => f.write_str("invalid code in LZW stream"),
        }
    }
}
impl std::error::Error for LzwError {}
