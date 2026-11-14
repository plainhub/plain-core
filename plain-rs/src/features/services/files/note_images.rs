use crate::runtime::Runtime as CoreRuntime;

pub async fn import(state: &CoreRuntime, source: &str, name: &str) -> Result<String, String> {
    if name.contains('\0') {
        return Err("Invalid image name".into());
    }
    let name = name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let name = name.trim_end_matches(['.', ' ']);
    let name = if name.is_empty() { "image.jpg" } else { name };
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
    {
        return Err("Reserved image name".into());
    }
    let temporary = crate::features::services::files::source_files::stage(
        source,
        state.db.clone(),
        &state.directory,
        &state.resources,
        &state.cache.image_cache(),
        state.stop.clone(),
    )
    .await?;
    let record = state
        .app_files
        .import(
            temporary.path().to_path_buf(),
            name.to_owned(),
            plain_libs::utils::mime::mime_from_ext(name).to_owned(),
            false,
        )
        .await?;
    Ok(format!(
        "fid:{}",
        record.real_path.rsplit('/').next().unwrap()
    ))
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/note_images.rs"]
mod tests;
