use plain_rs::types::{Instant, Long};

use async_graphql::{ID, InputObject};
use chrono::SubsecRound;
use plain_rs::{chat::messages::share_card, files::shares::client};

#[derive(InputObject)]
pub(super) struct SharedLinkInput {
    host: String,
    port: i32,
    shared_id: ID,
    token: String,
    page_url: Option<String>,
}

#[derive(InputObject)]
struct SharedPeerAddressInput {
    id: ID,
    ip: String,
    port: i32,
}

#[derive(InputObject)]
pub(super) struct SharedMessageCardInput {
    share_id: ID,
    url_token: String,
    peer_info: SharedPeerAddressInput,
    name: String,
    item_count: Long,
    total_size: Long,
    expires_at: Option<Instant>,
}

#[derive(InputObject)]
pub(super) struct SharedEntryInput {
    name: String,
    relative_path: String,
    is_dir: bool,
    size: Long,
    mime_type: String,
    has_thumb: bool,
}

fn port(value: i32) -> async_graphql::Result<u16> {
    u16::try_from(value).map_err(|_| async_graphql::Error::new("Invalid share port"))
}

impl TryFrom<SharedLinkInput> for client::Link {
    type Error = async_graphql::Error;
    fn try_from(value: SharedLinkInput) -> Result<Self, Self::Error> {
        Ok(Self {
            host: value.host,
            port: port(value.port)?,
            shared_id: value.shared_id.0,
            token: value.token,
            page_url: value.page_url.unwrap_or_default(),
        })
    }
}

impl TryFrom<SharedMessageCardInput> for share_card::Card {
    type Error = async_graphql::Error;
    fn try_from(value: SharedMessageCardInput) -> Result<Self, Self::Error> {
        Ok(Self {
            share_id: value.share_id.0,
            url_token: value.url_token,
            peer_info: share_card::Peer {
                id: value.peer_info.id.0,
                ip: value.peer_info.ip,
                port: port(value.peer_info.port)?,
            },
            name: value.name,
            item_count: value.item_count.0,
            total_size: value.total_size.0,
            expires_at: value.expires_at.map(|at| at.0.trunc_subsecs(3)),
        })
    }
}

impl From<SharedEntryInput> for client::File {
    fn from(value: SharedEntryInput) -> Self {
        Self {
            name: value.name,
            relative_path: value.relative_path,
            is_dir: value.is_dir,
            size: value.size.0,
            mime_type: value.mime_type,
            has_thumb: value.has_thumb,
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/shares/client_inputs.rs"]
mod tests;
