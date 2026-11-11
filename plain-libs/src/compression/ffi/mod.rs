use crate::compression::Compression;
use crate::compression::mem::{
    CompressError, DecompressError, FlushCompress, FlushDecompress, Status,
};
mod zlib_rs;
pub use self::zlib_rs::*;
impl core::fmt::Debug for ErrorMessage {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        self.get().fmt(f)
    }
}

fn initialize_buffer(output: &mut [core::mem::MaybeUninit<u8>]) -> &mut [u8] {
    unsafe {
        output.as_mut_ptr().write_bytes(0, output.len());
        core::slice::from_raw_parts_mut(output.as_mut_ptr().cast(), output.len())
    }
}
