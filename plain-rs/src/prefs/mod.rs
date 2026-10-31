//! One instance, one flat preferences file: prefs.json.
pub mod dlna;
pub mod identity;
pub mod media;
pub mod pref_keys;
pub mod samba;
pub mod security;
pub mod settings;
pub mod storage;

pub use identity::{AppIdentity, ensure_identity, ensure_mdns_hostname, ensure_url_token};

use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

static GLOBAL: OnceLock<Arc<Prefs>> = OnceLock::new();

/// Install the process-wide preferences (called once by the host shell
/// right after loading). Panics if called twice.
pub fn set_global(prefs: Arc<Prefs>) {
    if GLOBAL.set(prefs).is_err() {
        panic!("prefs::set_global called twice");
    }
}

/// Returns the process-wide preferences. Panics if `set_global` was not
/// called yet — same contract as `media::kv::get_default`.
pub fn get_default() -> &'static Prefs {
    GLOBAL
        .get()
        .expect("prefs::set_global must be called before get_default")
        .as_ref()
}

/// Non-panicking variant for background paths that may run before (or
/// without) the global being installed — e.g. the automount watcher.
pub fn try_get_default() -> Option<Arc<Prefs>> {
    GLOBAL.get().cloned()
}

/// Prefs failures carry the file path so a hand-edit mistake or disk
/// error is loud and actionable. Implements `std::error::Error`, so
/// `?` converts into the hosts' anyhow / GraphQL error types.
#[derive(Debug)]
pub enum PrefsError {
    UnknownKey(String),
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    Encode(serde_json::Error),
    /// Deserialize of a stored value into the requested type failed.
    Decode(serde_json::Error),
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for PrefsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrefsError::UnknownKey(key) => write!(f, "Unknown preference: {key}"),
            PrefsError::Read { path, source } => {
                write!(f, "read prefs {}: {source}", path.display())
            }
            PrefsError::Parse { path, source } => {
                write!(f, "parse prefs {}: {source}", path.display())
            }
            PrefsError::Encode(source) => write!(f, "encode prefs: {source}"),
            PrefsError::Decode(source) => write!(f, "decode prefs value: {source}"),
            PrefsError::Write { path, source } => {
                write!(f, "write prefs {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for PrefsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PrefsError::UnknownKey(_) => None,
            PrefsError::Read { source, .. } | PrefsError::Write { source, .. } => Some(source),
            PrefsError::Parse { source, .. }
            | PrefsError::Encode(source)
            | PrefsError::Decode(source) => Some(source),
        }
    }
}

pub type Result<T> = std::result::Result<T, PrefsError>;

pub struct Prefs {
    path: PathBuf,
    inner: RwLock<Map<String, Value>>,
    change_hook: RwLock<Option<Arc<dyn Fn() + Send + Sync>>>,
}
impl fmt::Debug for Prefs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Prefs").field("path", &self.path).finish()
    }
}
pub fn default_path(data_dir: &Path) -> PathBuf {
    crate::data_paths::DataPaths::new(data_dir).prefs()
}
impl Prefs {
    pub fn set_change_hook(&self, hook: Option<Arc<dyn Fn() + Send + Sync>>) {
        *self.change_hook.write().unwrap() = hook;
    }
    pub fn notify_changed(&self) {
        let hook = self.change_hook.read().unwrap().clone();
        if let Some(hook) = hook {
            hook();
        }
    }
    pub fn in_memory() -> Self {
        Self {
            path: PathBuf::new(),
            inner: RwLock::new(Map::new()),
            change_hook: RwLock::new(None),
        }
    }
    pub fn load(path: &Path) -> Result<Self> {
        let inner = match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|source| PrefsError::Parse {
                path: path.to_owned(),
                source,
            })?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Map::new(),
            Err(source) => {
                return Err(PrefsError::Read {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        Ok(Self {
            path: path.to_owned(),
            inner: RwLock::new(inner),
            change_hook: RwLock::new(None),
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        self.inner
            .read()
            .unwrap()
            .get(key)
            .map(|value| serde_json::from_value(value.clone()).map_err(PrefsError::Decode))
            .transpose()
    }
    pub fn get_or<T: DeserializeOwned>(&self, key: &str, default: T) -> T {
        self.get(key).unwrap_or(None).unwrap_or(default)
    }
    pub fn set<T: Serialize>(&self, key: &str, value: T) -> Result<bool> {
        let value = serde_json::to_value(value).map_err(PrefsError::Encode)?;
        self.update(|entries| {
            if entries.get(key) == Some(&value) {
                return Ok(false);
            }
            entries.insert(key.to_owned(), value);
            Ok(true)
        })
    }
    pub fn remove(&self, key: &str) -> Result<bool> {
        self.update(|entries| Ok(entries.remove(key).is_some()))
    }
    pub fn clear(&self) -> Result<()> {
        self.update(|entries| {
            entries.clear();
            Ok(())
        })
    }
    pub(crate) fn update<T>(
        &self,
        update: impl FnOnce(&mut Map<String, Value>) -> Result<T>,
    ) -> Result<T> {
        let mut stored = self.inner.write().unwrap();
        let mut next = stored.clone();
        let result = update(&mut next)?;
        if let Some(key) = next.keys().find(|key| pref_keys::access(key).is_none()) {
            return Err(PrefsError::UnknownKey(key.clone()));
        }
        if next != *stored {
            self.save(&next)?;
            *stored = next;
            drop(stored);
            self.notify_changed();
        }
        Ok(result)
    }
    pub fn entries(&self) -> Vec<(String, Value)> {
        self.inner
            .read()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
    pub fn entries_sorted(&self) -> Vec<(String, String)> {
        let mut entries: Vec<_> = self
            .entries()
            .into_iter()
            .map(|(key, value)| (key, value.to_string()))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    }
    fn save(&self, inner: &Map<String, Value>) -> Result<bool> {
        if self.path.as_os_str().is_empty() {
            return Ok(true);
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| PrefsError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_string_pretty(inner).map_err(PrefsError::Encode)?,
        )
        .map_err(|source| PrefsError::Write {
            path: tmp.clone(),
            source,
        })?;
        std::fs::rename(&tmp, &self.path).map_err(|source| PrefsError::Write {
            path: self.path.clone(),
            source,
        })?;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/prefs/mod.rs"]
mod tests;
