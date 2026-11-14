pub async fn read(
    resources: &crate::infrastructure::host::resources::NativeResources,
    directory: &std::path::Path,
    path: &str,
) -> Result<Option<Vec<u8>>, String> {
    crate::features::services::files::input::read_source(resources, directory, path, |reader| {
        Ok(
            plain_libs::media::cover::extract_embedded_cover_reader(reader)
                .map(|cover| cover.bytes),
        )
    })
    .await
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/audio/covers.rs"]
mod tests;
