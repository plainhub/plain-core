use crate::compression::backend::{DeflateFlush, InflateError, InflateFlush};
use core::fmt;
pub const MZ_NO_FLUSH: isize = DeflateFlush::NoFlush as isize;
pub const MZ_PARTIAL_FLUSH: isize = DeflateFlush::PartialFlush as isize;
pub const MZ_SYNC_FLUSH: isize = DeflateFlush::SyncFlush as isize;
pub const MZ_FULL_FLUSH: isize = DeflateFlush::FullFlush as isize;
pub const MZ_FINISH: isize = DeflateFlush::Finish as isize;
pub const MZ_DEFAULT_WINDOW_BITS: core::ffi::c_int = 15;
use crate::compression::mem::{CompressError, DecompressError};
use crate::compression::mem::{compress_failed, decompress_failed};
use crate::compression::{Compression, FlushCompress, FlushDecompress, Status};
use core::mem::MaybeUninit;
impl From<crate::compression::backend::Status> for crate::compression::mem::Status {
    fn from(value: crate::compression::backend::Status) -> Self {
        match value {
            crate::compression::backend::Status::Ok => crate::compression::mem::Status::Ok,
            crate::compression::backend::Status::BufError => {
                crate::compression::mem::Status::BufError
            }
            crate::compression::backend::Status::StreamEnd => {
                crate::compression::mem::Status::StreamEnd
            }
        }
    }
}
#[derive(Clone, Default)]
pub struct ErrorMessage(Option<&'static str>);
impl ErrorMessage {
    pub fn get(&self) -> Option<&str> {
        self.0
    }
}
pub struct Inflate {
    pub(crate) inner: crate::compression::backend::Inflate,
}
impl fmt::Debug for Inflate {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "zlib_rs inflate internal state. total_in: {}, total_out: {}",
            self.total_in(),
            self.total_out(),
        )
    }
}
impl Inflate {
    pub(crate) fn make(zlib_header: bool, window_bits: u8) -> Self {
        Inflate {
            inner: crate::compression::backend::Inflate::new(zlib_header, window_bits),
        }
    }

    pub(crate) fn decompress_uninit(
        &mut self,
        input: &[u8],
        output: &mut [MaybeUninit<u8>],
        flush: FlushDecompress,
    ) -> Result<Status, DecompressError> {
        let flush = match flush {
            FlushDecompress::None => InflateFlush::NoFlush,
            FlushDecompress::Sync => InflateFlush::SyncFlush,
            FlushDecompress::Finish => InflateFlush::Finish,
        };
        let result = self.inner.decompress_uninit(input, output, flush);
        match result {
            Ok(status) => Ok(status.into()),
            Err(InflateError::NeedDict { dict_id }) => {
                crate::compression::mem::decompress_need_dict(dict_id)
            }
            Err(_) => self.decompress_error(),
        }
    }
    pub(crate) fn reset(&mut self, zlib_header: bool) {
        self.inner.reset(zlib_header);
    }
}
impl Inflate {
    #[inline]
    pub(crate) fn total_in(&self) -> u64 {
        self.inner.total_in()
    }
    #[inline]
    pub(crate) fn total_out(&self) -> u64 {
        self.inner.total_out()
    }
}
impl Inflate {
    fn decompress_error<T>(&self) -> Result<T, DecompressError> {
        decompress_failed(ErrorMessage(self.inner.error_message()))
    }
}
pub struct Deflate {
    pub(crate) inner: crate::compression::backend::Deflate,
}
impl fmt::Debug for Deflate {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "zlib_rs deflate internal state. total_in: {}, total_out: {}",
            self.total_in(),
            self.total_out(),
        )
    }
}
impl Deflate {
    pub(crate) fn make(level: Compression, zlib_header: bool, window_bits: u8) -> Self {
        debug_assert!(level.level() <= 9);
        Deflate {
            inner: crate::compression::backend::Deflate::new(
                level.level() as i32,
                zlib_header,
                window_bits,
            ),
        }
    }

    pub(crate) fn compress_uninit(
        &mut self,
        input: &[u8],
        output: &mut [MaybeUninit<u8>],
        flush: FlushCompress,
    ) -> Result<Status, CompressError> {
        let flush = match flush {
            FlushCompress::None => DeflateFlush::NoFlush,
            FlushCompress::Partial => DeflateFlush::PartialFlush,
            FlushCompress::Sync => DeflateFlush::SyncFlush,
            FlushCompress::Full => DeflateFlush::FullFlush,
            FlushCompress::Finish => DeflateFlush::Finish,
        };
        let result = self.inner.compress_uninit(input, output, flush);
        match result {
            Ok(status) => Ok(status.into()),
            Err(_) => self.compress_error(),
        }
    }
    pub(crate) fn reset(&mut self) {
        self.inner.reset();
    }
}
impl Deflate {
    #[inline]
    pub(crate) fn total_in(&self) -> u64 {
        self.inner.total_in()
    }
    #[inline]
    pub(crate) fn total_out(&self) -> u64 {
        self.inner.total_out()
    }
}
impl Deflate {
    fn compress_error<T>(&self) -> Result<T, CompressError> {
        compress_failed(ErrorMessage(self.inner.error_message()))
    }
}

impl fmt::Debug for ErrorMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.get().fmt(f)
    }
}
