#![forbid(unsafe_code)]
#![forbid(missing_docs)]
pub(crate) const MAX_CODESIZE: u8 = 12;
pub(crate) const MAX_ENTRIES: usize = 1 << MAX_CODESIZE as usize;
pub(crate) type Code = u16;
pub(crate) const STREAM_BUF_SIZE: usize = 1 << 24;
#[derive(Clone, Copy, Debug)]
pub enum BitOrder {
    Msb,
    Lsb,
}
pub(crate) enum StreamBuf<'d> {
    Borrowed(&'d mut [u8]),
    Owned(std::vec::Vec<u8>),
}
#[cold]
fn assert_decode_size(size: u8) {
    assert!(
        size <= MAX_CODESIZE,
        "Maximum code size 12 required, got {}",
        size
    );
}
#[cold]
fn assert_encode_size(size: u8) {
    assert!(size >= 2, "Minimum code size 2 required, got {}", size);
    assert!(
        size <= MAX_CODESIZE,
        "Maximum code size 12 required, got {}",
        size
    );
}
pub mod decode;
pub mod encode;
mod error;
pub use self::error::StreamResult;
pub use self::error::{BufferResult, LzwError, LzwStatus};
