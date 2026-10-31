use std::path::{Path, PathBuf};
pub fn resolve(
    db: std::sync::Arc<plain_rs::db::Db>,
    directory: PathBuf,
    path: &str,
) -> anyhow::Result<String> {
    if path
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("fid:"))
    {
        return Ok(plain_rs::files::app_files::FileStore::new(db, directory)
            .resolve(&path[4..])
            .map_err(anyhow::Error::msg)?
            .to_string_lossy()
            .into_owned());
    }
    if path
        .get(..6)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("app://"))
    {
        anyhow::ensure!(directory.is_absolute(), "Absolute data directory required");
        let suffix = Path::new(&path[6..]);
        anyhow::ensure!(
            !path[6..].contains(['\\', '\0'])
                && !suffix.is_absolute()
                && suffix.components().all(|part| matches!(
                    part,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )),
            "Invalid app file path"
        );
        return Ok(directory.join(suffix).to_string_lossy().into_owned());
    }
    let path = plain_rs::utils::file_uri::local_path(path).map_err(anyhow::Error::msg)?;
    if Path::new(&path).is_absolute() || path.contains("://") {
        return Ok(path);
    }
    anyhow::ensure!(directory.is_absolute(), "Absolute data directory required");
    Ok(directory.join(path).to_string_lossy().into_owned())
}

pub fn resolve_many(
    paths: Vec<String>,
    mut resolve: impl FnMut(&str) -> Option<String>,
) -> Vec<Option<String>> {
    let mut values = std::collections::HashMap::new();
    paths
        .into_iter()
        .map(|path| {
            values
                .entry(path.clone())
                .or_insert_with(|| resolve(&path))
                .clone()
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/references.rs"]
mod tests;
