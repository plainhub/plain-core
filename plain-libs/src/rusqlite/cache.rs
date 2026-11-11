use crate::hashlink::LruCache;
use crate::rusqlite::raw_statement::RawStatement;
use crate::rusqlite::{Connection, PrepFlags, Result, Statement};
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
impl Connection {
    #[inline]
    pub fn prepare_cached(&self, sql: &str) -> Result<CachedStatement<'_>> {
        self.cache.get(self, sql)
    }

    #[inline]
    pub fn flush_prepared_statement_cache(&self) {
        self.cache.flush();
    }
}
#[derive(Debug)]
pub struct StatementCache(RefCell<LruCache<Arc<str>, RawStatement>>);
unsafe impl Send for StatementCache {}
pub struct CachedStatement<'conn> {
    stmt: Option<Statement<'conn>>,
    cache: &'conn StatementCache,
}
impl<'conn> Deref for CachedStatement<'conn> {
    type Target = Statement<'conn>;
    #[inline]
    fn deref(&self) -> &Statement<'conn> {
        self.stmt.as_ref().unwrap()
    }
}
impl<'conn> DerefMut for CachedStatement<'conn> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Statement<'conn> {
        self.stmt.as_mut().unwrap()
    }
}
impl Drop for CachedStatement<'_> {
    #[inline]
    fn drop(&mut self) {
        if let Some(stmt) = self.stmt.take() {
            self.cache.cache_stmt(unsafe { stmt.into_raw() });
        }
    }
}
impl CachedStatement<'_> {
    #[inline]
    fn new<'conn>(stmt: Statement<'conn>, cache: &'conn StatementCache) -> CachedStatement<'conn> {
        CachedStatement {
            stmt: Some(stmt),
            cache,
        }
    }
    #[inline]
    pub fn discard(mut self) {
        self.stmt = None;
    }
}
impl StatementCache {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(RefCell::new(LruCache::new(capacity)))
    }
    #[inline]
    fn set_capacity(&self, capacity: usize) {
        self.0.borrow_mut().set_capacity(capacity);
    }
    fn get<'conn>(
        &'conn self,
        conn: &'conn Connection,
        sql: &str,
    ) -> Result<CachedStatement<'conn>> {
        let trimmed = sql.trim();
        let mut cache = self.0.borrow_mut();
        let stmt = match cache.remove(trimmed) {
            Some(raw_stmt) => Ok(Statement::new(conn, raw_stmt)),
            None => conn.prepare_with_flags(trimmed, PrepFlags::SQLITE_PREPARE_PERSISTENT),
        };
        stmt.map(|mut stmt| {
            stmt.stmt.set_statement_cache_key(trimmed);
            CachedStatement::new(stmt, self)
        })
    }
    fn cache_stmt(&self, mut stmt: RawStatement) {
        if stmt.is_null() {
            return;
        }
        let mut cache = self.0.borrow_mut();
        stmt.clear_bindings();
        if let Some(sql) = stmt.statement_cache_key() {
            cache.insert(sql, stmt);
        } else {
            debug_assert!(
                false,
                "bug in statement cache code, statement returned to cache that without key"
            );
        }
    }
    #[inline]
    fn flush(&self) {
        let mut cache = self.0.borrow_mut();
        cache.clear();
    }
}
