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
    let directory = crate::libs::data_paths::DataPaths::new(&state.directory).note_images();
    let temporary = crate::features::services::files::source_files::stage(
        source,
        state.db.clone(),
        &state.directory,
        &state.resources,
        &directory,
        state.stop.clone(),
    )
    .await?;
    let target = directory.join(name);
    tokio::task::spawn_blocking(move || {
        let mut temporary = temporary;
        loop {
            let destination = crate::libs::utils::unique_path::unique_sibling(&target)
                .map_err(|error| error.to_string())?;
            match temporary.persist_noclobber(&destination) {
                Ok(_) => {
                    return Ok(format!(
                        "app://note-images/{}",
                        destination
                            .file_name()
                            .and_then(|value| value.to_str())
                            .ok_or("Invalid image name")?
                    ));
                }
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    temporary = error.file
                }
                Err(error) => return Err(error.to_string()),
            }
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/note_images.rs"]
mod tests;
