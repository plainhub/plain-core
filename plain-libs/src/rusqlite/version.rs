use crate::rusqlite::ffi;
use std::ffi::CStr;
#[inline]
#[must_use]
pub fn version_number() -> i32 {
    unsafe { ffi::sqlite3_libversion_number() }
}
#[inline]
#[must_use]
pub fn version() -> &'static str {
    let cstr = unsafe { CStr::from_ptr(ffi::sqlite3_libversion()) };
    cstr.to_str()
        .expect("SQLite version string is not valid UTF8 ?!")
}
