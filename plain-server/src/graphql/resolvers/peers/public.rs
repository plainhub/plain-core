//! Public peer, SIM and pairing roots.

use crate::graphql::resolvers::device::public::device_type as parse_device_type;
use crate::http::auth::public_gate;
use async_graphql::{Context, Enum, InputObject, Object, SimpleObject};
use plain_rs::host::bridge::Host;
use plain_rs::prefs::Prefs;
use plain_rs::services::device::types::DeviceType;
use serde_json::{Value, json};
use std::sync::Arc;

pub use plain_rs::chat::enums::PeerStatus;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum DiscoveryMethod {
    Lan,
    Ble,
    Qr,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct Peer {
    pub id: async_graphql::ID,
    pub name: String,
    pub ip: String,
    pub status: PeerStatus,
    pub port: Option<i32>,
    #[graphql(name = "deviceType")]
    pub device_type: DeviceType,
    #[graphql(name = "createdAt")]
    pub created_at: plain_rs::types::Instant,
    #[graphql(name = "updatedAt")]
    pub updated_at: plain_rs::types::Instant,
    pub online: bool,
    #[graphql(guard = "crate::http::auth::public_gate::LocalOwner")]
    pub key: String,
    pub public_key: String,
    #[graphql(guard = "crate::http::auth::public_gate::LocalOwner")]
    pub token: String,
    pub address: Option<super::records::PeerAddress>,
}

impl Peer {
    pub(crate) fn from_record(
        row: plain_rs::db::DPeer,
        online: bool,
        local: &[plain_rs::transport::lan::Interface],
    ) -> Self {
        Self {
            address: Some(
                plain_rs::services::peers::peer_address::address_with_interfaces(&row, local)
                    .into(),
            ),
            key: row.key,
            public_key: row.public_key,
            token: row.token,
            online,
            id: row.id.into(),
            name: row.name,
            ip: row.ip,
            status: match row.status {
                plain_rs::chat::enums::PeerStatus::Paired => PeerStatus::Paired,
                plain_rs::chat::enums::PeerStatus::Unpaired => PeerStatus::Unpaired,
                plain_rs::chat::enums::PeerStatus::Channel => PeerStatus::Channel,
            },
            port: row.port.map(i32::from),
            device_type: parse_device_type(&row.device_type.to_string()),
            created_at: crate::graphql::types::facts::stored_instant(&row.created_at),
            updated_at: crate::graphql::types::facts::stored_instant(&row.updated_at),
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct Sim {
    pub id: async_graphql::ID,
    pub label: String,
    pub number: String,
    #[graphql(name = "subscriptionId")]
    pub subscription_id: i32,
}

#[derive(InputObject, Clone)]
pub struct PairingDeviceInput {
    pub id: Option<async_graphql::ID>,
    pub host: Option<String>,
    pub mode: Option<PairingMode>,
    pub password: Option<String>,
    pub generation: Option<String>,
}

#[derive(InputObject, Clone, Debug)]
pub struct PairingRequestInput {
    #[graphql(name = "fromId")]
    pub from_id: async_graphql::ID,
    #[graphql(name = "fromName")]
    pub from_name: String,
    pub port: i32,
    #[graphql(name = "deviceType")]
    pub device_type: DeviceType,
    #[graphql(name = "ecdhPublicKey")]
    pub ecdh_public_key: String,
    #[graphql(name = "signaturePublicKey")]
    pub signature_public_key: String,
    pub timestamp: plain_rs::types::Long,
    pub ips: Vec<String>,
    pub signature: String,
    #[graphql(name = "fromIp")]
    pub from_ip: String,
    #[graphql(name = "awareSupported")]
    pub aware_supported: bool,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PairingMode {
    Chat,
    Manage,
}
impl From<PairingMode> for plain_rs::services::discovery::pairing_attempts::Mode {
    fn from(mode: PairingMode) -> Self {
        match mode {
            PairingMode::Chat => Self::Chat,
            PairingMode::Manage => Self::Manage,
        }
    }
}
#[derive(SimpleObject, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingState {
    device_id: Option<async_graphql::ID>,
    generation: String,
    mode: PairingMode,
    phase: String,
    error: Option<String>,
    verification_code: Option<String>,
}

#[derive(Default)]
pub struct PeersQuery;

#[Object]
impl PeersQuery {
    async fn pairing_states(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<PairingState>> {
        let value = plain_rs::services::discovery::pairing_runtime::execute(
            ctx.data::<crate::server::ServerState>()?,
            plain_rs::services::discovery::pairing_runtime::Request::States,
        )
        .await?;
        Ok(serde_json::from_value(value)?)
    }
    async fn peers(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Peer>> {
        let state = ctx.data::<crate::server::ServerState>()?;
        let rows = plain_rs::db::chat_store::peers::all(&state.db)?;
        let local = plain_rs::transport::lan::local_interfaces();
        let connections = state.peer_status.connections.snapshot_peers(&rows);
        let online = connections["online"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<std::collections::HashSet<_>>();
        Ok(rows
            .into_iter()
            .map(|row| {
                let connected = online.contains(row.id.as_str());
                Peer::from_record(row, connected, &local)
            })
            .collect())
    }

    async fn sims(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Sim>> {
        public_gate::require(
            ctx,
            ctx.data_unchecked::<Arc<Prefs>>(),
            &["READ_PHONE_NUMBERS"],
        )?;
        let facts = host_call(ctx, "systemSimFacts", json!({})).await?;
        Ok(crate::graphql::types::facts::rows(&facts, |value| Sim {
            id: crate::graphql::types::facts::id(value, "id"),
            label: crate::graphql::types::facts::text(value, "label"),
            number: crate::graphql::types::facts::text(value, "number"),
            subscription_id: crate::graphql::types::facts::integer(value, "subscriptionId") as i32,
        }))
    }
}

#[derive(Default)]
pub struct PeersMutation;

#[Object]
impl PeersMutation {
    async fn pair_device(
        &self,
        ctx: &Context<'_>,
        input: PairingDeviceInput,
    ) -> async_graphql::Result<PairingState> {
        if (input.mode == Some(PairingMode::Manage) || input.generation.is_some())
            && !crate::http::auth::public_gate::is_local(ctx)
        {
            return Err(async_graphql::Error::new("no_permission"));
        }
        let state = plain_rs::services::discovery::pairing_attempts::start(
            ctx.data::<crate::server::ServerState>()?,
            input.id.map(|id| id.to_string()),
            input.host,
            input.mode.map(Into::into),
            input.password,
            input.generation,
            ctx.data_opt::<plain_rs::host::request_scope::NativeRequestScope>()
                .cloned(),
        )
        .await?;
        Ok(serde_json::from_value(serde_json::to_value(state)?)?)
    }

    async fn cancel_pairing(
        &self,
        ctx: &Context<'_>,
        generation: String,
    ) -> async_graphql::Result<bool> {
        let state = ctx.data::<crate::server::ServerState>()?;
        if state
            .pairing_runtime
            .attempts
            .get(&generation)
            .is_some_and(|s| {
                s.mode == plain_rs::services::discovery::pairing_attempts::Mode::Manage
            })
            && !crate::http::auth::public_gate::is_local(ctx)
        {
            return Err(async_graphql::Error::new("no_permission"));
        }
        Ok(plain_rs::services::discovery::pairing_attempts::cancel(state, &generation).await?)
    }

    async fn respond_to_pairing(
        &self,
        ctx: &Context<'_>,
        input: PairingRequestInput,
        accepted: bool,
    ) -> async_graphql::Result<bool> {
        plain_rs::services::discovery::pairing_runtime::respond_device(
            ctx.data::<crate::server::ServerState>()?,
            serde_json::from_value(request(&input))?,
            accepted,
        )
        .await?;
        Ok(true)
    }

    async fn delete_peer(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
        crate::graphql::resolvers::peers::actions::remove(
            ctx.data::<crate::server::ServerState>()?,
            id.as_str(),
        )
        .await?;
        Ok(true)
    }

    async fn unpair_peer(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
        crate::graphql::resolvers::peers::actions::unpair(
            ctx.data::<crate::server::ServerState>()?,
            id.as_str(),
        )
        .await?;
        Ok(true)
    }
}

fn request(input: &PairingRequestInput) -> Value {
    json!({
        "fromId": input.from_id.as_str(),
        "fromName": input.from_name,
        "port": input.port,
        "deviceType": input.device_type.as_str(),
        "ecdhPublicKey": input.ecdh_public_key,
        "signaturePublicKey": input.signature_public_key,
        "timestamp": input.timestamp.0,
        "ips": input.ips,
        "signature": input.signature,
        "fromIp": input.from_ip,
        "awareSupported": input.aware_supported,
    })
}

async fn host_call(ctx: &Context<'_>, method: &str, params: Value) -> async_graphql::Result<Value> {
    ctx.data_unchecked::<Arc<Host>>()
        .call(method, params)
        .await
        .map_err(async_graphql::Error::new)
}

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../../tests/unit/graphql/resolvers/peers/public.rs"]
mod tests;
