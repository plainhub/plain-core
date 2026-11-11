use crate::rusqlite::ffi;
use crate::rusqlite::{Connection, Error, Name, Result, Statement};
use std::ffi::{CStr, c_char};
use std::ptr;
use std::str;
impl Statement<'_> {
    pub fn column_names(&self) -> impl DoubleEndedIterator<Item = &str> + ExactSizeIterator {
        ColumnsIter::new(self, Statement::column_name_unwrap)
    }
    #[inline]
    pub fn column_count(&self) -> usize {
        self.stmt.column_count()
    }
    #[inline]
    pub(super) fn column_name_unwrap(&self, col: usize) -> &str {
        self.column_name(col).expect("Column out of bounds")
    }
    #[inline]
    pub fn column_name(&self, col: usize) -> Result<&str> {
        self.stmt
            .column_name(col)
            .ok_or(Error::InvalidColumnIndex(col))
            .map(|slice| {
                slice
                    .to_str()
                    .expect("Invalid UTF-8 sequence in column name")
            })
    }
    #[inline]
    pub fn column_index(&self, name: &str) -> Result<usize> {
        let bytes = name.as_bytes();
        let n = self.column_count();
        for i in 0..n {
            if bytes.eq_ignore_ascii_case(self.stmt.column_name(i).unwrap().to_bytes()) {
                return Ok(i);
            }
        }
        Err(Error::InvalidColumnName(String::from(name)))
    }
}
impl Connection {
    pub fn table_exists<N: Name>(&self, db_name: Option<N>, table_name: N) -> Result<bool> {
        self.exists(db_name, table_name, None)
    }

    fn exists<N: Name>(
        &self,
        db_name: Option<N>,
        table_name: N,
        column_name: Option<N>,
    ) -> Result<bool> {
        let cs = db_name.as_ref().map(N::as_cstr).transpose()?;
        let db_name = cs.as_ref().map_or(ptr::null(), |s| s.as_ptr());
        let table_name = table_name.as_cstr()?;
        let cn = column_name.as_ref().map(N::as_cstr).transpose()?;
        let column_name = cn.as_ref().map_or(ptr::null(), |s| s.as_ptr());
        let r = unsafe {
            ffi::sqlite3_table_column_metadata(
                self.handle(),
                db_name,
                table_name.as_ptr(),
                column_name,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        match r {
            ffi::SQLITE_OK => Ok(true),
            ffi::SQLITE_ERROR => Ok(false),
            _ => self.db.borrow().decode_result(r).map(|()| false),
        }
    }
}
struct ColumnsIter<'s, T> {
    stmt: &'s Statement<'s>,
    range: std::ops::Range<usize>,
    map: fn(stmt: &'s Statement<'s>, col: usize) -> T,
}
impl<'s, T> ColumnsIter<'s, T> {
    fn new(stmt: &'s Statement<'s>, map: fn(stmt: &'s Statement<'s>, col: usize) -> T) -> Self {
        Self {
            stmt,
            range: 0..stmt.column_count(),
            map,
        }
    }
}
impl<T> Iterator for ColumnsIter<'_, T> {
    type Item = T;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        Some((self.map)(self.stmt, self.range.next()?))
    }
    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        Some((self.map)(self.stmt, self.range.nth(n)?))
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}
impl<T> DoubleEndedIterator for ColumnsIter<'_, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        Some((self.map)(self.stmt, self.range.next_back()?))
    }
    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        Some((self.map)(self.stmt, self.range.nth_back(n)?))
    }
}
impl<T> ExactSizeIterator for ColumnsIter<'_, T> {}
