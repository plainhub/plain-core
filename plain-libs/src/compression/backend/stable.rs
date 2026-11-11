use crate::compression::backend::ReturnCode;
use crate::compression::backend::deflate::DeflateConfig;
use crate::compression::backend::inflate::InflateConfig;
pub use crate::compression::backend::{DeflateFlush, InflateFlush};
use core::{ffi::c_uint, mem::MaybeUninit};
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Status {
    Ok,
    BufError,
    StreamEnd,
}
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum InflateError {
    NeedDict { dict_id: u32 } = 2,
    StreamError = -2,
    DataError = -3,
    MemError = -4,
}
impl From<InflateError> for ReturnCode {
    fn from(value: InflateError) -> Self {
        match value {
            InflateError::NeedDict { .. } => ReturnCode::NeedDict,
            InflateError::StreamError => ReturnCode::StreamError,
            InflateError::DataError => ReturnCode::DataError,
            InflateError::MemError => ReturnCode::MemError,
        }
    }
}
impl InflateError {
    pub fn as_str(self) -> &'static str {
        ReturnCode::from(self).error_message_str()
    }
}
pub struct Inflate {
    inner: crate::compression::backend::inflate::InflateStream<'static>,
    total_in: u64,
    total_out: u64,
}
impl Inflate {
    pub fn total_in(&self) -> u64 {
        self.total_in
    }
    pub fn total_out(&self) -> u64 {
        self.total_out
    }
    pub fn error_message(&self) -> Option<&'static str> {
        if self.inner.msg.is_null() {
            None
        } else {
            unsafe { core::ffi::CStr::from_ptr(self.inner.msg).to_str() }.ok()
        }
    }
    pub fn new(expect_header: bool, window_bits: u8) -> Self {
        let config = InflateConfig {
            window_bits: if expect_header {
                i32::from(window_bits)
            } else {
                -i32::from(window_bits)
            },
        };
        Self {
            inner: crate::compression::backend::inflate::InflateStream::new(config),
            total_in: 0,
            total_out: 0,
        }
    }
    pub fn reset(&mut self, zlib_header: bool) {
        let mut config = InflateConfig::default();
        if !zlib_header {
            config.window_bits = -config.window_bits;
        }
        self.total_in = 0;
        self.total_out = 0;
        crate::compression::backend::inflate::reset_with_config(&mut self.inner, config);
    }
    pub fn decompress(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: InflateFlush,
    ) -> Result<Status, InflateError> {
        self.decompress_uninit(
            input,
            unsafe { &mut *(output as *mut _ as *mut [MaybeUninit<u8>]) },
            flush,
        )
    }
    pub fn decompress_uninit(
        &mut self,
        input: &[u8],
        output: &mut [MaybeUninit<u8>],
        flush: InflateFlush,
    ) -> Result<Status, InflateError> {
        self.inner.avail_in = Ord::min(input.len(), c_uint::MAX as usize) as c_uint;
        self.inner.avail_out = Ord::min(output.len(), c_uint::MAX as usize) as c_uint;
        self.inner.next_in = input.as_ptr().cast_mut();
        self.inner.next_out = output.as_mut_ptr().cast();
        let start_in = self.inner.next_in;
        let start_out = self.inner.next_out;
        let ret = unsafe { crate::compression::backend::inflate::inflate(&mut self.inner, flush) };
        self.total_in += (self.inner.next_in as usize - start_in as usize) as u64;
        self.total_out += (self.inner.next_out as usize - start_out as usize) as u64;
        match ret {
            ReturnCode::Ok => Ok(Status::Ok),
            ReturnCode::StreamEnd => Ok(Status::StreamEnd),
            ReturnCode::NeedDict => Err(InflateError::NeedDict {
                dict_id: self.inner.adler as u32,
            }),
            ReturnCode::ErrNo => unreachable!("the rust API does not use files"),
            ReturnCode::StreamError => Err(InflateError::StreamError),
            ReturnCode::DataError => Err(InflateError::DataError),
            ReturnCode::MemError => Err(InflateError::MemError),
            ReturnCode::BufError => Ok(Status::BufError),
            ReturnCode::VersionError => unreachable!("the rust API does not use the version"),
        }
    }
}
impl Drop for Inflate {
    fn drop(&mut self) {
        let _ = crate::compression::backend::inflate::end(&mut self.inner);
    }
}
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum DeflateError {
    StreamError = -2,
    DataError = -3,
    MemError = -4,
}
impl From<DeflateError> for ReturnCode {
    fn from(value: DeflateError) -> Self {
        match value {
            DeflateError::StreamError => ReturnCode::StreamError,
            DeflateError::DataError => ReturnCode::DataError,
            DeflateError::MemError => ReturnCode::MemError,
        }
    }
}
impl DeflateError {
    pub fn as_str(self) -> &'static str {
        ReturnCode::from(self).error_message_str()
    }
}
impl From<ReturnCode> for Result<Status, DeflateError> {
    fn from(value: ReturnCode) -> Self {
        match value {
            ReturnCode::Ok => Ok(Status::Ok),
            ReturnCode::StreamEnd => Ok(Status::StreamEnd),
            ReturnCode::NeedDict => unreachable!("compression does not use dictionary"),
            ReturnCode::ErrNo => unreachable!("the rust API does not use files"),
            ReturnCode::StreamError => Err(DeflateError::StreamError),
            ReturnCode::DataError => Err(DeflateError::DataError),
            ReturnCode::MemError => Err(DeflateError::MemError),
            ReturnCode::BufError => Ok(Status::BufError),
            ReturnCode::VersionError => unreachable!("the rust API does not use the version"),
        }
    }
}
pub struct Deflate {
    inner: crate::compression::backend::deflate::DeflateStream<'static>,
    total_in: u64,
    total_out: u64,
}
impl Deflate {
    pub fn total_in(&self) -> u64 {
        self.total_in
    }
    pub fn total_out(&self) -> u64 {
        self.total_out
    }
    pub fn error_message(&self) -> Option<&'static str> {
        if self.inner.msg.is_null() {
            None
        } else {
            unsafe { core::ffi::CStr::from_ptr(self.inner.msg).to_str() }.ok()
        }
    }
    pub fn new(level: i32, zlib_header: bool, window_bits: u8) -> Self {
        let config = DeflateConfig {
            window_bits: if zlib_header {
                i32::from(window_bits)
            } else {
                -i32::from(window_bits)
            },
            level,
            ..DeflateConfig::default()
        };
        Self::new_with_config(config)
    }
    pub fn new_with_config(config: DeflateConfig) -> Self {
        Self {
            inner: crate::compression::backend::deflate::DeflateStream::new(config),
            total_in: 0,
            total_out: 0,
        }
    }
    pub fn reset(&mut self) {
        self.total_in = 0;
        self.total_out = 0;
        crate::compression::backend::deflate::reset(&mut self.inner);
    }
    pub fn compress(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: DeflateFlush,
    ) -> Result<Status, DeflateError> {
        self.compress_uninit(
            input,
            unsafe { &mut *(output as *mut _ as *mut [MaybeUninit<u8>]) },
            flush,
        )
    }
    pub fn compress_uninit(
        &mut self,
        input: &[u8],
        output: &mut [MaybeUninit<u8>],
        flush: DeflateFlush,
    ) -> Result<Status, DeflateError> {
        self.inner.avail_in = Ord::min(input.len(), c_uint::MAX as usize) as c_uint;
        self.inner.avail_out = Ord::min(output.len(), c_uint::MAX as usize) as c_uint;
        self.inner.next_in = input.as_ptr().cast_mut();
        self.inner.next_out = output.as_mut_ptr().cast();
        let start_in = self.inner.next_in;
        let start_out = self.inner.next_out;
        let ret = crate::compression::backend::deflate::deflate(&mut self.inner, flush).into();
        self.total_in += (self.inner.next_in as usize - start_in as usize) as u64;
        self.total_out += (self.inner.next_out as usize - start_out as usize) as u64;
        self.inner.next_in = core::ptr::null_mut();
        self.inner.next_out = core::ptr::null_mut();
        self.inner.avail_in = 0;
        self.inner.avail_out = 0;
        ret
    }
}
impl Drop for Deflate {
    fn drop(&mut self) {
        let _ = crate::compression::backend::deflate::end(&mut self.inner);
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/compression/backend/stable.rs"]
mod tests;
