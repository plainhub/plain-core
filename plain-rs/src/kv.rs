use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

#[path = "db/schema/key_values.rs"]
mod schema;

#[derive(Clone)]
pub struct Db(Arc<Inner>);

struct Inner {
    connection: Arc<Mutex<Connection>>,
    #[cfg(test)]
    write_failure: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    write_attempts: std::sync::atomic::AtomicUsize,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = crate::sqlite::open(path)?;
        schema::init(&connection)?;
        if path == Path::new(":memory:") {
            return Ok(Self::from_connection(Arc::new(Mutex::new(connection))));
        }
        let path = std::fs::canonicalize(path)?;
        static OPEN: OnceLock<Mutex<HashMap<PathBuf, Weak<Inner>>>> = OnceLock::new();
        let mut opened = OPEN
            .get_or_init(Mutex::default)
            .lock()
            .map_err(|_| anyhow::anyhow!("Key-value database registry poisoned"))?;
        if let Some(inner) = opened.get(&path).and_then(Weak::upgrade) {
            return Ok(Self(inner));
        }
        let db = Self::from_connection(Arc::new(Mutex::new(connection)));
        opened.insert(path, Arc::downgrade(&db.0));
        Ok(db)
    }

    pub(crate) fn from_connection(connection: Arc<Mutex<Connection>>) -> Self {
        Self(Arc::new(Inner {
            connection,
            #[cfg(test)]
            write_failure: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            write_attempts: std::sync::atomic::AtomicUsize::new(0),
        }))
    }

    pub fn get(&self, key: impl AsRef<str>) -> Result<Option<String>> {
        let connection = self.0.connection.lock().unwrap();
        Ok(get(&connection, key.as_ref())?)
    }

    pub fn insert(&self, key: impl AsRef<str>, value: impl AsRef<str>) -> Result<()> {
        let connection = self.0.connection.lock().unwrap();
        self.before_write()?;
        set_in_connection(&connection, key.as_ref(), Some(value.as_ref()))?;
        Ok(())
    }

    pub fn remove(&self, key: impl AsRef<str>) -> Result<()> {
        let connection = self.0.connection.lock().unwrap();
        self.before_write()?;
        set_in_connection(&connection, key.as_ref(), None)?;
        Ok(())
    }

    pub fn scan_prefix(
        &self,
        prefix: impl AsRef<str>,
    ) -> std::vec::IntoIter<Result<(String, String)>> {
        let connection = self.0.connection.lock().unwrap();
        match rows(&connection, prefix.as_ref()) {
            Ok(rows) => rows.into_iter().map(Ok).collect::<Vec<_>>(),
            Err(error) => vec![Err(error.into())],
        }
        .into_iter()
    }

    pub fn iter(&self) -> std::vec::IntoIter<Result<(String, String)>> {
        self.scan_prefix("")
    }

    pub fn get_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        self.get(key)?
            .map(|value| serde_json::from_str(&value).map_err(Into::into))
            .transpose()
    }

    pub fn set_json<T: serde::Serialize>(&self, key: &str, value: T) -> Result<bool> {
        let value = serde_json::to_string(&value)?;
        let connection = self.0.connection.lock().unwrap();
        let transaction =
            rusqlite::Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        if get(&transaction, key)?.as_deref() == Some(&value) {
            return Ok(false);
        }
        self.before_write()?;
        set_in_connection(&transaction, key, Some(&value))?;
        transaction.commit()?;
        Ok(true)
    }

    pub fn update_json<T: serde::Serialize + serde::de::DeserializeOwned + Default, R>(
        &self,
        key: &str,
        change: impl FnOnce(&mut T) -> Result<R>,
    ) -> Result<R> {
        let connection = self.0.connection.lock().unwrap();
        let transaction =
            rusqlite::Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        let mut value: T = get(&transaction, key)?
            .map(|value| serde_json::from_str(&value))
            .transpose()?
            .unwrap_or_default();
        let result = change(&mut value)?;
        self.before_write()?;
        set_in_connection(&transaction, key, Some(&serde_json::to_string(&value)?))?;
        transaction.commit()?;
        Ok(result)
    }

    pub fn update_json_prefix<R>(
        &self,
        prefix: &str,
        change: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>) -> Result<R>,
    ) -> Result<R> {
        let connection = self.0.connection.lock().unwrap();
        let transaction =
            rusqlite::Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        let mut values = serde_json::Map::new();
        for (key, value) in rows(&transaction, prefix)? {
            values.insert(key, serde_json::from_str(&value)?);
        }
        let previous = values.clone();
        let result = change(&mut values)?;
        for key in values.keys() {
            anyhow::ensure!(key.starts_with(prefix), "Invalid state key");
        }
        if previous != values {
            self.before_write()?;
            for key in previous.keys().filter(|key| !values.contains_key(*key)) {
                set_in_connection(&transaction, key, None)?;
            }
            for (key, value) in &values {
                if previous.get(key) != Some(value) {
                    set_in_connection(&transaction, key, Some(&serde_json::to_string(value)?))?;
                }
            }
        }
        transaction.commit()?;
        Ok(result)
    }

    pub fn batch(&self) -> Batch {
        self.batch_with_capacity(16)
    }

    pub fn batch_with_capacity(&self, items: usize) -> Batch {
        Batch {
            changes: Vec::with_capacity(items),
        }
    }

    pub fn apply_batch(&self, batch: Batch) -> Result<()> {
        let connection = self.0.connection.lock().unwrap();
        let transaction =
            rusqlite::Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        self.before_write()?;
        for (key, value) in batch.changes {
            set_in_connection(&transaction, &key, value.as_deref())?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn before_write(&self) -> Result<()> {
        #[cfg(test)]
        {
            self.0
                .write_attempts
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            anyhow::ensure!(
                !self
                    .0
                    .write_failure
                    .load(std::sync::atomic::Ordering::Relaxed),
                "injected KV write failure"
            );
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn set_write_failure_for_test(&self, failure: bool) {
        self.0
            .write_failure
            .store(failure, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(test)]
    pub(crate) fn write_attempts_for_test(&self) -> usize {
        self.0
            .write_attempts
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

fn get(connection: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    connection
        .query_row("SELECT value FROM key_values WHERE key=?1", [key], |row| {
            row.get(0)
        })
        .optional()
}

pub(crate) fn set_in_connection(
    connection: &Connection,
    key: &str,
    value: Option<&str>,
) -> rusqlite::Result<()> {
    if let Some(value) = value {
        connection.execute("INSERT INTO key_values(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value])?;
    } else {
        connection.execute("DELETE FROM key_values WHERE key=?1", [key])?;
    }
    Ok(())
}

fn rows(connection: &Connection, prefix: &str) -> rusqlite::Result<Vec<(String, String)>> {
    let mut upper = prefix.to_owned();
    let end = loop {
        match upper.pop() {
            Some(last) if last < char::MAX => {
                let next = last as u32 + 1;
                upper.push(char::from_u32(next).unwrap_or('\u{e000}'));
                break Some(upper);
            }
            Some(_) => {}
            None => break None,
        }
    };
    let sql = if end.is_some() {
        "SELECT key,value FROM key_values WHERE key>=?1 AND key<?2 ORDER BY key"
    } else {
        "SELECT key,value FROM key_values WHERE key>=?1 ORDER BY key"
    };
    let mut statement = connection.prepare(sql)?;
    let params = if let Some(end) = end.as_deref() {
        vec![prefix, end]
    } else {
        vec![prefix]
    };
    statement
        .query_map(rusqlite::params_from_iter(params), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect()
}

pub struct LazyDb {
    path: PathBuf,
    db: Mutex<Option<Arc<Db>>>,
}

impl LazyDb {
    pub fn from_open(path: PathBuf, db: Arc<Db>) -> Self {
        Self {
            path,
            db: Mutex::new(Some(db)),
        }
    }

    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            db: Mutex::new(None),
        }
    }

    pub fn get(&self) -> Result<Arc<Db>> {
        let mut stored = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Key-value database poisoned"))?;
        if stored.is_none() {
            *stored = Some(Arc::new(Db::open(&self.path)?));
        }
        Ok(stored.as_ref().unwrap().clone())
    }
}

pub struct Batch {
    changes: Vec<(String, Option<String>)>,
}

impl Batch {
    pub fn insert(&mut self, key: impl AsRef<str>, value: impl AsRef<str>) {
        self.changes
            .push((key.as_ref().to_owned(), Some(value.as_ref().to_owned())));
    }

    pub fn remove(&mut self, key: impl AsRef<str>) {
        self.changes.push((key.as_ref().to_owned(), None));
    }
}

#[cfg(test)]
#[path = "../tests/unit/kv.rs"]
mod tests;
