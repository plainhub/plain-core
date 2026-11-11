pub mod crc32;
crate::cpu_features::cfg_select! {
_ => { pub (crate) mod allocate ; pub (crate) mod c_api ; pub (crate) mod deflate ; pub (crate) mod inflate ; pub (crate) const MIN_WBITS : i32 = 8 ; pub (crate) const MAX_WBITS : i32 = 15 ; }
}

mod stable;
mod weak_slice;
pub use stable::{Deflate, Inflate, InflateError, Status};
macro_rules! traceln {
    ($ ($ arg : tt) *) => {};
}
pub(crate) use traceln;
macro_rules! trace {
    ($ ($ arg : tt) *) => {};
}
pub(crate) use trace;

pub(crate) const ENOUGH_LENS: usize = 1332;
pub(crate) const ENOUGH_DISTS: usize = 592;
pub(crate) const ADLER32_INITIAL_VALUE: usize = 1;
pub(crate) const CRC32_INITIAL_VALUE: u32 = 0;
pub(crate) const DEF_WBITS: i32 = MAX_WBITS;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeflateFlush {
    #[default]
    NoFlush = 0,
    PartialFlush = 1,
    SyncFlush = 2,
    FullFlush = 3,
    Finish = 4,
    Block = 5,
}
impl TryFrom<i32> for DeflateFlush {
    type Error = ();
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::NoFlush),
            1 => Ok(Self::PartialFlush),
            2 => Ok(Self::SyncFlush),
            3 => Ok(Self::FullFlush),
            4 => Ok(Self::Finish),
            5 => Ok(Self::Block),
            _ => Err(()),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InflateFlush {
    #[default]
    NoFlush = 0,
    SyncFlush = 2,
    Finish = 4,
    Block = 5,
    Trees = 6,
}
impl TryFrom<i32> for InflateFlush {
    type Error = ();
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::NoFlush),
            2 => Ok(Self::SyncFlush),
            4 => Ok(Self::Finish),
            5 => Ok(Self::Block),
            6 => Ok(Self::Trees),
            _ => Err(()),
        }
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Code {
    pub op: u8,
    pub bits: u8,
    pub val: u16,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(i32)]
pub enum ReturnCode {
    Ok = 0,
    StreamEnd = 1,
    NeedDict = 2,
    ErrNo = -1,
    StreamError = -2,
    DataError = -3,
    MemError = -4,
    BufError = -5,
    VersionError = -6,
}
impl From<i32> for ReturnCode {
    fn from(value: i32) -> Self {
        match Self::try_from_c_int(value) {
            Some(value) => value,
            None => panic!("invalid return code {value}"),
        }
    }
}
impl ReturnCode {
    fn error_message_str(self) -> &'static str {
        self.error_message_str_with_null().trim_end_matches('\0')
    }
    const fn error_message_str_with_null(self) -> &'static str {
        match self {
            ReturnCode::Ok => "\0",
            ReturnCode::StreamEnd => "stream end\0",
            ReturnCode::NeedDict => "need dictionary\0",
            ReturnCode::ErrNo => "file error\0",
            ReturnCode::StreamError => "stream error\0",
            ReturnCode::DataError => "data error\0",
            ReturnCode::MemError => "insufficient memory\0",
            ReturnCode::BufError => "buffer error\0",
            ReturnCode::VersionError => "incompatible version\0",
        }
    }
    pub const fn error_message(self) -> *const core::ffi::c_char {
        let msg = self.error_message_str_with_null();
        msg.as_ptr().cast::<core::ffi::c_char>()
    }
    pub const fn try_from_c_int(err: core::ffi::c_int) -> Option<Self> {
        match err {
            0 => Some(Self::Ok),
            1 => Some(Self::StreamEnd),
            2 => Some(Self::NeedDict),
            -1 => Some(Self::ErrNo),
            -2 => Some(Self::StreamError),
            -3 => Some(Self::DataError),
            -4 => Some(Self::MemError),
            -5 => Some(Self::BufError),
            -6 => Some(Self::VersionError),
            _ => None,
        }
    }
}

#[inline]
pub(super) fn as_uninit(output: &mut [u8]) -> &mut [core::mem::MaybeUninit<u8>] {
    // Initialized bytes may be borrowed as uninitialized storage with the same layout and lifetime.
    unsafe { core::slice::from_raw_parts_mut(output.as_mut_ptr().cast(), output.len()) }
}
