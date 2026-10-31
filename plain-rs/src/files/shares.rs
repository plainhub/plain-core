pub mod batch_plan;
pub mod client;
pub mod summary;
use crate::db::Db;
use crate::db::ShareRow;
use crate::prefs::Prefs;
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "graphql", derive(async_graphql::SimpleObject))]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub relative_path: String,
    pub real_path: String,
    pub is_dir: bool,
}

pub struct Service {
    db: Arc<Db>,
    prefs: Arc<Prefs>,
}
impl Service {
    pub fn new(db: Arc<Db>, prefs: Arc<Prefs>) -> Arc<Self> {
        Arc::new(Self { db, prefs })
    }
    pub fn roots(row: &ShareRow) -> anyhow::Result<Vec<Root>> {
        Ok(serde_json::from_str(&row.data)?)
    }
    fn build_roots(paths: Vec<String>) -> anyhow::Result<String> {
        anyhow::ensure!(!paths.is_empty(), "share needs at least one root");
        let mut seen = HashSet::new();
        let mut names = HashSet::new();
        let mut roots = Vec::new();
        for path in paths {
            let path = std::fs::canonicalize(path)?;
            if !seen.insert(path.clone()) {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow::anyhow!("invalid root name"))?;
            anyhow::ensure!(names.insert(name.to_owned()), "duplicate root name: {name}");
            let meta = path.metadata()?;
            anyhow::ensure!(meta.is_dir() || meta.is_file(), "unsupported share root");
            roots.push(Root {
                relative_path: format!("{name}{}", if meta.is_dir() { "/" } else { "" }),
                real_path: path
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("invalid root path"))?
                    .to_owned(),
                is_dir: meta.is_dir(),
            });
        }
        Ok(serde_json::to_string(&roots)?)
    }
    pub fn create(
        &self,
        name: String,
        paths: Vec<String>,
        read_only: bool,
        expires: Option<String>,
    ) -> anyhow::Result<ShareRow> {
        validate_expiry(expires.as_deref())?;
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let row = ShareRow {
            id: crate::short_uuid::short_uuid(),
            name,
            password: String::new(),
            url_token: crate::crypto::gen_token(),
            read_only,
            expires_at: expires,
            data: Self::build_roots(paths)?,
            created_at: now.clone(),
            updated_at: now,
        };
        self.db.share_save(&row)?;
        Ok(row)
    }
    pub fn update(
        &self,
        id: &str,
        name: &str,
        expires: Option<String>,
        paths: Option<Vec<String>>,
    ) -> anyhow::Result<ShareRow> {
        validate_expiry(expires.as_deref())?;
        let data = paths.map(Self::build_roots).transpose()?;
        anyhow::ensure!(
            self.db.share_update(
                id,
                name,
                expires.as_deref(),
                data.as_deref(),
                &chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            )?,
            "share not found"
        );
        self.db
            .share_get(id)?
            .ok_or_else(|| anyhow::anyhow!("share not found"))
    }
    pub fn active(&self, id: &str, guest: bool) -> anyhow::Result<Option<ShareRow>> {
        if guest
            && !self
                .prefs
                .get::<bool>(crate::prefs::pref_keys::DESKTOP_ACCESS)?
                .unwrap_or(true)
        {
            return Ok(None);
        }
        Ok(self
            .db
            .share_get(id)?
            .filter(|r| match r.expires_at.as_deref() {
                None => true,
                Some(t) => {
                    chrono::DateTime::parse_from_rfc3339(t).is_ok_and(|t| t > chrono::Utc::now())
                }
            }))
    }
    pub fn token(&self, id: &str) -> anyhow::Result<String> {
        anyhow::ensure!(self.db.share_get(id)?.is_some(), "share not found");
        let secret = self
            .prefs
            .get::<String>(crate::prefs::pref_keys::MASTER_SECRET)?
            .ok_or_else(|| anyhow::anyhow!("master secret not initialized"))?;
        let key = crate::utils::base64::base64_decode_checked(&secret)?;
        anyhow::ensure!(key.len() == 32, "invalid master secret");
        let mut mac = Hmac::<sha2_mac::Sha256>::new_from_slice(&key)?;
        mac.update(id.as_bytes());
        Ok(crate::utils::base64::base64_encode_url_safe(
            &mac.finalize().into_bytes(),
        ))
    }
    pub fn resolve(
        &self,
        id: &str,
        relative_path: &str,
        guest: bool,
    ) -> anyhow::Result<Option<String>> {
        let Some(row) = self.active(id, guest)? else {
            return Ok(None);
        };
        Self::resolve_row(&row, relative_path)
    }
    fn resolve_row(row: &ShareRow, relative_path: &str) -> anyhow::Result<Option<String>> {
        let roots = Self::roots(row)?;
        let normalized = relative_path.trim().trim_start_matches('/');
        let (top, rest) = normalized.split_once('/').unwrap_or((normalized, ""));
        let root = if normalized.is_empty() {
            roots.first()
        } else {
            roots
                .iter()
                .find(|r| r.relative_path.trim_end_matches('/') == top)
        };
        let Some(root) = root else { return Ok(None) };
        let base = Path::new(&root.real_path);
        if !root.is_dir && !rest.is_empty() {
            return Ok(None);
        };
        let mut relative = PathBuf::new();
        for part in Path::new(rest).components() {
            match part {
                Component::Normal(p) => relative.push(p),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !relative.pop() {
                        return Ok(None);
                    }
                }
                _ => return Ok(None),
            }
        }
        // Joining an empty path adds a directory separator, which makes file
        // roots fail to canonicalize on Unix.
        let path = if relative.as_os_str().is_empty() {
            base.to_path_buf()
        } else {
            base.join(relative)
        };
        let candidate = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };
        if !candidate.starts_with(base) {
            return Ok(None);
        };
        Ok(candidate.to_str().map(str::to_owned))
    }
    pub fn browse(&self, id: &str, relative_path: &str) -> anyhow::Result<(ShareRow, Vec<Root>)> {
        let row = self
            .active(id, true)?
            .ok_or_else(|| anyhow::anyhow!("share inactive"))?;
        let mut entries = Vec::new();
        if relative_path.trim().trim_matches('/').is_empty() {
            for root in Self::roots(&row)? {
                if Self::resolve_row(&row, &root.relative_path)?.is_some() {
                    entries.push(root);
                }
            }
        } else {
            let path = Self::resolve_row(&row, relative_path)?
                .ok_or_else(|| anyhow::anyhow!("path not allowed"))?;
            if Path::new(&path).is_dir() {
                let parent = relative_path.trim().trim_matches('/');
                for entry in std::fs::read_dir(path)? {
                    let entry = entry?;
                    let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                        continue;
                    };
                    if name.starts_with('.') {
                        continue;
                    }
                    let relative_path = format!("{parent}/{name}");
                    let Some(real_path) = Self::resolve_row(&row, &relative_path)? else {
                        continue;
                    };
                    let meta = Path::new(&real_path).metadata()?;
                    if !meta.is_dir() && !meta.is_file() {
                        continue;
                    }
                    entries.push(Root {
                        relative_path,
                        real_path,
                        is_dir: meta.is_dir(),
                    });
                }
                entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
            }
        }
        Ok((row, entries))
    }

    pub fn archive(&self, id: &str, encrypted: &str) -> anyhow::Result<Vec<Root>> {
        let path = self
            .resolve_file(id, encrypted)?
            .ok_or_else(|| anyhow::anyhow!("share path not allowed"))?;
        let base = PathBuf::from(path);
        anyhow::ensure!(base.is_dir(), "shared path is not a directory");
        let mut pending = vec![base.clone()];
        let mut result = Vec::new();
        while let Some(dir) = pending.pop() {
            anyhow::ensure!(self.active(id, true)?.is_some(), "share inactive");
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                let meta = path.symlink_metadata()?;
                if meta.file_type().is_symlink() {
                    continue;
                }
                let canonical = std::fs::canonicalize(&path)?;
                anyhow::ensure!(canonical.starts_with(&base), "archive path outside root");
                if meta.is_dir() {
                    anyhow::ensure!(result.len() < 100_000, "too many archive entries");
                    result.push(Root {
                        relative_path: format!(
                            "{}/",
                            path.strip_prefix(&base)?
                                .to_str()
                                .ok_or_else(|| anyhow::anyhow!("invalid entry name"))?
                        ),
                        real_path: canonical
                            .to_str()
                            .ok_or_else(|| anyhow::anyhow!("invalid entry path"))?
                            .into(),
                        is_dir: true,
                    });
                    pending.push(canonical);
                } else if meta.is_file() {
                    anyhow::ensure!(result.len() < 100_000, "too many archive files");
                    result.push(Root {
                        relative_path: path
                            .strip_prefix(&base)?
                            .to_str()
                            .ok_or_else(|| anyhow::anyhow!("invalid entry name"))?
                            .into(),
                        real_path: canonical
                            .to_str()
                            .ok_or_else(|| anyhow::anyhow!("invalid entry path"))?
                            .into(),
                        is_dir: false,
                    });
                }
            }
        }
        result.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(result)
    }

    pub fn resolve_file(&self, id: &str, encrypted: &str) -> anyhow::Result<Option<String>> {
        let Some(row) = self.active(id, true)? else {
            return Ok(None);
        };
        let Ok((scope, payload)) = crate::files::resource_link::parse(encrypted) else {
            return Ok(None);
        };
        if scope != crate::files::resource_link::Scope::Share(id.into()) {
            return Ok(None);
        }
        let key = crate::utils::base64::base64_decode_checked(&row.url_token)?;
        let Ok(resource) = crate::files::resource_link::open(&scope, &key, payload) else {
            return Ok(None);
        };
        Self::resolve_row(&row, &resource.id)
    }
}
fn validate_expiry(value: Option<&str>) -> anyhow::Result<()> {
    if let Some(v) = value {
        chrono::DateTime::parse_from_rfc3339(v)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/files/shares.rs"]
mod tests;

pub mod batch_queue;
