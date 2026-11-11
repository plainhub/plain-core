pub use crate::rusqlite::bind::BindIndex;
pub use crate::rusqlite::cache::CachedStatement;
use crate::rusqlite::cache::StatementCache;
pub use crate::rusqlite::error::{Error, to_sqlite_error};
pub use crate::rusqlite::ffi::ErrorCode;
use crate::rusqlite::inner_connection::InnerConnection;
pub use crate::rusqlite::params::{Params, ParamsFromIter, params_from_iter};
use crate::rusqlite::raw_statement::RawStatement;
pub use crate::rusqlite::row::{AndThenRows, Map, MappedRows, Row, RowIndex, Rows};
pub use crate::rusqlite::statement::{Statement, StatementStatus};
pub use crate::rusqlite::transaction::TransactionState;
pub use crate::rusqlite::transaction::{DropBehavior, Savepoint, Transaction, TransactionBehavior};
pub use crate::rusqlite::types::ToSql;
use crate::rusqlite::types::ValueRef;
pub use crate::rusqlite::util::Name;
pub use crate::rusqlite::version::*;
pub use libsqlite3_sys as ffi;
use std::cell::RefCell;
use std::default::Default;
use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_void};
use std::fmt;
use std::path::Path;
use std::result;
use std::str;
use std::sync::{Arc, Mutex};
#[macro_use]
mod error;

pub mod backup;
mod bind;
mod busy;
mod cache;
mod column;

