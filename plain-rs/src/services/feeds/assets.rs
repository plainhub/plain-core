use futures_util::StreamExt;
use plain_rs::db::Db;
use plain_rs::error::DomainError;
use plain_rs::error::DomainResult;
use plain_rs::utils::html_to_markdown::dom::Dom;
use plain_rs::utils::http_url::join;
use plain_rs::utils::http_url::parse_http_url;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::Notify;

#[derive(Default)]
struct Writes {
    pending: AtomicUsize,
    finished: Notify,
}
struct Write(Arc<Writes>);
impl Drop for Write {
    fn drop(&mut self) {
        if self.0.pending.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.0.finished.notify_waiters();
        }
    }
}
struct ImportedAsset {
    db: Arc<Db>,
    directory: PathBuf,
    file: plain_rs::chat::attachments::app_file_store::ImportResult,
    retained: bool,
    _write: Write,
}
impl ImportedAsset {
    fn retain(mut self) -> String {
        self.retained = true;
        self.file.real_path.to_string_lossy().into_owned()
    }
}
impl Drop for ImportedAsset {
    fn drop(&mut self) {
        if !self.retained {
            if let Err(error) = plain_rs::chat::attachments::app_file_store::release(
                &self.db,
                &self.directory,
                &self.file.id,
            ) {
                log::warn!("Cancelled feed asset release failed: {error}");
            }
        }
    }
}
pub struct FeedAssets {
    pub db: Arc<Db>,
    pub directory: PathBuf,
    writes: Arc<Writes>,
}
impl FeedAssets {
    pub fn new(db: Arc<Db>, directory: PathBuf) -> Self {
        Self {
            db,
            directory,
            writes: Arc::new(Writes::default()),
        }
    }
    pub(super) fn scoped(&self) -> Self {
        Self::new(self.db.clone(), self.directory.clone())
    }
    pub(super) async fn wait_writes(&self) {
        loop {
            let finished = self.writes.finished.notified();
            tokio::pin!(finished);
            finished.as_mut().enable();
            if self.writes.pending.load(Ordering::Acquire) == 0 {
                return;
            }
            finished.await;
        }
    }
    async fn import(
        &self,
        operation: impl FnOnce(
            &Db,
            &Path,
        ) -> std::io::Result<
            plain_rs::chat::attachments::app_file_store::ImportResult,
        > + Send
        + 'static,
    ) -> DomainResult<String> {
        self.writes.pending.fetch_add(1, Ordering::AcqRel);
        let write = Write(self.writes.clone());
        let db = self.db.clone();
        let directory = self.directory.clone();
        tokio::task::spawn_blocking(move || {
            let file = operation(&db, &directory)?;
            Ok::<_, std::io::Error>(ImportedAsset {
                db,
                directory,
                file,
                retained: false,
                _write: write,
            })
        })
        .await
        .map_err(|error| DomainError::Other(error.to_string()))?
        .map(ImportedAsset::retain)
        .map_err(|error| DomainError::Other(error.to_string()))
    }

    pub async fn cache(&self, url: &str) -> DomainResult<String> {
        self.cache_formats(url, false).await
    }
    pub async fn cache_bookmark_icon(&self, url: &str) -> DomainResult<String> {
        self.cache_formats(url, true).await
    }
    async fn cache_formats(&self, url: &str, bookmark: bool) -> DomainResult<String> {
        let response = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| DomainError::Other(e.to_string()))?
            .get(url)
            .send()
            .await
            .map_err(|e| DomainError::Other(e.to_string()))?
            .error_for_status()
            .map_err(|e| DomainError::Other(e.to_string()))?;
        let mime = response
            .headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok())
            .unwrap_or_default()
            .split(';')
            .next()
            .unwrap_or_default()
            .to_lowercase();
        if !["image/png", "image/jpeg", "image/webp"].contains(&mime.as_str())
            && !(bookmark
                && ["image/x-icon", "image/vnd.microsoft.icon", "image/svg+xml"]
                    .contains(&mime.as_str()))
        {
            return Err(DomainError::Other("unsupported feed image format".into()));
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| DomainError::Other(e.to_string()))?;
            if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
                return Err(DomainError::Other("feed image too large".into()));
            }
            bytes.extend_from_slice(&chunk);
        }
        self.import(move |db, directory| {
            plain_rs::chat::attachments::app_file_store::import_bytes(db, directory, &bytes, &mime)
        })
        .await
    }

    pub fn release(&self, uri: &str) {
        let path = std::path::Path::new(uri);
        let Some(id) = path.file_stem().and_then(|v| v.to_str()) else {
            return;
        };
        let Some(file) = self.db.get_app_file(id) else {
            return;
        };
        let owned = self.directory.join(&file.real_path);
        if path != owned || !owned.starts_with(self.directory.join("files")) {
            return;
        }
        if let Err(error) =
            plain_rs::chat::attachments::app_file_store::release(&self.db, &self.directory, id)
        {
            log::warn!("feed asset release failed: {error}");
        }
    }
}
/// Resolves `reference` against `base`, keeping only http(s). A `javascript:`
/// or `data:` reference must stay as it is instead of turning into a
/// site-relative path.
pub fn absolute_url(base: &str, reference: &str) -> Option<String> {
    let reference = reference.trim();
    if reference.is_empty() {
        return None;
    }
    if let Some(scheme) = scheme_of(reference) {
        return ["http", "https"]
            .contains(&scheme.as_str())
            .then(|| reference.to_string());
    }
    if let Some(rest) = reference.strip_prefix("//") {
        return Some(format!("{}://{rest}", parse_http_url(base)?.scheme));
    }
    join(base, reference)
}

fn scheme_of(reference: &str) -> Option<String> {
    let colon = reference.find(':')?;
    let scheme = &reference[..colon];
    let valid = !scheme.is_empty()
        && !scheme.contains('/')
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.');
    valid.then(|| scheme.to_ascii_lowercase())
}

/// Rewrites relative `src`/`href` references to absolute ones, so article
/// HTML keeps working after it is read outside the page it came from.
pub fn normalize_html(html: &str, base: &str) -> String {
    let mut dom = Dom::parse(html);
    for node in &mut dom.0 {
        for (name, value) in node.attrs.iter_mut() {
            if matches!(name.as_str(), "src" | "href")
                && let Some(url) = absolute_url(base, value)
            {
                *value = url;
            }
        }
    }
    plain_rs::services::feeds::inner_html(&dom, 0)
}

pub fn main_image(html: &str, base: &str) -> Option<String> {
    Dom::parse(html)
        .0
        .iter()
        .filter(|node| node.tag == "img")
        .find_map(|node| absolute_url(base, node.attr("src")))
}

#[cfg(test)]
#[path = "../../../tests/unit/services/feeds/assets.rs"]
mod tests;
