use crate::runtime::Runtime as CoreRuntime;

pub async fn import(state: &CoreRuntime, source: &str) -> Result<String, String> {
    let destination = crate::libs::data_paths::DataPaths::new(&state.directory).pomodoro_sound();
    let temporary = crate::features::services::files::source_files::stage(
        source,
        state.db.clone(),
        &state.directory,
        &state.resources,
        destination.parent().ok_or("Invalid sound directory")?,
        state.stop.clone(),
    )
    .await?;
    temporary
        .persist(&destination)
        .map_err(|error| error.to_string())?;
    Ok("app://audio/pomodoro_sound.mp3".into())
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/services/pomodoro_sound.rs"]
mod tests;
