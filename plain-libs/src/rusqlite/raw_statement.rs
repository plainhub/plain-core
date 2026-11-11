use super::StatementStatus;
use super::ffi;
use crate::rusqlite::util::ParamIndexCache;
use crate::rusqlite::util::SqliteMallocString;
use std::ffi::{CStr, c_int};
use std::ptr;
use std::sync::Arc;
#[derive(Debug)]
pub struct RawStatement {
    ptr: *mut ffi::sqlite3_stmt,
    cache: ParamIndexCache,
    statement_cache_key: Option<Arc<str>>,
}
impl RawStatement {
    #[inline]
    pub unsafe fn new(stmt: *mut ffi::sqlite3_stmt) -> Self {
        Self {
            ptr: stmt,
            cache: ParamIndexCache::default(),
            statement_cache_key: None,
        }
    }
    #[inline]
    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }
    #[inline]
    pub(crate) fn set_statement_cache_key(&mut self, p: impl Into<Arc<str>>) {
        self.statement_cache_key = Some(p.into());
    }
    #[inline]
    pub(crate) fn statement_cache_key(&self) -> Option<Arc<str>> {
        self.statement_cache_key.clone()
    }
    #[inline]
    pub unsafe fn ptr(&self) -> *mut ffi::sqlite3_stmt {
        self.ptr
    }
    #[inline]
    pub fn column_count(&self) -> usize {
        unsafe { ffi::sqlite3_column_count(self.ptr) as usize }
    }
    #[inline]
    pub fn column_type(&self, idx: usize) -> c_int {
        unsafe { ffi::sqlite3_column_type(self.ptr, idx as c_int) }
    }
    #[inline]
    pub fn column_name(&self, idx: usize) -> Option<&CStr> {
        let idx = idx as c_int;
        if idx < 0 || idx >= self.column_count() as c_int {
            return None;
        }
        unsafe {
            let ptr = ffi::sqlite3_column_name(self.ptr, idx);
            assert!(
                !ptr.is_null(),
                "Null pointer from sqlite3_column_name: Out of memory?"
            );
            Some(CStr::from_ptr(ptr))
        }
    }
    #[inline]
    pub fn step(&self) -> c_int {
        cfg_select! { any () => { use crate::rusqlite::unlock_notify ; let mut db = ptr::null_mut::< ffi::sqlite3 > () ; loop { unsafe { let mut rc = ffi::sqlite3_step (self . ptr) ; if (rc & 0xff) != ffi::SQLITE_LOCKED { break rc ; } if db . is_null () { db = ffi::sqlite3_db_handle (self . ptr) ; } if ! unlock_notify::is_locked (db , rc) { break rc ; } rc = unlock_notify::wait_for_unlock_notify (db) ; if rc != ffi::SQLITE_OK { break rc ; } self . reset () ; } } } _ => { unsafe { ffi::sqlite3_step (self . ptr) } } }
    }
    #[inline]
    pub fn reset(&self) -> c_int {
        unsafe { ffi::sqlite3_reset(self.ptr) }
    }
    #[inline]
    pub fn bind_parameter_count(&self) -> usize {
        unsafe { ffi::sqlite3_bind_parameter_count(self.ptr) as usize }
    }
    #[inline]
    pub fn bind_parameter_index(&self, name: &str) -> Option<usize> {
        self.cache.get_or_insert_with(name, |param_cstr| {
            let r = unsafe { ffi::sqlite3_bind_parameter_index(self.ptr, param_cstr.as_ptr()) };
            match r {
                0 => None,
                i => Some(i as usize),
            }
        })
    }
    #[inline]
    pub fn bind_parameter_name(&self, index: i32) -> Option<&CStr> {
        unsafe {
            let name = ffi::sqlite3_bind_parameter_name(self.ptr, index);
            if name.is_null() {
                None
            } else {
                Some(CStr::from_ptr(name))
            }
        }
    }
    #[inline]
    pub fn clear_bindings(&mut self) {
        unsafe {
            ffi::sqlite3_clear_bindings(self.ptr);
        }
    }
    #[inline]
    pub fn sql(&self) -> Option<&CStr> {
        if self.ptr.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(ffi::sqlite3_sql(self.ptr)) })
        }
    }
    #[inline]
    pub fn finalize(mut self) -> c_int {
        self.finalize_()
    }
    #[inline]
    fn finalize_(&mut self) -> c_int {
        let r = unsafe { ffi::sqlite3_finalize(self.ptr) };
        self.ptr = ptr::null_mut();
        r
    }
    #[inline]
    pub fn readonly(&self) -> bool {
        unsafe { ffi::sqlite3_stmt_readonly(self.ptr) != 0 }
    }
    #[inline]
    pub(crate) fn expanded_sql(&self) -> Option<SqliteMallocString> {
        unsafe { expanded_sql(self.ptr) }
    }
    #[inline]
    pub fn get_status(&self, status: StatementStatus, reset: bool) -> i32 {
        unsafe { stmt_status(self.ptr, status, reset) }
    }
    #[inline]
    pub fn is_explain(&self) -> i32 {
        unsafe { ffi::sqlite3_stmt_isexplain(self.ptr) }
    }
}
#[inline]
pub(crate) unsafe fn expanded_sql(ptr: *mut ffi::sqlite3_stmt) -> Option<SqliteMallocString> {
    unsafe { unsafe { SqliteMallocString::from_raw(ffi::sqlite3_expanded_sql(ptr)) } }
}
#[inline]
pub(crate) unsafe fn stmt_status(
    ptr: *mut ffi::sqlite3_stmt,
    status: StatementStatus,
    reset: bool,
) -> i32 {
    unsafe {
        assert!(!ptr.is_null());
        unsafe { ffi::sqlite3_stmt_status(ptr, status as c_int, c_int::from(reset)) }
    }
}
impl Drop for RawStatement {
    fn drop(&mut self) {
        self.finalize_();
    }
}
