use crate::rusqlite::types::FromSqlError;
use crate::rusqlite::types::Type;
use crate::rusqlite::{Result, errmsg_to_string, ffi};
use std::error;
use std::ffi::{NulError, c_char, c_int};
use std::fmt;
use std::path::PathBuf;
use std::str;
macro_rules! cfg_select { ({ $($tt : tt) * }) => { { $crate::rusqlite::cfg_select ! { $($tt) * } } } ; (_ => { $($output : tt) * }) => { $($output) * } ; ($cfg : meta => $output : tt $($($rest : tt) +) ?) => { { #[cfg ($cfg)] cfg_select ! { _ => $output } $(#[cfg (not ($cfg))] cfg_select ! { $($rest) + }) ? } } }
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    SqliteFailure(ffi::Error, Option<String>),
    SqliteSingleThreadedMode,
    FromSqlConversionFailure(usize, Type, Box<dyn error::Error + Send + Sync + 'static>),
    IntegralValueOutOfRange(usize, i64),
    Utf8Error(usize, str::Utf8Error),
    NulError(NulError),
    InvalidParameterName(String),
    InvalidPath(PathBuf),
    ExecuteReturnedResults,
    QueryReturnedNoRows,
    QueryReturnedMoreThanOneRow,
    InvalidColumnIndex(usize),
    InvalidColumnName(String),
    InvalidColumnType(usize, String, Type),
    StatementChangedRows(usize),
    ToSqlConversionFailure(Box<dyn error::Error + Send + Sync + 'static>),
    InvalidQuery,
    UnwindingPanic,
    MultipleStatement,
    InvalidParameterCount(usize, usize),
    SqlInputError {
        error: ffi::Error,
        msg: String,
        sql: String,
        offset: c_int,
    },
    InvalidDatabaseIndex(usize),
}
impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::SqliteFailure(e1, s1), Self::SqliteFailure(e2, s2)) => e1 == e2 && s1 == s2,
            (Self::SqliteSingleThreadedMode, Self::SqliteSingleThreadedMode) => true,
            (Self::IntegralValueOutOfRange(i1, n1), Self::IntegralValueOutOfRange(i2, n2)) => {
                i1 == i2 && n1 == n2
            }
            (Self::Utf8Error(i1, e1), Self::Utf8Error(i2, e2)) => i1 == i2 && e1 == e2,
            (Self::NulError(e1), Self::NulError(e2)) => e1 == e2,
            (Self::InvalidParameterName(n1), Self::InvalidParameterName(n2)) => n1 == n2,
            (Self::InvalidPath(p1), Self::InvalidPath(p2)) => p1 == p2,
            (Self::ExecuteReturnedResults, Self::ExecuteReturnedResults) => true,
            (Self::QueryReturnedNoRows, Self::QueryReturnedNoRows) => true,
            (Self::QueryReturnedMoreThanOneRow, Self::QueryReturnedMoreThanOneRow) => true,
            (Self::InvalidColumnIndex(i1), Self::InvalidColumnIndex(i2)) => i1 == i2,
            (Self::InvalidColumnName(n1), Self::InvalidColumnName(n2)) => n1 == n2,
            (Self::InvalidColumnType(i1, n1, t1), Self::InvalidColumnType(i2, n2, t2)) => {
                i1 == i2 && t1 == t2 && n1 == n2
            }
            (Self::StatementChangedRows(n1), Self::StatementChangedRows(n2)) => n1 == n2,
            (Self::InvalidQuery, Self::InvalidQuery) => true,
            (Self::UnwindingPanic, Self::UnwindingPanic) => true,
            (Self::InvalidParameterCount(i1, n1), Self::InvalidParameterCount(i2, n2)) => {
                i1 == i2 && n1 == n2
            }
            (
                Self::SqlInputError {
                    error: e1,
                    msg: m1,
                    sql: s1,
                    offset: o1,
                },
                Self::SqlInputError {
                    error: e2,
                    msg: m2,
                    sql: s2,
                    offset: o2,
                },
            ) => e1 == e2 && m1 == m2 && s1 == s2 && o1 == o2,
            (Self::InvalidDatabaseIndex(i1), Self::InvalidDatabaseIndex(i2)) => i1 == i2,
            (..) => false,
        }
    }
}
impl From<str::Utf8Error> for Error {
    #[cold]
    fn from(err: str::Utf8Error) -> Self {
        Self::Utf8Error(UNKNOWN_COLUMN, err)
    }
}
impl From<NulError> for Error {
    #[cold]
    fn from(err: NulError) -> Self {
        Self::NulError(err)
    }
}
const UNKNOWN_COLUMN: usize = usize::MAX;
impl From<FromSqlError> for Error {
    #[cold]
    fn from(err: FromSqlError) -> Self {
        match err {
            FromSqlError::OutOfRange(val) => Self::IntegralValueOutOfRange(UNKNOWN_COLUMN, val),
            FromSqlError::InvalidBlobSize { .. } => {
                Self::FromSqlConversionFailure(UNKNOWN_COLUMN, Type::Blob, Box::new(err))
            }
            FromSqlError::Other(source) => {
                Self::FromSqlConversionFailure(UNKNOWN_COLUMN, Type::Null, source)
            }
            _ => Self::FromSqlConversionFailure(UNKNOWN_COLUMN, Type::Null, Box::new(err)),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::SqliteFailure(ref err, None) => err.fmt(f),
            Self::SqliteFailure(_, Some(ref s)) => write!(f, "{s}"),
            Self::SqliteSingleThreadedMode => write!(
                f,
                "SQLite was compiled or configured for single-threaded use only"
            ),
            Self::FromSqlConversionFailure(i, ref t, ref err) => {
                if i == UNKNOWN_COLUMN {
                    err.fmt(f)
                } else {
                    write!(f, "Conversion error from type {t} at index: {i}, {err}")
                }
            }
            Self::IntegralValueOutOfRange(col, val) => {
                if col == UNKNOWN_COLUMN {
                    write!(f, "Integer {val} out of range")
                } else {
                    write!(f, "Integer {val} out of range at index {col}")
                }
            }
            Self::Utf8Error(col, ref err) => {
                if col == UNKNOWN_COLUMN {
                    err.fmt(f)
                } else {
                    write!(f, "{err} at index {col}")
                }
            }
            Self::NulError(ref err) => err.fmt(f),
            Self::InvalidParameterName(ref name) => write!(f, "Invalid parameter name: {name}"),
            Self::InvalidPath(ref p) => write!(f, "Invalid path: {}", p.to_string_lossy()),
            Self::ExecuteReturnedResults => {
                write!(f, "Execute returned results - did you mean to call query?")
            }
            Self::QueryReturnedNoRows => write!(f, "Query returned no rows"),
            Self::QueryReturnedMoreThanOneRow => write!(f, "Query returned more than one row"),
            Self::InvalidColumnIndex(i) => write!(f, "Invalid column index: {i}"),
            Self::InvalidColumnName(ref name) => write!(f, "Invalid column name: {name}"),
            Self::InvalidColumnType(i, ref name, ref t) => {
                write!(f, "Invalid column type {t} at index: {i}, name: {name}")
            }
            Self::InvalidParameterCount(i1, n1) => write!(
                f,
                "Wrong number of parameters passed to query. Got {i1}, needed {n1}"
            ),
            Self::StatementChangedRows(i) => write!(f, "Query changed {i} rows"),
            Self::ToSqlConversionFailure(ref err) => err.fmt(f),
            Self::InvalidQuery => write!(f, "Query is not read-only"),
            Self::UnwindingPanic => write!(f, "unwinding panic"),
            Self::MultipleStatement => write!(f, "Multiple statements provided"),
            Self::SqlInputError {
                ref msg,
                offset,
                ref sql,
                ..
            } => write!(f, "{msg} in {sql} at offset {offset}"),
            Self::InvalidDatabaseIndex(i) => write!(f, "Invalid database index: {i}"),
        }
    }
}
impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            Self::SqliteFailure(ref err, _) => Some(err),
            Self::Utf8Error(_, ref err) => Some(err),
            Self::NulError(ref err) => Some(err),
            Self::IntegralValueOutOfRange(..)
            | Self::SqliteSingleThreadedMode
            | Self::InvalidParameterName(_)
            | Self::ExecuteReturnedResults
            | Self::QueryReturnedNoRows
            | Self::QueryReturnedMoreThanOneRow
            | Self::InvalidColumnIndex(_)
            | Self::InvalidColumnName(_)
            | Self::InvalidColumnType(..)
            | Self::InvalidPath(_)
            | Self::InvalidParameterCount(..)
            | Self::StatementChangedRows(_)
            | Self::InvalidQuery
            | Self::MultipleStatement => None,
            Self::FromSqlConversionFailure(_, _, ref err)
            | Self::ToSqlConversionFailure(ref err) => Some(&**err),
            Self::UnwindingPanic => None,
            Self::SqlInputError { ref error, .. } => Some(error),
            Self::InvalidDatabaseIndex(_) => None,
        }
    }
}
impl Error {
    #[inline]
    #[must_use]
    pub fn sqlite_error(&self) -> Option<&ffi::Error> {
        match self {
            Self::SqliteFailure(error, _) => Some(error),
            _ => None,
        }
    }
}
#[cold]
pub fn error_from_sqlite_code(code: c_int, message: Option<String>) -> Error {
    Error::SqliteFailure(ffi::Error::new(code), message)
}
macro_rules! err { ($code : expr $(,) ?) => { $crate::rusqlite::error::error_from_sqlite_code ($code , None) } ; ($code : expr , $msg : literal $(,) ?) => { $crate::rusqlite::error::error_from_sqlite_code ($code , Some (format ! ($msg))) } ; ($code : expr , $err : expr $(,) ?) => { $crate::rusqlite::error::error_from_sqlite_code ($code , Some (format ! ($err))) } ; ($code : expr , $fmt : expr , $($arg : tt) *) => { $crate::rusqlite::error::error_from_sqlite_code ($code , Some (format ! ($fmt , $($arg) *))) } ; }
#[cold]
pub unsafe fn error_from_handle(db: *mut ffi::sqlite3, code: c_int) -> Error {
    unsafe { error_from_sqlite_code(code, unsafe { error_msg(db, code) }) }
}
unsafe fn error_msg(db: *mut ffi::sqlite3, code: c_int) -> Option<String> {
    unsafe {
        unsafe {
            if db.is_null() || ffi::sqlite3_errcode(db) != code {
                let err_str = ffi::sqlite3_errstr(code);
                if err_str.is_null() {
                    None
                } else {
                    Some(errmsg_to_string(err_str))
                }
            } else {
                Some(errmsg_to_string(ffi::sqlite3_errmsg(db)))
            }
        }
    }
}
pub unsafe fn decode_result_raw(db: *mut ffi::sqlite3, code: c_int) -> Result<()> {
    unsafe {
        if code == ffi::SQLITE_OK {
            Ok(())
        } else {
            Err(unsafe { error_from_handle(db, code) })
        }
    }
}
#[cold]
pub unsafe fn error_with_offset(db: *mut ffi::sqlite3, code: c_int, sql: &str) -> Error {
    unsafe {
        unsafe {
            if db.is_null() {
                error_from_sqlite_code(code, None)
            } else {
                let error = ffi::Error::new(code);
                let msg = error_msg(db, code);
                if ffi::ErrorCode::Unknown == error.code {
                    let offset = ffi::sqlite3_error_offset(db);
                    if offset >= 0 {
                        return Error::SqlInputError {
                            error,
                            msg: msg.unwrap_or("error".to_owned()),
                            sql: sql.to_owned(),
                            offset,
                        };
                    }
                }
                Error::SqliteFailure(error, msg)
            }
        }
    }
}
pub fn check(code: c_int) -> Result<()> {
    if code == ffi::SQLITE_OK {
        Ok(())
    } else {
        Err(error_from_sqlite_code(code, None))
    }
}
pub unsafe fn to_sqlite_error(e: &Error, err_msg: *mut *mut c_char) -> c_int {
    unsafe {
        use crate::rusqlite::util::alloc;
        match e {
            Error::SqliteFailure(err, s) => {
                if let Some(s) = s {
                    unsafe { *err_msg = alloc(s) };
                }
                err.extended_code
            }
            err => {
                unsafe { *err_msg = alloc(&err.to_string()) };
                ffi::SQLITE_ERROR
            }
        }
    }
}
pub unsafe fn set_errmsg(
    db: *mut ffi::sqlite3,
    code: c_int,
    msg: Option<&std::ffi::CStr>,
) -> Result<()> {
    unsafe {
        unsafe {
            decode_result_raw(
                db,
                ffi::sqlite3_set_errmsg(
                    db,
                    code,
                    msg.map_or(std::ptr::null(), std::ffi::CStr::as_ptr),
                ),
            )
        }
    }
}
