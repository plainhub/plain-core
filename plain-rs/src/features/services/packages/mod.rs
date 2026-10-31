pub mod query;

pub mod install;
mod splits;

pub async fn read_manifest(
    services: &crate::runtime::Runtime,
    path: &str,
) -> anyhow::Result<String> {
    let path = services.file_platform.resolve_reference(path)?;
    crate::features::services::files::input::read_source(
        &services.resources,
        services.cache.root(),
        &path,
        |reader| crate::libs::apk::manifest::xml(reader).map_err(|error| error.to_string()),
    )
    .await
    .map_err(anyhow::Error::msg)
}
