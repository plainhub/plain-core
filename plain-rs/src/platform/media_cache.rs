pub(crate) async fn clear(
    paths: &plain_rs::cache_paths::CachePaths,
    host: &plain_rs::host::bridge::Host,
) -> Result<(), String> {
    let mut directories = Vec::new();
    for path in [paths.image_cache(), paths.video_cache()] {
        match tokio::fs::symlink_metadata(&path).await {
            Ok(metadata) if metadata.is_dir() && !metadata.is_symlink() => directories.push(path),
            Ok(_) => return Err("Invalid media cache directory".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        if host
            .call_wait("systemClearMediaCache", serde_json::json!({}))
            .await?
            != true
        {
            return Err("Media cache clear failed".into());
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let _ = host;
    #[cfg(not(target_os = "android"))]
    for path in directories {
        #[cfg(target_os = "ios")]
        if path == paths.image_cache() {
            continue;
        }
        match tokio::fs::remove_dir_all(path).await {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            result => result.map_err(|error| error.to_string())?,
        }
    }
    Ok(())
}
