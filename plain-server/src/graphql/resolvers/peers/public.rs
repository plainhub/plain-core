//! Public peer and pairing roots.

use plain_graphql::{Context, Enum, InputObject, Object, SimpleObject};

use plain_libs::serde_json::Value;
use plain_rs::features::services::device::types::DeviceType;

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

#[derive(InputObject, Clone)]
pub struct PairingDeviceInput {
    pub peer_id: plain_graphql::ID,
    pub mode: PairingMode,
    pub password: Option<String>,
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
#[derive(Default)]
pub struct PeersQuery;

#[Object]
impl PeersQuery {
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
}

fn management_login(
    ctx: &Context<'_>,
    state: &crate::server::ServerState,
) -> plain_graphql::Result<()> {
    let id = ctx
        .data_opt::<crate::graphql::executor::RequestClient>()
        .ok_or_else(|| plain_graphql::Error::new("no_permission"))?
        .client_id();
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
    ) -> plain_graphql::Result<bool> {
        let core = ctx.data::<crate::server::ServerState>()?;
        if input.mode == PairingMode::Manage && !crate::http::auth::public_gate::is_local(ctx) {
            if core
                .native_client_id()
                .is_ok_and(|local| input.peer_id.as_str() == local)
            {
                return Err(plain_graphql::Error::new("no_permission"));
            }
            management_login(ctx, core)?;
        }
        plain_rs::features::services::discovery::pairing_attempts::start(
            core,
            input.peer_id.to_string(),
            input.mode.into(),
            input.password,
            ctx.data_opt::<plain_rs::infrastructure::host::request_scope::NativeRequestScope>()
                .cloned(),
        )
        .await
        .map_err(Into::into)
    }

    async fn cancel_pairing(
        &self,
        ctx: &Context<'_>,
        peer_id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        let state = ctx.data::<crate::server::ServerState>()?;
        if state
            .pairing_runtime
            .attempts
            .mode(peer_id.as_str())
            .is_some_and(|mode| {
                mode == plain_rs::features::services::discovery::pairing_attempts::Mode::Manage
            })
            && !crate::http::auth::public_gate::is_local(ctx)
        {
            management_login(ctx, state)?;
        }
        Ok(
            plain_rs::features::services::discovery::pairing_attempts::cancel(
                state,
                peer_id.as_str(),
            )
            .await?,
        )
    }

    async fn respond_to_pairing(
        &self,
        ctx: &Context<'_>,
        peer_id: plain_graphql::ID,
        accepted: bool,
    ) -> plain_graphql::Result<bool> {
        let state = ctx.data::<crate::server::ServerState>()?;
        if state.pairing_runtime.attempts.mode(peer_id.as_str())
            == Some(plain_rs::features::services::discovery::pairing_attempts::Mode::Manage)
            && !crate::http::auth::public_gate::is_local(ctx)
        {
            management_login(ctx, state)?;
        }
        plain_rs::features::services::discovery::pairing_confirmation::respond(
            state,
            peer_id.as_str(),
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

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/peers/public.rs"]
mod tests;
