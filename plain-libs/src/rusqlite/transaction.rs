use crate::rusqlite::pragma::Sql;
use crate::rusqlite::{Connection, Result};
use std::ops::Deref;
#[derive(Copy, Clone)]
#[non_exhaustive]
pub enum TransactionBehavior {
    Deferred,
    Immediate,
    Exclusive,
}
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DropBehavior {
    Rollback,
    Commit,
    Ignore,
    Panic,
}
#[derive(Debug)]
pub struct Transaction<'conn> {
    conn: &'conn Connection,
    drop_behavior: DropBehavior,
}
#[derive(Debug)]
pub struct Savepoint<'conn> {
    conn: &'conn Connection,
    name: String,
    drop_behavior: DropBehavior,
    committed: bool,
}
impl Transaction<'_> {
    #[inline]
    pub fn new(conn: &mut Connection, behavior: TransactionBehavior) -> Result<Transaction<'_>> {
        Self::new_unchecked(conn, behavior)
    }
    #[inline]
    pub fn new_unchecked(
        conn: &Connection,
        behavior: TransactionBehavior,
    ) -> Result<Transaction<'_>> {
        let query = match behavior {
            TransactionBehavior::Deferred => "BEGIN DEFERRED",
            TransactionBehavior::Immediate => "BEGIN IMMEDIATE",
            TransactionBehavior::Exclusive => "BEGIN EXCLUSIVE",
        };
        conn.execute_batch(query).map(move |()| Transaction {
            conn,
            drop_behavior: DropBehavior::Rollback,
        })
    }
    #[inline]
    pub fn savepoint(&mut self) -> Result<Savepoint<'_>> {
        Savepoint::new_(self.conn)
    }
    #[inline]
    pub fn savepoint_with_name<T: Into<String>>(&mut self, name: T) -> Result<Savepoint<'_>> {
        Savepoint::with_name_(self.conn, name)
    }
    #[inline]
    #[must_use]
    pub fn drop_behavior(&self) -> DropBehavior {
        self.drop_behavior
    }
    #[inline]
    pub fn set_drop_behavior(&mut self, drop_behavior: DropBehavior) {
        self.drop_behavior = drop_behavior;
    }
    #[inline]
    pub fn commit(mut self) -> Result<()> {
        self.commit_()
    }
    #[inline]
    fn commit_(&mut self) -> Result<()> {
        self.conn.execute_batch("COMMIT")?;
        Ok(())
    }
    #[inline]
    pub fn rollback(mut self) -> Result<()> {
        self.rollback_()
    }
    #[inline]
    fn rollback_(&mut self) -> Result<()> {
        self.conn.execute_batch("ROLLBACK")?;
        Ok(())
    }
    #[inline]
    pub fn finish(mut self) -> Result<()> {
        self.finish_()
    }
    #[inline]
    fn finish_(&mut self) -> Result<()> {
        if self.conn.is_autocommit() {
            return Ok(());
        }
        match self.drop_behavior() {
            DropBehavior::Commit => self.commit_().or_else(|_| self.rollback_()),
            DropBehavior::Rollback => self.rollback_(),
            DropBehavior::Ignore => Ok(()),
            DropBehavior::Panic => panic!("Transaction dropped unexpectedly."),
        }
    }
}
impl Deref for Transaction<'_> {
    type Target = Connection;
    #[inline]
    fn deref(&self) -> &Connection {
        self.conn
    }
}
impl Drop for Transaction<'_> {
    #[inline]
    fn drop(&mut self) {
        self.finish_();
    }
}
impl Savepoint<'_> {
    #[inline]
    fn with_name_<T: Into<String>>(conn: &Connection, name: T) -> Result<Savepoint<'_>> {
        let name = name.into();
        let sql = cmd("SAVEPOINT", false, name.as_str())?;
        conn.execute_batch(sql.as_str()).map(|()| Savepoint {
            conn,
            name,
            drop_behavior: DropBehavior::Rollback,
            committed: false,
        })
    }
    #[inline]
    fn new_(conn: &Connection) -> Result<Savepoint<'_>> {
        Savepoint::with_name_(conn, "_rusqlite_sp")
    }
    #[inline]
    pub fn new(conn: &mut Connection) -> Result<Savepoint<'_>> {
        Savepoint::new_(conn)
    }
    #[inline]
    pub fn with_name<T: Into<String>>(conn: &mut Connection, name: T) -> Result<Savepoint<'_>> {
        Savepoint::with_name_(conn, name)
    }
    #[inline]
    pub fn savepoint(&mut self) -> Result<Savepoint<'_>> {
        Savepoint::new_(self.conn)
    }
    #[inline]
    pub fn savepoint_with_name<T: Into<String>>(&mut self, name: T) -> Result<Savepoint<'_>> {
        Savepoint::with_name_(self.conn, name)
    }
    #[inline]
    #[must_use]
    pub fn drop_behavior(&self) -> DropBehavior {
        self.drop_behavior
    }
    #[inline]
    pub fn set_drop_behavior(&mut self, drop_behavior: DropBehavior) {
        self.drop_behavior = drop_behavior;
    }
    #[inline]
    pub fn commit(mut self) -> Result<()> {
        self.commit_()
    }
    #[inline]
    fn commit_(&mut self) -> Result<()> {
        let sql = cmd("RELEASE", false, self.name.as_str())?;
        self.conn.execute_batch(sql.as_str())?;
        self.committed = true;
        Ok(())
    }
    #[inline]
    pub fn rollback(&mut self) -> Result<()> {
        let sql = cmd("ROLLBACK", true, self.name.as_str())?;
        self.conn.execute_batch(sql.as_str())
    }
    #[inline]
    pub fn finish(mut self) -> Result<()> {
        self.finish_()
    }
    #[inline]
    fn finish_(&mut self) -> Result<()> {
        if self.committed {
            return Ok(());
        }
        match self.drop_behavior() {
            DropBehavior::Commit => self
                .commit_()
                .or_else(|_| self.rollback().and_then(|()| self.commit_())),
            DropBehavior::Rollback => self.rollback().and_then(|()| self.commit_()),
            DropBehavior::Ignore => Ok(()),
            DropBehavior::Panic => panic!("Savepoint dropped unexpectedly."),
        }
    }
}
fn cmd(cmd: &'static str, to: bool, name: &str) -> Result<Sql> {
    let mut sql = Sql::new();
    sql.push_keyword(cmd)?;
    sql.push_space();
    if to {
        sql.push_keyword("TO")?;
        sql.push_space();
    }
    sql.push_identifier(name);
    Ok(sql)
}
impl Deref for Savepoint<'_> {
    type Target = Connection;
    #[inline]
    fn deref(&self) -> &Connection {
        self.conn
    }
}
impl Drop for Savepoint<'_> {
    #[inline]
    fn drop(&mut self) {
        self.finish_();
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransactionState {
    None,
    Read,
    Write,
}
impl Connection {
    #[inline]
    pub fn transaction(&mut self) -> Result<Transaction<'_>> {
        Transaction::new(self, self.transaction_behavior)
    }
    #[inline]
    pub fn transaction_with_behavior(
        &mut self,
        behavior: TransactionBehavior,
    ) -> Result<Transaction<'_>> {
        Transaction::new(self, behavior)
    }
    pub fn unchecked_transaction(&self) -> Result<Transaction<'_>> {
        Transaction::new_unchecked(self, self.transaction_behavior)
    }
    #[inline]
    pub fn savepoint(&mut self) -> Result<Savepoint<'_>> {
        Savepoint::new(self)
    }
    #[inline]
    pub fn savepoint_with_name<T: Into<String>>(&mut self, name: T) -> Result<Savepoint<'_>> {
        Savepoint::with_name(self, name)
    }
}
