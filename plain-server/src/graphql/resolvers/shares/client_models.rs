use plain_rs::types::{Instant, Long};

use async_graphql::SimpleObject;
use plain_rs::{chat::messages::share_card, files::shares::client};
use serde::Deserialize;

#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SharedLink {
    host: String,
    port: i32,
    shared_id: async_graphql::ID,
    token: String,
    page_url: String,
}

#[derive(SimpleObject)]
struct SharedPeerAddress {
    id: async_graphql::ID,
    ip: String,
    port: i32,
}

#[derive(SimpleObject)]
struct SharedMessageCard {
    share_id: async_graphql::ID,
    url_token: String,
    peer_info: SharedPeerAddress,
    name: String,
    item_count: Long,
    total_size: Long,
    expires_at: Option<Instant>,
}

#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SharedEntry {
    name: String,
    virtual_path: String,
    is_dir: bool,
    size: Long,
    mime_type: String,
    has_thumb: bool,
}

#[derive(SimpleObject)]
pub(super) struct SharedLinkInfo {
    name: String,
    read_only: bool,
    requires_password: bool,
    expires_at: Option<Instant>,
    url_token: String,
    entries: Vec<SharedEntry>,
}

#[derive(SimpleObject)]
pub(super) struct SharedBrowseResult {
    info: SharedLinkInfo,
    link: SharedLink,
    card: SharedMessageCard,
}

impl From<client::Link> for SharedLink {
    fn from(row: client::Link) -> Self {
        Self {
            host: row.host,
            port: i32::from(row.port),
            shared_id: row.shared_id.into(),
            token: row.token,
            page_url: row.page_url,
        }
    }
}

impl From<share_card::Peer> for SharedPeerAddress {
    fn from(row: share_card::Peer) -> Self {
        Self {
            id: row.id.into(),
            ip: row.ip,
            port: i32::from(row.port),
        }
    }
}

impl From<share_card::Card> for SharedMessageCard {
    fn from(row: share_card::Card) -> Self {
        Self {
            share_id: row.share_id.into(),
            url_token: row.url_token,
            peer_info: row.peer_info.into(),
            name: row.name,
            item_count: Long(row.item_count),
            total_size: Long(row.total_size),
            expires_at: row.expires_at.map(Instant),
        }
    }
}

impl From<client::File> for SharedEntry {
    fn from(row: client::File) -> Self {
        Self {
            name: row.name,
            virtual_path: row.virtual_path,
            is_dir: row.is_dir,
            size: Long(row.size),
            mime_type: row.mime_type,
            has_thumb: row.has_thumb,
        }
    }
}

impl TryFrom<client::Info> for SharedLinkInfo {
    type Error = async_graphql::Error;
    fn try_from(row: client::Info) -> async_graphql::Result<Self> {
        Ok(Self {
            name: row.name,
            read_only: row.read_only,
            requires_password: row.requires_password,
            expires_at: row
                .expires_at
                .map(|value| {
                    chrono::DateTime::from_timestamp_millis(value)
                        .map(Instant)
                        .ok_or_else(|| async_graphql::Error::new("Invalid share expiry"))
                })
                .transpose()?,
            url_token: row.url_token,
            entries: row.entries.into_iter().map(Into::into).collect(),
        })
    }
}

impl TryFrom<plain_rs::services::peers::shared_client::BrowseResult> for SharedBrowseResult {
    type Error = async_graphql::Error;
    fn try_from(
        value: plain_rs::services::peers::shared_client::BrowseResult,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            info: value.info.try_into()?,
            link: value.link.into(),
            card: value.card.into(),
        })
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/shares/client_models.rs"]
mod tests;
