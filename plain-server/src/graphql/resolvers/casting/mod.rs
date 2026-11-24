pub mod mutation;
pub mod query;

use plain_graphql::{Result, SimpleObject};
use plain_rs::features::services::casting::dlna_sender_runtime as sender;
use plain_rs::features::services::casting::dlna_sender_runtime::DlnaCastType;
use plain_rs::infrastructure::types::Long;

#[derive(SimpleObject, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DlnaSenderDevice {
    id: plain_graphql::ID,
    host_address: String,
    name: String,
    location: String,
}

impl From<sender::Device> for DlnaSenderDevice {
    fn from(row: sender::Device) -> Self {
        Self {
            id: row.id.into(),
            host_address: row.host_address,
            name: row.name,
            location: row.location,
        }
    }
}

#[derive(SimpleObject, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DlnaSenderMedia {
    path: String,
    title: String,
    album_art: String,
    artist: String,
    duration_ms: Long,
    #[graphql(name = "type")]
    #[serde(rename = "type")]
    media_type: DlnaCastType,
}

impl From<sender::Item> for DlnaSenderMedia {
    fn from(row: sender::Item) -> Self {
        Self {
            path: row.path,
            title: row.title,
            album_art: row.album_art,
            artist: row.artist,
            duration_ms: Long(row.duration_ms),
            media_type: row.media_type,
        }
    }
}

#[derive(SimpleObject, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DlnaSenderState {
    version: Long,
    devices: Vec<DlnaSenderDevice>,
    current_device: Option<DlnaSenderDevice>,
    items: Vec<DlnaSenderMedia>,
    current_uri: String,
    playing: bool,
    progress_ms: Long,
    duration_ms: Long,
    supports_callback: bool,
}

impl TryFrom<sender::Snapshot> for DlnaSenderState {
    type Error = plain_graphql::Error;
    fn try_from(row: sender::Snapshot) -> Result<Self> {
        Ok(Self {
            version: Long(row.version.try_into()?),
            devices: row.devices.into_iter().map(Into::into).collect(),
            current_device: row.current_device.map(Into::into),
            items: row.items.into_iter().map(Into::into).collect(),
            current_uri: row.current_uri,
            playing: row.playing,
            progress_ms: Long(row.progress_ms),
            duration_ms: Long(row.duration_ms),
            supports_callback: row.supports_callback,
        })
    }
}
