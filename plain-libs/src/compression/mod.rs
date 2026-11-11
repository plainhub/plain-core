pub(crate) use crate::compression::crc::Crc;
pub use crate::compression::mem::{Compress, CompressError, Decompress, DecompressError, Status};
pub use crate::compression::mem::{FlushCompress, FlushDecompress};
use std::vec::Vec;
use std::{error, io};
mod bufreader;
mod crc;
mod deflate;
mod ffi;
mod gz;
mod mem;
mod zio;
mod zlib;
pub mod read {
    pub use crate::compression::gz::read::GzDecoder;
    pub use crate::compression::zlib::read::ZlibDecoder;
}
pub mod write {
    pub use crate::compression::deflate::write::DeflateEncoder;
    pub use crate::compression::zlib::write::ZlibEncoder;
}
pub mod bufread {
    pub use crate::compression::deflate::bufread::DeflateDecoder;
}

pub struct Compression(u32);
impl Compression {
    pub const fn new(level: u32) -> Compression {
        Compression(level)
    }
    pub const fn none() -> Compression {
        Compression(0)
    }
    pub const fn fast() -> Compression {
        Compression(1)
    }
    pub const fn best() -> Compression {
        Compression(9)
    }
    pub fn level(&self) -> u32 {
        self.0
    }
}
impl Default for Compression {
    fn default() -> Compression {
        Compression(6)
    }
}

mod backend;

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/compression.rs"]
mod tests;
