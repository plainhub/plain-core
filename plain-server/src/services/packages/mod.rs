pub(crate) mod query;

pub(crate) mod install;
pub(crate) mod manifest;
mod splits;

mod attributes;
mod framework;
mod input;
mod resources;
mod xml;

pub(crate) async fn read_manifest(
    services: &crate::assembly::services::Services,
    path: &str,
) -> anyhow::Result<String> {
    let path = services.file_platform.resolve_reference(path)?;
    crate::services::files::input::read_source(
        &services.resources,
        services.cache.root(),
        &path,
        |reader| manifest::xml(reader).map_err(|error| error.to_string()),
    )
    .await
    .map_err(anyhow::Error::msg)
}
