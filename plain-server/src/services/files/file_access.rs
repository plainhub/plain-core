pub(crate) async fn resolve_native(
    state: &crate::assembly::server::ServerState,
    reference: &str,
) -> Result<std::path::PathBuf, String> {
    state.native_client_id()?;
    let path = crate::services::files::references::resolve(state.db.clone(), state.directory.clone(), reference)
        .map_err(|error| error.to_string())?;
    let path = std::path::PathBuf::from(path);
    if !path.is_absolute() {
        return Err("absolute native file path required".into());
    }
    state
        .file_platform
        .authorize(path.to_str().ok_or("invalid file path encoding")?, false)
        .await
        .map_err(|error| error.to_string())?;
    state.native_client_id()?;
    Ok(path)
}
