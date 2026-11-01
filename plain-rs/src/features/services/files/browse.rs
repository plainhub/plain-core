use crate::libs::filesystem::{browse::Request, record::FileRecord};
use serde_json::Value;

pub async fn execute(
    platform: &crate::features::services::files::platform::FilePlatform,
    request: Request,
) -> Result<Value, String> {
    let plan = request.plan().map_err(|e| e.to_string())?;
    let page = if let Some(archive) = plan.archive_path() {
        authorize(platform, archive).await?;
        let reference = plan.root.clone();
        let entries = tokio::task::spawn_blocking(move || {
            crate::features::services::files::archive_listing::list(&reference)
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

pub async fn search(
    platform: &crate::features::services::files::platform::FilePlatform,
    request: Request,
) -> Result<
    tokio::sync::mpsc::Receiver<Result<(crate::libs::filesystem::browse::Page, bool), String>>,
    String,
> {
    let plan = request.plan().map_err(|error| error.to_string())?;
    if plan.archive_path().is_some() {
        return Err("Directory search requires a filesystem directory".into());
    }
    authorize(platform, &plan.root).await?;
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    tokio::task::spawn_blocking(move || {
        let result = plan.walk(
            |page| sender.blocking_send(Ok((page, false))).is_ok(),
            || sender.is_closed(),
        );
        if !sender.is_closed() {
            let result = result
                .map(|page| (page, true))
                .map_err(|error| error.to_string());
            let _ = sender.blocking_send(result);
        }
    });
    Ok(receiver)
}

async fn authorize(
    platform: &crate::features::services::files::platform::FilePlatform,
    path: &str,
) -> Result<(), String> {
    platform
        .authorize(path, false)
        .await
        .map_err(|error| error.to_string())
}

pub async fn stat_record(
    platform: &crate::features::services::files::platform::FilePlatform,
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
#[path = "../../../../../tests/plain-rs/unit/features/services/files/browse.rs"]
mod tests;
