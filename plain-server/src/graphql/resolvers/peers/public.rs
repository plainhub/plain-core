//! Public peer, SIM and pairing roots.

use crate::http::auth::public_gate;
use plain_graphql::{Context, Enum, InputObject, Object, SimpleObject};
use plain_rs::features::access::AccessFeature;

use plain_rs::features::prefs::Prefs;
use plain_rs::features::services::device::types::DeviceType;
use plain_rs::infrastructure::host::bridge::Host;
use serde_json::{Value, json};
use std::sync::Arc;

pub use plain_rs::features::chat::enums::PeerStatus;

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
    pub address: Option<super::records::PeerAddress>,
}

impl Peer {
    pub(crate) fn from_record(
        row: plain_rs::infrastructure::db::DPeer,
        online: bool,
        local: &[plain_rs::infrastructure::transport::lan::Interface],
    ) -> Self {
        let managed = row.is_paired() && !row.token.is_empty();
        Self {
            address: Some(
                plain_rs::features::services::peers::peer_address::address_with_interfaces(
                    &row, local,
                )
                .into(),
            ),
            key: row.key,
            public_key: row.public_key,
            token: row.token,
            online,
            managed,
            id: row.id.into(),
            name: row.name,
            ip: row.ip,
            status: match row.status {
                plain_rs::features::chat::enums::PeerStatus::Paired => PeerStatus::Paired,
                plain_rs::features::chat::enums::PeerStatus::Unpaired => PeerStatus::Unpaired,
                plain_rs::features::chat::enums::PeerStatus::Channel => PeerStatus::Channel,
            },
            port: row.port.map(i32::from),
            device_type: row.device_type.into(),
            created_at: crate::graphql::types::facts::stored_instant(&row.created_at),
            updated_at: crate::graphql::types::facts::stored_instant(&row.updated_at),
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct Sim {
    pub id: plain_graphql::ID,
    pub label: String,
    pub number: String,
    #[graphql(name = "subscriptionId")]
    pub subscription_id: i32,
}

#[derive(InputObject, Clone)]
pub struct PairingDeviceInput {
    pub id: Option<plain_graphql::ID>,
    pub host: Option<String>,
    pub mode: Option<PairingMode>,
    pub password: Option<String>,
    pub generation: Option<String>,
    pub confirm: Option<bool>,
}

#[derive(InputObject, Clone, Debug)]
pub struct PairingRequestInput {
    pub to_id: plain_graphql::ID,
    pub target_public_key: String,
    #[graphql(name = "fromId")]
    pub from_id: plain_graphql::ID,
    #[graphql(name = "fromName")]
    pub from_name: String,
    pub port: i32,
    #[graphql(name = "deviceType")]
    pub device_type: DeviceType,
    #[graphql(name = "ecdhPublicKey")]
    pub ecdh_public_key: String,
    #[graphql(name = "signaturePublicKey")]
    pub signature_public_key: String,
    pub timestamp: plain_rs::infrastructure::types::Long,
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
impl From<PairingMode> for plain_rs::features::services::discovery::pairing_attempts::Mode {
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
    device_id: Option<plain_graphql::ID>,
    generation: String,
    mode: PairingMode,
    phase: plain_rs::features::services::discovery::pairing_attempts::PairingPhase,
    error: Option<String>,
    verification_code: Option<String>,
    identity_changed: bool,
}

#[derive(Default)]
pub struct PeersQuery;

#[Object]
impl PeersQuery {
    async fn pairing_states(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<PairingState>> {
        let value = plain_rs::features::services::discovery::pairing_runtime::execute(
            ctx.data::<crate::server::ServerState>()?,
            plain_rs::features::services::discovery::pairing_runtime::Request::States,
        )
        .await?;
        Ok(serde_json::from_value(value)?)
    }
    fn peers(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<Peer>> {
        let state = ctx.data::<crate::server::ServerState>()?;
        let rows = plain_rs::infrastructure::db::chat_store::peers::all(&state.db)?;
        let local = plain_rs::infrastructure::transport::lan::local_interfaces();
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

    async fn sims(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<Sim>> {
        public_gate::require(
            ctx,
            ctx.data_unchecked::<Arc<Prefs>>(),
            &[AccessFeature::PhoneNumber],
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

fn management_login(
    ctx: &Context<'_>,
    state: &crate::server::ServerState,
) -> plain_graphql::Result<()> {
    let id = ctx.data::<crate::graphql::executor::RequestClient>()?.client_id();
    crate::http::managed_proxy::caller(state, id)
        .map(|_| ())
        .map_err(|_| plain_graphql::Error::new("no_permission"))
}

#[derive(Default)]
pub struct PeersMutation;

#[Object]
impl PeersMutation {
    async fn pair_device(
        &self,
        ctx: &Context<'_>,
        input: PairingDeviceInput,
    ) -> plain_graphql::Result<PairingState> {
        let core = ctx.data::<crate::server::ServerState>()?;
        if (input.mode == Some(PairingMode::Manage) || input.generation.is_some())
            && !crate::http::auth::public_gate::is_local(ctx)
        {
            if input.host.is_some()
                || (input.id.is_none() && input.generation.is_none())
                || input.id.as_ref().is_some_and(|id| {
                    core.native_client_id().is_ok_and(|local| id.as_str() == local)
                })
            {
                return Err(plain_graphql::Error::new("no_permission"));
            }
            management_login(ctx, core)?;
        }
        if input.confirm == Some(true) {
            let generation = input.generation.as_deref().ok_or_else(|| plain_graphql::Error::new("Missing pairing generation"))?;
            if input.id.is_some() || input.host.is_some() || input.password.is_some() || input.mode.is_some() {
                return Err(plain_graphql::Error::new("Invalid pairing confirmation"));
            }
            plain_rs::features::services::discovery::pairing_confirmation::confirm(core, generation).await?;
            let state = core.pairing_runtime.attempts.get(generation).ok_or_else(|| plain_graphql::Error::new("Pairing expired"))?;
            return Ok(serde_json::from_value(serde_json::to_value(state)?)?);
        }
        let state = plain_rs::features::services::discovery::pairing_attempts::start(
            core,
            input.id.map(|id| id.to_string()),
            input.host,
            input.mode.map(Into::into),
            input.password,
            input.generation,
            ctx.data_opt::<plain_rs::infrastructure::host::request_scope::NativeRequestScope>()
                .cloned(),
        )
        .await?;
        Ok(serde_json::from_value(serde_json::to_value(state)?)?)
    }

    async fn cancel_pairing(
        &self,
        ctx: &Context<'_>,
        generation: String,
    ) -> plain_graphql::Result<bool> {
        let state = ctx.data::<crate::server::ServerState>()?;
        if state
            .pairing_runtime
            .attempts
            .get(&generation)
            .is_some_and(|s| {
                s.mode == plain_rs::features::services::discovery::pairing_attempts::Mode::Manage
            })
            && !crate::http::auth::public_gate::is_local(ctx)
        {
            management_login(ctx, state)?;
        }
        Ok(
            plain_rs::features::services::discovery::pairing_attempts::cancel(state, &generation)
                .await?,
        )
    }

    async fn respond_to_pairing(
        &self,
        ctx: &Context<'_>,
        input: PairingRequestInput,
        accepted: bool,
    ) -> plain_graphql::Result<bool> {
        plain_rs::features::services::discovery::pairing_runtime::respond_device(
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
        id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
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
        id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
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
        "toId": input.to_id,
        "targetPublicKey": input.target_public_key,
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

async fn host_call(ctx: &Context<'_>, method: &str, params: Value) -> plain_graphql::Result<Value> {
    ctx.data_unchecked::<Arc<Host>>()
        .call(method, params)
        .await
        .map_err(plain_graphql::Error::new)
}

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/peers/public.rs"]
mod tests;
