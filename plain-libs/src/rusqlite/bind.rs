use crate::rusqlite::{Error, Result, Statement, ffi};
use std::ffi::CStr;
mod sealed {
    use std::ffi::CStr;
    pub trait Sealed {}
    impl Sealed for usize {}
    impl Sealed for &str {}
    impl Sealed for &CStr {}
}
pub trait BindIndex: sealed::Sealed + Copy {
    fn idx(&self, stmt: &Statement<'_>) -> Result<usize>;
}
impl BindIndex for usize {
    #[inline]
    fn idx(&self, _: &Statement<'_>) -> Result<usize> {
        Ok(*self)
    }
}
impl BindIndex for &'_ str {
    fn idx(&self, stmt: &Statement<'_>) -> Result<usize> {
        match stmt.parameter_index(self)? {
            Some(idx) => Ok(idx),
            None => Err(Error::InvalidParameterName(self.to_string())),
        }
    }
}
impl BindIndex for &CStr {
    fn idx(&self, stmt: &Statement<'_>) -> Result<usize> {
        let r = unsafe { ffi::sqlite3_bind_parameter_index(stmt.ptr(), self.as_ptr()) };
        match r {
            0 => Err(Error::InvalidParameterName(
                self.to_string_lossy().to_string(),
            )),
            i => Ok(i as usize),
        }
    }
}
