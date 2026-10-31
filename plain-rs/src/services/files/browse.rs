use plain_rs::files::filesystem::{browse::Request, record::FileRecord};
use serde_json::Value;

pub async fn execute(
    platform: &plain_rs::services::files::platform::FilePlatform,
    request: Request,
) -> Result<Value, String> {
    let plan = request.plan().map_err(|e| e.to_string())?;
    let page = if let Some(archive) = plan.archive_path() {
        authorize(platform, archive).await?;
        let reference = plan.root.clone();
        let entries = tokio::task::spawn_blocking(move || {
            plain_rs::services::files::archive_listing::list(&reference)
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
        plan.archive_page(entries).map_err(|e| e.to_string())?
    } else {
        authorize(platform, &plan.root).await?;
        tokio::task::spawn_blocking(move || plan.execute())
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?
    };
    serde_json::to_value(page).map_err(|e| e.to_string())
}

async fn authorize(
    platform: &plain_rs::services::files::platform::FilePlatform,
    path: &str,
) -> Result<(), String> {
    platform
        .authorize(path, false)
        .await
        .map_err(|error| error.to_string())
}

pub async fn stat_record(
    platform: &plain_rs::services::files::platform::FilePlatform,
    path: &str,
) -> Option<FileRecord> {
    if path.trim().is_empty() || path == "." {
        return None;
    }
    let path = platform.resolve_reference(path).ok()?;
    if !std::path::Path::new(&path).is_absolute() {
        return None;
    }
    if authorize(platform, &path).await.is_err() {
        return None;
    }
    tokio::task::spawn_blocking(move || FileRecord::stat(std::path::Path::new(&path)).ok())
        .await
        .ok()
        .flatten()
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/browse.rs"]
mod tests;
