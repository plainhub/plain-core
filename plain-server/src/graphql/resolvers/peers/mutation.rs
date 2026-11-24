//! Public peer and pairing roots.
use super::{PairPeerInput, PairingMode};
use plain_graphql::{Context, Object};
use plain_rs::features::access_error::AccessErrorCode;

fn management_login(
    ctx: &Context<'_>,
    state: &crate::server::ServerState,
) -> plain_graphql::Result<()> {
    let id = ctx
        .data_opt::<crate::graphql::executor::RequestClient>()
        .ok_or_else(|| plain_graphql::Error::from(AccessErrorCode::NoPermission))?
        .client_id();
    crate::http::managed_proxy::caller(state, id)
        .map(|_| ())
        .map_err(|_| plain_graphql::Error::from(AccessErrorCode::NoPermission))
}

#[derive(Default)]
pub struct PeersMutation;

#[Object]
impl PeersMutation {
    async fn pair_peer(
        &self,
        ctx: &Context<'_>,
        peer_id: plain_graphql::ID,
        input: PairPeerInput,
    ) -> plain_graphql::Result<bool> {
        let core = ctx.data::<crate::server::ServerState>()?;
        if input.mode == PairingMode::Manage && !crate::http::auth::public_gate::is_local(ctx) {
            if core
                .native_client_id()
                .is_ok_and(|local| peer_id.as_str() == local)
            {
                return Err(plain_graphql::Error::from(AccessErrorCode::NoPermission));
            }
            management_login(ctx, core)?;
        }
        plain_rs::features::services::discovery::pairing_attempts::start(
            core,
            peer_id.to_string(),
            input.mode,
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
        if state.pairing_runtime.attempts.mode(peer_id.as_str()) == Some(PairingMode::Manage)
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
        if state.pairing_runtime.attempts.mode(peer_id.as_str()) == Some(PairingMode::Manage)
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
        peer_id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        crate::graphql::resolvers::peers::actions::remove(
            ctx.data::<crate::server::ServerState>()?,
            peer_id.as_str(),
        )
        .await?;
        Ok(true)
    }

    async fn unpair_peer(
        &self,
        ctx: &Context<'_>,
        peer_id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        crate::graphql::resolvers::peers::actions::unpair(
            ctx.data::<crate::server::ServerState>()?,
            peer_id.as_str(),
        )
        .await?;
        Ok(true)
    }
}
