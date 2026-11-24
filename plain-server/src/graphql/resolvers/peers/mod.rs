pub mod actions;
pub mod records;

pub mod mutation;
pub mod query;

use plain_graphql::{InputObject, SimpleObject};
pub use plain_rs::features::chat::enums::PeerStatus;
use plain_rs::features::services::device::types::DeviceType;
pub use plain_rs::features::services::discovery::pairing_attempts::Mode as PairingMode;

#[derive(SimpleObject, Clone, Debug)]
pub struct Peer {
    pub id: plain_graphql::ID,
    pub name: String,
    pub ip: String,
    pub status: PeerStatus,
    pub port: Option<i32>,
    #[graphql(name = "deviceType")]
    pub device_type: DeviceType,
    #[graphql(name = "createdAt")]
    pub created_at: plain_rs::infrastructure::types::Instant,
    #[graphql(name = "updatedAt")]
    pub updated_at: plain_rs::infrastructure::types::Instant,
    pub online: bool,
    pub managed: bool,
    #[graphql(guard = "crate::http::auth::public_gate::LocalOwner")]
    pub key: String,
    pub public_key: String,
    #[graphql(guard = "crate::http::auth::public_gate::LocalOwner")]
    pub token: String,
}

impl Peer {
    pub(crate) fn from_record(row: plain_rs::infrastructure::db::DPeer, online: bool) -> Self {
        let managed = row.is_paired() && !row.token.is_empty();
        Self {
            key: row.key,
            public_key: row.public_key,
            token: row.token,
            online,
            managed,
            id: row.id.into(),
            name: row.name,
            ip: row.ip,
            status: row.status,
            port: row.port.map(i32::from),
            device_type: row.device_type.into(),
            created_at: crate::graphql::types::facts::stored_instant(&row.created_at),
            updated_at: crate::graphql::types::facts::stored_instant(&row.updated_at),
        }
    }
}

#[derive(InputObject, Clone)]
pub struct PairPeerInput {
    pub mode: PairingMode,
    pub password: Option<String>,
}

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/peers/public.rs"]
mod tests;
