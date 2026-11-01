pub async fn resolve_native(
    state: &crate::runtime::Runtime,
    reference: &str,
) -> anyhow::Result<std::path::PathBuf> {
    state.native_client_id().map_err(anyhow::Error::msg)?;
    let path = crate::features::services::files::references::resolve(
        state.db.clone(),
        state.directory.clone(),
        reference,
    )?;
    let path = std::path::PathBuf::from(path);
    if !path.is_absolute() {
        anyhow::bail!("absolute native file path required");
    }
    state
        .file_platform
        .authorize(
            path.to_str()
                .ok_or_else(|| anyhow::anyhow!("invalid file path encoding"))?,
            false,
        )
        .await?;
    state.native_client_id().map_err(anyhow::Error::msg)?;
    Ok(path)
}
