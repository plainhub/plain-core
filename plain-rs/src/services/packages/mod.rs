pub mod query;

pub mod install;
pub mod manifest;
mod splits;

mod attributes;
mod framework;
mod input;
mod resources;
mod xml;

pub async fn read_manifest(
    services: &plain_rs::runtime::Runtime,
    path: &str,
) -> anyhow::Result<String> {
    let path = services.file_platform.resolve_reference(path)?;
    plain_rs::services::files::input::read_source(
        &services.resources,
        services.cache.root(),
        &path,
        |reader| manifest::xml(reader).map_err(|error| error.to_string()),
    )
    .await
    .map_err(anyhow::Error::msg)
}
