pub(crate) mod param_cache;
mod small_cstr;
pub(crate) use param_cache::ParamIndexCache;
pub(crate) use small_cstr::SmallCString;
mod sqlite_string;
pub(crate) use sqlite_string::{SqliteMallocString, alloc};
pub(crate) unsafe extern "C" fn free_boxed_value<T>(p: *mut std::ffi::c_void) {
    unsafe {
        drop(unsafe { Box::from_raw(p.cast::<T>()) });
    }
}
pub(crate) unsafe fn from_raw_parts<'a, T>(data: *const T, len: std::ffi::c_int) -> &'a [T] {
    unsafe {
        if len > 0 {
            return unsafe { std::slice::from_raw_parts(data, len as usize) };
        }
        &[]
    }
}
use crate::rusqlite::Result;
use std::ffi::CStr;
pub enum Named<'a> {
    Small(SmallCString),
    C(&'a CStr),
}
impl std::ops::Deref for Named<'_> {
    type Target = CStr;
    #[inline]
    fn deref(&self) -> &CStr {
        match self {
            Named::Small(s) => s.as_cstr(),
            Named::C(s) => s,
        }
    }
}
pub trait Name: std::fmt::Debug + Copy {
    fn as_cstr(&self) -> Result<Named<'_>>;
}
impl Name for &str {
    fn as_cstr(&self) -> Result<Named<'_>> {
        let ss = SmallCString::new(self)?;
        Ok(Named::Small(ss))
    }
}
impl Name for &CStr {
    #[inline]
    fn as_cstr(&self) -> Result<Named<'_>> {
        Ok(Named::C(self))
    }
}
