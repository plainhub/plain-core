use super::ffi::{self, sqlite3_stmt};
use super::str_for_sqlite;
use super::{
    AndThenRows, Connection, Error, MappedRows, Params, RawStatement, Result, Row, Rows, ValueRef,
};
use crate::rusqlite::bind::BindIndex;
use crate::rusqlite::types::{ToSql, ToSqlOutput, Value};
use crate::rusqlite::util::from_raw_parts;
use std::ffi::{c_int, c_void};
use std::{fmt, mem, ptr, str};
pub struct Statement<'conn> {
    pub(crate) conn: &'conn Connection,
    pub(crate) stmt: RawStatement,
}
impl Statement<'_> {
    #[inline]
    pub fn execute<P: Params>(&mut self, params: P) -> Result<usize> {
        params.__bind_in(self)?;
        self.execute_with_bound_parameters()
    }
    #[inline]
    pub fn insert<P: Params>(&mut self, params: P) -> Result<i64> {
        let changes = self.execute(params)?;
        match changes {
            1 => Ok(self.conn.last_insert_rowid()),
            _ => Err(Error::StatementChangedRows(changes)),
        }
    }
    #[inline]
    pub fn query<P: Params>(&mut self, params: P) -> Result<Rows<'_>> {
        params.__bind_in(self)?;
        Ok(Rows::new(self))
    }
    pub fn query_map<T, P, F>(&mut self, params: P, f: F) -> Result<MappedRows<'_, F>>
    where
        P: Params,
        F: FnMut(&Row<'_>) -> Result<T>,
    {
        self.query(params).map(|rows| rows.mapped(f))
    }

    #[inline]
    pub fn exists<P: Params>(&mut self, params: P) -> Result<bool> {
        let mut rows = self.query(params)?;
        let exists = rows.next()?.is_some();
        Ok(exists)
    }
    pub fn query_row<T, P, F>(&mut self, params: P, f: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&Row<'_>) -> Result<T>,
    {
        let mut rows = self.query(params)?;
        rows.get_expected_row().and_then(f)
    }
    pub fn query_one<T, P, F>(&mut self, params: P, f: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&Row<'_>) -> Result<T>,
    {
        let mut rows = self.query(params)?;
        let row = rows.get_expected_row().and_then(f)?;
        if rows.next()?.is_some() {
            return Err(Error::QueryReturnedMoreThanOneRow);
        }
        Ok(row)
    }
    #[inline]
    pub fn finalize(mut self) -> Result<()> {
        self.finalize_()
    }
    #[inline]
    pub fn parameter_index(&self, name: &str) -> Result<Option<usize>> {
        Ok(self.stmt.bind_parameter_index(name))
    }

    #[inline]
    pub(crate) fn bind_parameters<P>(&mut self, params: P) -> Result<()>
    where
        P: IntoIterator,
        P::Item: ToSql,
    {
        let expected = self.stmt.bind_parameter_count();
        let mut index = 0;
        for p in params {
            index += 1;
            if index > expected {
                break;
            }
            self.bind_parameter(&p, index)?;
        }
        if index == expected {
            Ok(())
        } else {
            Err(Error::InvalidParameterCount(index, expected))
        }
    }
    #[inline]
    pub(crate) fn ensure_parameter_count(&self, n: usize) -> Result<()> {
        let count = self.parameter_count();
        if count == n {
            Ok(())
        } else {
            Err(Error::InvalidParameterCount(n, count))
        }
    }
    #[inline]
    pub(crate) fn bind_parameters_named<S: BindIndex, T: ToSql>(
        &mut self,
        params: &[(S, T)],
    ) -> Result<()> {
        for (name, value) in params {
            let i = name.idx(self)?;
            let ts: &dyn ToSql = &value;
            self.bind_parameter(ts, i)?;
        }
        Ok(())
    }
    #[inline]
    pub fn parameter_count(&self) -> usize {
        self.stmt.bind_parameter_count()
    }
    #[inline]
    pub fn raw_bind_parameter<I: BindIndex, T: ToSql>(
        &mut self,
        one_based_index: I,
        param: T,
    ) -> Result<()> {
        self.bind_parameter(&param, one_based_index.idx(self)?)
    }

    #[inline]
    pub fn raw_query(&mut self) -> Rows<'_> {
        Rows::new(self)
    }
    fn bind_parameter<P: ?Sized + ToSql>(&self, param: &P, ndx: usize) -> Result<()> {
        let value = param.to_sql()?;
        let ptr = unsafe { self.stmt.ptr() };
        self . conn . decode_result (match value { ToSqlOutput::Borrowed (ValueRef::Null) | ToSqlOutput::Owned (Value::Null) => { unsafe { ffi::sqlite3_bind_null (ptr , ndx as c_int) } } , ToSqlOutput::Borrowed (ValueRef::Integer (i)) | ToSqlOutput::Owned (Value::Integer (i)) => { unsafe { ffi::sqlite3_bind_int64 (ptr , ndx as c_int , i) } } , ToSqlOutput::Borrowed (ValueRef::Real (r)) | ToSqlOutput::Owned (Value::Real (r)) => { unsafe { ffi::sqlite3_bind_double (ptr , ndx as c_int , r) } } , ToSqlOutput::Borrowed (ValueRef::Text (s)) => { Self::bind_text (ptr , ndx , s) } , ToSqlOutput::Owned (Value::Text (s)) => { Self::bind_text (ptr , ndx , s . as_bytes ()) } , ToSqlOutput::Borrowed (ValueRef::Blob (b)) => { Self::bind_blob (ptr , ndx , b) } , ToSqlOutput::Owned (Value::Blob (b)) => { Self::bind_blob (ptr , ndx , b . as_slice ()) } , })
    }
    fn bind_text(stmt: *mut sqlite3_stmt, ndx: usize, s: &[u8]) -> c_int {
        unsafe {
            let (c_str, len, destructor) = str_for_sqlite(s);
            ffi::sqlite3_bind_text64(
                stmt,
                ndx as c_int,
                c_str,
                len,
                destructor,
                ffi::SQLITE_UTF8 as _,
            )
        }
    }
    fn bind_blob(stmt: *mut sqlite3_stmt, ndx: usize, b: &[u8]) -> c_int {
        unsafe {
            let length = b.len();
            if length == 0 {
                ffi::sqlite3_bind_zeroblob64(stmt, ndx as c_int, 0)
            } else {
                ffi::sqlite3_bind_blob64(
                    stmt,
                    ndx as c_int,
                    b.as_ptr().cast::<c_void>(),
                    length as ffi::sqlite3_uint64,
                    ffi::SQLITE_TRANSIENT(),
                )
            }
        }
    }
    #[inline]
    fn execute_with_bound_parameters(&mut self) -> Result<usize> {
        self.check_update()?;
        let r = self.stmt.step();
        let rr = self.stmt.reset();
        match r {
            ffi::SQLITE_DONE => match rr {
                ffi::SQLITE_OK => Ok(self.conn.changes() as usize),
                _ => Err(self.conn.decode_result(rr).unwrap_err()),
            },
            ffi::SQLITE_ROW => Err(Error::ExecuteReturnedResults),
            _ => Err(self.conn.decode_result(r).unwrap_err()),
        }
    }
    #[inline]
    fn finalize_(&mut self) -> Result<()> {
        let mut stmt = unsafe { RawStatement::new(ptr::null_mut()) };
        mem::swap(&mut stmt, &mut self.stmt);
        self.conn.decode_result(stmt.finalize())
    }
    #[inline]
    fn check_update(&self) -> Result<()> {
        cfg_select! { any () => { if self . column_count () > 0 && self . stmt . readonly () { return Err (Error::ExecuteReturnedResults) ; } } _ => { } }
        Ok(())
    }
    pub fn expanded_sql(&self) -> Option<String> {
        self.stmt
            .expanded_sql()
            .map(|s| s.to_string_lossy().to_string())
    }
    #[inline]
    pub fn get_status(&self, status: StatementStatus) -> i32 {
        self.stmt.get_status(status, false)
    }

    #[inline]
    pub fn is_explain(&self) -> i32 {
        self.stmt.is_explain()
    }
    #[inline]
    pub fn readonly(&self) -> bool {
        self.stmt.readonly()
    }
    #[inline]
    pub(crate) unsafe fn into_raw(mut self) -> RawStatement {
        unsafe {
            let mut stmt = RawStatement::new(ptr::null_mut());
            mem::swap(&mut stmt, &mut self.stmt);
            stmt
        }
    }
    pub fn clear_bindings(&mut self) {
        self.stmt.clear_bindings();
    }
    pub(crate) unsafe fn ptr(&self) -> *mut ffi::sqlite3_stmt {
        unsafe { self.stmt.ptr() }
    }
}
impl fmt::Debug for Statement<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sql = if self.stmt.is_null() {
            Ok("")
        } else {
            self.stmt.sql().unwrap().to_str()
        };
        f.debug_struct("Statement")
            .field("conn", self.conn)
            .field("stmt", &self.stmt)
            .field("sql", &sql)
            .finish()
    }
}
impl Drop for Statement<'_> {
    #[inline]
    fn drop(&mut self) {
        self.finalize_();
    }
}
impl Statement<'_> {
    #[inline]
    pub(super) fn new(conn: &Connection, stmt: RawStatement) -> Statement<'_> {
        Statement { conn, stmt }
    }
    pub(super) fn value_ref(&self, col: usize) -> ValueRef<'_> {
        let raw = unsafe { self.stmt.ptr() };
        match self.stmt.column_type(col) {
            ffi::SQLITE_NULL => ValueRef::Null,
            ffi::SQLITE_INTEGER => {
                ValueRef::Integer(unsafe { ffi::sqlite3_column_int64(raw, col as c_int) })
            }
            ffi::SQLITE_FLOAT => {
                ValueRef::Real(unsafe { ffi::sqlite3_column_double(raw, col as c_int) })
            }
            ffi::SQLITE_TEXT => {
                let s = unsafe {
                    let text = ffi::sqlite3_column_text(raw, col as c_int);
                    let len = ffi::sqlite3_column_bytes(raw, col as c_int);
                    assert!(
                        !text.is_null(),
                        "unexpected SQLITE_TEXT column type with NULL data"
                    );
                    from_raw_parts(text.cast::<u8>(), len)
                };
                ValueRef::Text(s)
            }
            ffi::SQLITE_BLOB => {
                let (blob, len) = unsafe {
                    (
                        ffi::sqlite3_column_blob(raw, col as c_int),
                        ffi::sqlite3_column_bytes(raw, col as c_int),
                    )
                };
                assert!(
                    len >= 0,
                    "unexpected negative return from sqlite3_column_bytes"
                );
                if len > 0 {
                    assert!(
                        !blob.is_null(),
                        "unexpected SQLITE_BLOB column type with NULL data"
                    );
                    ValueRef::Blob(unsafe { from_raw_parts(blob.cast::<u8>(), len) })
                } else {
                    ValueRef::Blob(&[])
                }
            }
            _ => unreachable!("sqlite3_column_type returned invalid value"),
        }
    }
    #[inline]
    pub(super) fn step(&self) -> Result<bool> {
        match self.stmt.step() {
            ffi::SQLITE_ROW => Ok(true),
            ffi::SQLITE_DONE => Ok(false),
            code => Err(self.conn.decode_result(code).unwrap_err()),
        }
    }
    #[inline]
    pub(super) fn reset(&self) -> Result<()> {
        match self.stmt.reset() {
            ffi::SQLITE_OK => Ok(()),
            code => Err(self.conn.decode_result(code).unwrap_err()),
        }
    }
}
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StatementStatus {
    FullscanStep = 1,
    Sort = 2,
    AutoIndex = 3,
    VmStep = 4,
    RePrepare = 5,
    Run = 6,
    FilterMiss = 7,
    FilterHit = 8,
    MemUsed = 99,
}
