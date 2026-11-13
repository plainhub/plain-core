pub use super::metadata::{Entry, Metadata};
use super::{metadata, transfer};
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::{
    anyhow::{Result, ensure},
    protocols::http_client::Url,
};
use std::time::Duration;
pub(crate) mod access;
mod execute;
mod plan;
pub(super) use execute::execute;

pub fn url(state: &CoreRuntime, raw: &str) -> Result<Url> {
    let url = if raw.starts_with("/fs?") {
        Url::parse(&plain_libs::utils::build_url::build_url(
            "https",
            "127.0.0.1",
            state.prefs.get::<u16>("https_port")?.unwrap_or(8443),
            raw,
        ))?
    } else {
        Url::parse(raw)?
    };
    ensure!(
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
            && url.path() == "/fs",
        "Invalid download resource URL"
    );
    let fields: Vec<_> = url.query_pairs().collect();
    ensure!(
        fields.len() == 2
            && fields.iter().filter(|(key, _)| key == "id").count() == 1
            && fields
                .iter()
                .any(|(key, value)| key == "action" && value == "download"),
        "Original resource URL required"
    );
    crate::features::files::resource_link::parse(
        fields
            .iter()
            .find(|(key, _)| key == "id")
            .unwrap()
            .1
            .as_ref(),
    )?;
    Ok(url)
}

pub fn is_local(state: &CoreRuntime, url: &Url) -> Result<bool> {
    let port = state.prefs.get::<u16>("https_port")?.unwrap_or(8443);
    if url.port_or_known_default() != Some(port) {
        return Ok(false);
    }
    Ok(url.host_str().is_some_and(|host| {
        host == "127.0.0.1"
            || host == "[::1]"
            || crate::infrastructure::transport::lan::local_interfaces()
                .iter()
                .any(|interface| host == interface.ip.to_string())
    }))
}

pub async fn metadata(
    state: &CoreRuntime,
    raw: &str,
    expected: &[crate::infrastructure::db::DPeer],
) -> Result<Metadata> {
    let url = url(state, raw)?;
    let id = url
        .query_pairs()
        .find(|(key, _)| key == "id")
        .unwrap()
        .1
        .into_owned();
    let mut metadata = if is_local(state, &url)? {
        metadata::describe(state, &id).await?
    } else {
        let _permit = state.lan.capacity.clone().acquire_owned().await?;
        let read = access::get(
            state,
            raw,
            crate::features::services::files::content::Action::Describe,
            expected,
        )
        .await?;
        let data =
            crate::features::services::peers::peer_lan::limited(read.response, 16 * 1024 * 1024)
                .await?;
        if let Some(route) = read.route {
            route.check_current(state)?;
        }
        plain_libs::serde_json::from_slice(&data)?
    };
    for entry in std::iter::once(&mut metadata.entry).chain(metadata.children.iter_mut()) {
        ensure!(entry.size_bytes >= 0, "Invalid resource size");
        let child = url.join(&entry.url)?;
        ensure!(child.origin() == url.origin(), "Resource origin changed");
        entry.url = child.to_string();
    }
    Ok(metadata)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/downloads/resources.rs"]
mod tests;
