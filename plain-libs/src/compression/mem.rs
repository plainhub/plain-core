use crate::compression::Compression;
use crate::compression::error::Error;
use crate::compression::ffi::{self, Deflate, ErrorMessage, Inflate};
use crate::compression::io;
use core::fmt;
use core::mem::MaybeUninit;
#[derive(Debug)]
pub struct Compress {
    inner: Deflate,
}
#[derive(Debug)]
pub struct Decompress {
    inner: Inflate,
}
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum FlushCompress {
    None = ffi::MZ_NO_FLUSH as isize,
    Partial = ffi::MZ_PARTIAL_FLUSH as isize,
    Sync = ffi::MZ_SYNC_FLUSH as isize,
    Full = ffi::MZ_FULL_FLUSH as isize,
    Finish = ffi::MZ_FINISH as isize,
}
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum FlushDecompress {
    None = ffi::MZ_NO_FLUSH as isize,
    Sync = ffi::MZ_SYNC_FLUSH as isize,
    Finish = ffi::MZ_FINISH as isize,
}
#[derive(Clone, Debug)]
pub(crate) enum DecompressErrorInner {
    General { msg: ErrorMessage },
    NeedsDictionary(u32),
}
#[derive(Clone, Debug)]
pub struct DecompressError(pub(crate) DecompressErrorInner);
impl DecompressError {
    pub fn needs_dictionary(&self) -> Option<u32> {
        match self.0 {
            DecompressErrorInner::NeedsDictionary(adler) => Some(adler),
            _ => None,
        }
    }
}
#[inline]
pub(crate) fn decompress_failed<T>(msg: ErrorMessage) -> Result<T, DecompressError> {
    Err(DecompressError(DecompressErrorInner::General { msg }))
}
#[inline]
pub(crate) fn decompress_need_dict<T>(adler: u32) -> Result<T, DecompressError> {
    Err(DecompressError(DecompressErrorInner::NeedsDictionary(
        adler,
    )))
}
#[derive(Clone, Debug)]
pub struct CompressError {
    pub(crate) msg: ErrorMessage,
}
#[inline]
pub(crate) fn compress_failed<T>(msg: ErrorMessage) -> Result<T, CompressError> {
    Err(CompressError { msg })
}
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Status {
    Ok,
    BufError,
    StreamEnd,
}
impl Compress {
    pub fn new(level: Compression, zlib_header: bool) -> Compress {
        Compress {
            inner: Deflate::make(level, zlib_header, ffi::MZ_DEFAULT_WINDOW_BITS as u8),
        }
    }

    pub fn total_in(&self) -> u64 {
        self.inner.total_in()
    }
    pub fn total_out(&self) -> u64 {
        self.inner.total_out()
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }

    pub fn compress(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: FlushCompress,
    ) -> Result<Status, CompressError> {
        self.inner.compress(input, output, flush)
    }
}
impl Decompress {
    pub fn new(zlib_header: bool) -> Decompress {
        Decompress {
            inner: Inflate::make(zlib_header, ffi::MZ_DEFAULT_WINDOW_BITS as u8),
        }
    }

    pub fn total_in(&self) -> u64 {
        self.inner.total_in()
    }
    pub fn total_out(&self) -> u64 {
        self.inner.total_out()
    }
    pub fn decompress(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: FlushDecompress,
    ) -> Result<Status, DecompressError> {
        self.inner.decompress(input, output, flush)
    }

    pub fn reset(&mut self, zlib_header: bool) {
        self.inner.reset(zlib_header);
    }
}
impl Error for DecompressError {}
impl DecompressError {
    pub fn message(&self) -> Option<&str> {
        match &self.0 {
            DecompressErrorInner::General { msg } => msg.get(),
            _ => None,
        }
    }
}
impl From<DecompressError> for io::Error {
    fn from(data: DecompressError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, data)
    }
}
impl fmt::Display for DecompressError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match &self.0 {
            DecompressErrorInner::General { msg } => msg.get(),
            DecompressErrorInner::NeedsDictionary { .. } => Some("requires a dictionary"),
        };
        match msg {
            Some(msg) => write!(f, "deflate decompression error: {msg}"),
            None => write!(f, "deflate decompression error"),
        }
    }
}
impl Error for CompressError {}
impl CompressError {
    pub fn message(&self) -> Option<&str> {
        self.msg.get()
    }
}
impl From<CompressError> for io::Error {
    fn from(data: CompressError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, data)
    }
}
impl fmt::Display for CompressError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.msg.get() {
            Some(msg) => write!(f, "deflate compression error: {msg}"),
            None => write!(f, "deflate compression error"),
        }
    }
}
unsafe fn write_to_spare_capacity_of_vec<T>(
    output: &mut Vec<u8>,
    writer: impl FnOnce(&mut [MaybeUninit<u8>]) -> (usize, T),
) -> T {
    unsafe {
        let cap = output.capacity();
        let len = output.len();
        let (bytes_written, ret) = writer(output.spare_capacity_mut());
        output.set_len(cap.min(len + bytes_written));
        ret
    }
}

impl Compress {
    pub fn compress_uninit(
        &mut self,
        input: &[u8],
        output: &mut [MaybeUninit<u8>],
        flush: FlushCompress,
    ) -> Result<Status, CompressError> {
        self.inner.compress_uninit(input, output, flush)
    }
    pub fn compress_vec(
        &mut self,
        input: &[u8],
        output: &mut Vec<u8>,
        flush: FlushCompress,
    ) -> Result<Status, CompressError> {
        unsafe {
            write_to_spare_capacity_of_vec(output, |out| {
                let before = self.total_out();
                let ret = self.compress_uninit(input, out, flush);
                let bytes_written = self.total_out() - before;
                (bytes_written as usize, ret)
            })
        }
    }
}

impl Decompress {
    pub fn decompress_uninit(
        &mut self,
        input: &[u8],
        output: &mut [MaybeUninit<u8>],
        flush: FlushDecompress,
    ) -> Result<Status, DecompressError> {
        self.inner.decompress_uninit(input, output, flush)
    }
    pub fn decompress_vec(
        &mut self,
        input: &[u8],
        output: &mut Vec<u8>,
        flush: FlushDecompress,
    ) -> Result<Status, DecompressError> {
        unsafe {
            write_to_spare_capacity_of_vec(output, |out| {
                let before = self.total_out();
                let ret = self.decompress_uninit(input, out, flush);
                let bytes_written = self.total_out() - before;
                (bytes_written as usize, ret)
            })
        }
    }
}
