use crate::compression::backend::allocate::Allocator;
use core::ffi::{c_char, c_int, c_uint, c_ulong, c_void};
pub type AllocFn = unsafe extern "C" fn(VoidPtr, Uint, Uint) -> VoidPtr;
pub type FreeFn = unsafe extern "C" fn(VoidPtr, VoidPtr);
pub type Bytef = u8;
pub type Uint = c_uint;
pub type Ulong = c_ulong;
pub type VoidPtr = *mut c_void;
#[repr(C)]
#[derive(Copy, Clone)]
pub struct ZStream {
    pub next_in: *const Bytef,
    pub avail_in: Uint,
    pub total_in: ZSize,
    pub next_out: *mut Bytef,
    pub avail_out: Uint,
    pub total_out: ZSize,
    pub msg: *mut c_char,
    pub state: *mut InternalState,
    pub zalloc: Option<AllocFn>,
    pub zfree: Option<FreeFn>,
    pub opaque: VoidPtr,
    pub data_type: c_int,
    pub adler: ZChecksum,
    pub reserved: Ulong,
}
impl Default for ZStream {
    fn default() -> Self {
        let mut stream = Self {
            next_in: core::ptr::null_mut(),
            avail_in: 0,
            total_in: 0,
            next_out: core::ptr::null_mut(),
            avail_out: 0,
            total_out: 0,
            msg: core::ptr::null_mut(),
            state: core::ptr::null_mut(),
            zalloc: None,
            zfree: None,
            opaque: core::ptr::null_mut(),
            data_type: 0,
            adler: 0,
            reserved: 0,
        };
        if stream.zalloc.is_none() || stream.zfree.is_none() {
            stream.configure_default_rust_allocator()
        }
        stream
    }
}
impl ZStream {
    fn configure_allocator(&mut self, alloc: Allocator) {
        self.zalloc = Some(alloc.zalloc);
        self.zfree = Some(alloc.zfree);
        self.opaque = alloc.opaque;
    }
    pub fn configure_default_rust_allocator(&mut self) {
        self.configure_allocator(crate::compression::backend::allocate::RUST)
    }
}
pub(crate) type ZSize = c_ulong;
pub(crate) type ZChecksum = c_ulong;
pub enum InternalState {}
pub const Z_DEFAULT_COMPRESSION: c_int = -1;
pub const Z_DEFLATED: c_int = 8;
pub const Z_UNKNOWN: c_int = 2;
#[derive(Debug)]
#[repr(C)]
pub struct GzHeader {
    pub text: i32,
    pub time: c_ulong,
    pub xflags: i32,
    pub os: i32,
    pub extra: *mut u8,
    pub extra_len: u32,
    pub extra_max: u32,
    pub name: *mut u8,
    pub name_max: u32,
    pub comment: *mut u8,
    pub comm_max: u32,
    pub hcrc: i32,
    pub done: i32,
}
impl Default for GzHeader {
    fn default() -> Self {
        Self {
            text: 0,
            time: 0,
            xflags: 0,
            os: 0,
            extra: core::ptr::null_mut(),
            extra_len: 0,
            extra_max: 0,
            name: core::ptr::null_mut(),
            name_max: 0,
            comment: core::ptr::null_mut(),
            comm_max: 0,
            hcrc: 0,
            done: 0,
        }
    }
}
impl GzHeader {
    pub const OS_CODE: u8 = {
        if cfg!(windows) {
            10
        } else if cfg!(target_os = "macos") {
            19
        } else if cfg!(unix) {
            3
        } else {
            3
        }
    };
    pub(crate) fn flags(&self) -> u8 {
        (if self.text != 0 { 1 } else { 0 })
            + (if self.hcrc != 0 { 2 } else { 0 })
            + (if self.extra.is_null() { 0 } else { 4 })
            + (if self.name.is_null() { 0 } else { 8 })
            + (if self.comment.is_null() { 0 } else { 16 })
    }
}