pub mod hooks;
mod inner_connection;
mod params;
mod pragma;
mod raw_statement;
mod row;
mod statement;
mod transaction;
pub mod types;
pub(crate) mod util;
mod version;
pub mod wal;
const STATEMENT_CACHE_DEFAULT_CAPACITY: usize = 16;
#[macro_export]
macro_rules! plain_rusqlite_params { () => { & [] as & [& dyn $crate::rusqlite::ToSql] } ; ($($param : expr) ,+ $(,) ?) => { & [$(&$param as & dyn $crate::rusqlite::ToSql) ,+] as & [& dyn $crate::rusqlite::ToSql] } ; }
#[macro_export]
macro_rules! plain_rusqlite_named_params { () => { & [] as & [(& str , & dyn $crate::rusqlite::ToSql)] } ; ($($param_name : literal : $param_val : expr) ,+ $(,) ?) => { & [$(($param_name , &$param_val as & dyn $crate::rusqlite::ToSql)) ,+] as & [(& str , & dyn $crate::rusqlite::ToSql)] } ; }
pub type Result<T, E = Error> = result::Result<T, E>;
pub trait OptionalExtension<T> {
    fn optional(self) -> Result<Option<T>>;
}
impl<T> OptionalExtension<T> for Result<T> {
    fn optional(self) -> Result<Option<T>> {
        match self {
            Ok(value) => Ok(Some(value)),
            Err(Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
unsafe fn errmsg_to_string(errmsg: *const c_char) -> String {
    unsafe {
        unsafe { CStr::from_ptr(errmsg) }
            .to_string_lossy()
            .into_owned()
    }
}
fn str_for_sqlite(
    s: &[u8],
) -> (
    *const c_char,
    ffi::sqlite3_uint64,
    ffi::sqlite3_destructor_type,
) {
    let len = s.len();
    let (ptr, dtor_info) = if len != 0 {
        (s.as_ptr().cast::<c_char>(), ffi::SQLITE_TRANSIENT())
    } else {
        ("".as_ptr().cast::<c_char>(), ffi::SQLITE_STATIC())
    };
    (ptr, len as ffi::sqlite3_uint64, dtor_info)
}
fn path_to_cstring(p: &Path) -> Result<CString> {
    cfg_select! { unix => { use std::os::unix::ffi::OsStrExt as _ ; Ok (CString::new (p . as_os_str () . as_bytes ()) ?) } _ => { let s = p . to_str () . ok_or_else (|| Error::InvalidPath (p . to_owned ())) ?; Ok (CString::new (s) ?) } }
}
pub const MAIN_DB: &CStr = c"main";
pub const DEFAULT_NAME: Option<&'static str> = None;
pub const TEMP_DB: &CStr = c"temp";
pub struct Connection {
    db: RefCell<InnerConnection>,
    cache: StatementCache,
    transaction_behavior: TransactionBehavior,
}
unsafe impl Send for Connection {}
impl Drop for Connection {
    #[inline]
    fn drop(&mut self) {
        self.flush_prepared_statement_cache();
    }
}
impl Connection {
    #[inline]
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let flags = OpenFlags::default();
        Self::open_with_flags(path, flags)
    }
    #[inline]
    pub fn open_in_memory() -> Result<Self> {
        let flags = OpenFlags::default();
        Self::open_in_memory_with_flags(flags)
    }
    #[inline]
    pub fn open_with_flags<P: AsRef<Path>>(path: P, flags: OpenFlags) -> Result<Self> {
        let c_path = path_to_cstring(path.as_ref())?;
        InnerConnection::open_with_flags(&c_path, flags, None).map(|db| Self {
            db: RefCell::new(db),
            cache: StatementCache::with_capacity(STATEMENT_CACHE_DEFAULT_CAPACITY),
            transaction_behavior: TransactionBehavior::Deferred,
        })
    }
    #[inline]
    pub fn open_with_flags_and_vfs<P: AsRef<Path>, V: Name>(
        path: P,
        flags: OpenFlags,
        vfs: V,
    ) -> Result<Self> {
        let c_path = path_to_cstring(path.as_ref())?;
        let c_vfs = vfs.as_cstr()?;
        InnerConnection::open_with_flags(&c_path, flags, Some(&c_vfs)).map(|db| Self {
            db: RefCell::new(db),
            cache: StatementCache::with_capacity(STATEMENT_CACHE_DEFAULT_CAPACITY),
            transaction_behavior: TransactionBehavior::Deferred,
        })
    }
    #[inline]
    pub fn open_in_memory_with_flags(flags: OpenFlags) -> Result<Self> {
        Self::open_with_flags(":memory:", flags)
    }

    pub fn execute_batch(&self, sql: &str) -> Result<()> {
        let mut sql = sql;
        while !sql.is_empty() {
            let (stmt, tail) = self
                .db
                .borrow_mut()
                .prepare(self, sql, PrepFlags::default())?;
            if !stmt.stmt.is_null() && stmt.step()? {
                if false {
                    return Err(Error::ExecuteReturnedResults);
                }
            }
            if tail == 0 || tail >= sql.len() {
                break;
            }
            sql = &sql[tail..];
        }
        Ok(())
    }
    #[inline]
    pub fn execute<P: Params>(&self, sql: &str, params: P) -> Result<usize> {
        self.prepare(sql).and_then(|mut stmt| stmt.execute(params))
    }
    #[inline]
    pub fn path(&self) -> Option<&str> {
        unsafe { inner_connection::db_filename(std::marker::PhantomData, self.handle(), MAIN_DB) }
    }
    #[inline]
    pub fn release_memory(&self) -> Result<()> {
        self.db.borrow_mut().release_memory()
    }
    #[inline]
    pub fn last_insert_rowid(&self) -> i64 {
        self.db.borrow_mut().last_insert_rowid()
    }
    #[inline]
    pub fn query_row<T, P, F>(&self, sql: &str, params: P, f: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&Row<'_>) -> Result<T>,
    {
        let mut stmt = self.prepare(sql)?;
        stmt.query_row(params, f)
    }
    pub fn query_one<T, P, F>(&self, sql: &str, params: P, f: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&Row<'_>) -> Result<T>,
    {
        let mut stmt = self.prepare(sql)?;
        stmt.query_one(params, f)
    }
    pub(crate) fn one_column<T, P>(&self, sql: &str, params: P) -> Result<T>
    where
        T: types::FromSql,
        P: Params,
    {
        self.query_one(sql, params, |r| r.get(0))
    }

    #[inline]
    pub fn prepare(&self, sql: &str) -> Result<Statement<'_>> {
        self.prepare_with_flags(sql, PrepFlags::default())
    }
    #[inline]
    pub fn prepare_with_flags(&self, sql: &str, flags: PrepFlags) -> Result<Statement<'_>> {
        let (stmt, tail) = self.db.borrow_mut().prepare(self, sql, flags)?;
        if tail != 0 && !self.prepare(&sql[tail..])?.stmt.is_null() {
            Err(Error::MultipleStatement)
        } else {
            Ok(stmt)
        }
    }
    #[inline]
    pub fn close(self) -> Result<(), (Self, Error)> {
        self.flush_prepared_statement_cache();
        let r = self.db.borrow_mut().close();
        r.map_err(move |err| (self, err))
    }
    #[inline]
    pub unsafe fn handle(&self) -> *mut ffi::sqlite3 {
        self.db.borrow().db()
    }
    #[inline]
    pub unsafe fn from_handle(db: *mut ffi::sqlite3) -> Result<Self> {
        let db = unsafe { InnerConnection::new(db, false) };
        Ok(Self {
            db: RefCell::new(db),
            cache: StatementCache::with_capacity(STATEMENT_CACHE_DEFAULT_CAPACITY),
            transaction_behavior: TransactionBehavior::Deferred,
        })
    }

    #[inline]
    pub fn get_interrupt_handle(&self) -> InterruptHandle {
        self.db.borrow().get_interrupt_handle()
    }
    #[inline]
    fn decode_result(&self, code: c_int) -> Result<()> {
        self.db.borrow().decode_result(code)
    }
    #[inline]
    pub fn changes(&self) -> u64 {
        self.db.borrow().changes()
    }
    #[inline]
    pub fn total_changes(&self) -> u64 {
        self.db.borrow().total_changes()
    }
    #[inline]
    pub fn is_autocommit(&self) -> bool {
        self.db.borrow().is_autocommit()
    }
    #[inline]
    pub fn is_busy(&self) -> bool {
        self.db.borrow().is_busy()
    }
    pub fn cache_flush(&self) -> Result<()> {
        self.db.borrow_mut().cache_flush()
    }

    pub fn db_name(&self, index: usize) -> Result<String> {
        unsafe {
            let db = self.handle();
            let name = ffi::sqlite3_db_name(db, index as c_int);
            if name.is_null() {
                Err(Error::InvalidDatabaseIndex(index))
            } else {
                Ok(CStr::from_ptr(name).to_str()?.to_owned())
            }
        }
    }
    pub fn is_interrupted(&self) -> bool {
        self.db.borrow().is_interrupted()
    }
    pub unsafe fn set_clientdata<T: Send + 'static, N: Name>(
        &self,
        name: N,
        data: Option<T>,
    ) -> Result<*mut T> {
        self.db
            .borrow_mut()
            .set_clientdata(name, data, |_, _| ffi::SQLITE_OK)
    }
    pub unsafe fn get_clientdata<T, N: Name>(&self, name: N) -> Result<Option<&T>> {
        unsafe {
            self.db
                .borrow()
                .get_clientdata(name)
                .map(|p: *mut T| p.as_ref())
        }
    }
    pub fn set_errmsg(&self, code: c_int, msg: Option<&CStr>) -> Result<()> {
        unsafe { error::set_errmsg(self.handle(), code, msg) }
    }
    pub fn file_control<N: Name>(&self, db_name: Option<N>, op_and_arg: FileControl) -> Result<()> {
        fn to_void<X>(r: &mut X) -> *mut c_void {
            std::ptr::from_mut(r).cast::<c_void>()
        }
        let (op, arg) = match op_and_arg {
            FileControl::SizeHint(p) => (ffi::SQLITE_FCNTL_SIZE_HINT, to_void(p)),
            FileControl::SizeLimit(p) => (ffi::SQLITE_FCNTL_SIZE_LIMIT, to_void(p)),
            FileControl::ChunkSize(p) => (ffi::SQLITE_FCNTL_CHUNK_SIZE, to_void(p)),
            FileControl::PersistWal(p) => (ffi::SQLITE_FCNTL_PERSIST_WAL, to_void(p)),
            FileControl::PowerSafeOverwrite(p) => {
                (ffi::SQLITE_FCNTL_POWERSAFE_OVERWRITE, to_void(p))
            }
            FileControl::MMapSize(p) => (ffi::SQLITE_FCNTL_MMAP_SIZE, to_void(p)),
            #[cfg(unix)]
            FileControl::HasMoved(p) => (ffi::SQLITE_FCNTL_HAS_MOVED, to_void(p)),
            FileControl::LockTimeout(p) => (ffi::SQLITE_FCNTL_LOCK_TIMEOUT, to_void(p)),
            FileControl::BlockOnConnect(p) => (ffi::SQLITE_FCNTL_BLOCK_ON_CONNECT, to_void(p)),
            FileControl::DataVersion(p) => (ffi::SQLITE_FCNTL_DATA_VERSION, to_void(p)),
            FileControl::ReserveBytes(p) => (ffi::SQLITE_FCNTL_RESERVE_BYTES, to_void(p)),
        };
        unsafe { self.db.borrow().file_control(db_name, op, arg) }
    }
}
impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("path", &self.path())
            .finish()
    }
}
#[derive(Debug)]
pub struct Batch<'conn, 'sql> {
    conn: &'conn Connection,
    sql: &'sql str,
    tail: usize,
}
impl<'conn, 'sql> Batch<'conn, 'sql> {
    pub fn new(conn: &'conn Connection, sql: &'sql str) -> Self {
        Batch { conn, sql, tail: 0 }
    }
}
impl<'conn> crate::fallible_iterator::FallibleIterator for Batch<'conn, '_> {
    type Error = Error;
    type Item = Statement<'conn>;
    fn next(&mut self) -> Result<Option<Statement<'conn>>> {
        while self.tail < self.sql.len() {
            let sql = &self.sql[self.tail..];
            let (next, tail) =
                self.conn
                    .db
                    .borrow_mut()
                    .prepare(self.conn, sql, PrepFlags::default())?;
            if tail == 0 {
                self.tail = self.sql.len();
            } else {
                self.tail += tail;
            }
            if next.stmt.is_null() {
                continue;
            }
            return Ok(Some(next));
        }
        Ok(None)
    }
}
bitflags::bitflags! { #[derive (Clone , Copy , Debug , Eq , Hash , PartialEq)] #[repr (C)] pub struct OpenFlags : c_int { const SQLITE_OPEN_READ_ONLY = ffi::SQLITE_OPEN_READONLY ; const SQLITE_OPEN_READ_WRITE = ffi::SQLITE_OPEN_READWRITE ; const SQLITE_OPEN_CREATE = ffi::SQLITE_OPEN_CREATE ; const SQLITE_OPEN_URI = ffi::SQLITE_OPEN_URI ; const SQLITE_OPEN_MEMORY = ffi::SQLITE_OPEN_MEMORY ; const SQLITE_OPEN_NO_MUTEX = ffi::SQLITE_OPEN_NOMUTEX ; const SQLITE_OPEN_FULL_MUTEX = ffi::SQLITE_OPEN_FULLMUTEX ; const SQLITE_OPEN_SHARED_CACHE = 0x0002_0000 ; const SQLITE_OPEN_PRIVATE_CACHE = 0x0004_0000 ; const SQLITE_OPEN_NOFOLLOW = 0x0100_0000 ; const SQLITE_OPEN_EXRESCODE = 0x0200_0000 ; } }
impl Default for OpenFlags {
    #[inline]
    fn default() -> Self {
        Self::SQLITE_OPEN_READ_WRITE
            | Self::SQLITE_OPEN_CREATE
            | Self::SQLITE_OPEN_NO_MUTEX
            | Self::SQLITE_OPEN_URI
    }
}
bitflags::bitflags! { #[derive (Clone , Copy , Debug , Default , Eq , Hash , PartialEq)] #[repr (C)] pub struct PrepFlags : c_uint { const SQLITE_PREPARE_PERSISTENT = 0x01 ; const SQLITE_PREPARE_NO_VTAB = 0x04 ; const SQLITE_PREPARE_DONT_LOG = 0x10 ; const SQLITE_PREPARE_FROM_DDL = 0x20 ; } }
pub struct InterruptHandle {
    db_lock: Arc<Mutex<*mut ffi::sqlite3>>,
}
unsafe impl Send for InterruptHandle {}
unsafe impl Sync for InterruptHandle {}
impl InterruptHandle {
    pub fn interrupt(&self) {
        let db_handle = self.db_lock.lock().unwrap();
        if !db_handle.is_null() {
            unsafe { ffi::sqlite3_interrupt(*db_handle) }
        }
    }
}
#[derive(Debug)]
#[non_exhaustive]
pub enum FileControl<'p> {
    SizeHint(&'p mut ffi::sqlite3_int64),
    SizeLimit(&'p mut ffi::sqlite3_int64),
    ChunkSize(&'p mut c_int),
    PersistWal(&'p mut c_int),
    PowerSafeOverwrite(&'p mut c_int),
    MMapSize(&'p mut ffi::sqlite3_int64),
    #[cfg(unix)]
    HasMoved(&'p mut c_int),
    LockTimeout(&'p mut c_int),
    BlockOnConnect(&'p mut c_int),
    DataVersion(&'p mut c_uint),
    ReserveBytes(&'p mut c_int),
}
pub use crate::{plain_rusqlite_named_params as named_params, plain_rusqlite_params as params};

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/rusqlite.rs"]
mod tests;
