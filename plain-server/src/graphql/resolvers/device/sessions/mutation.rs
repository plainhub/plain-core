use super::caller;
use crate::server::ServerState;
use plain_graphql::{Context, ID, Object, Result, SimpleObject};

#[derive(SimpleObject)]
struct CreatedApiToken {
    client_id: ID,
    token: String,
}

#[derive(Default)]
pub struct SessionsMutation;

#[Object]
impl SessionsMutation {
    fn create_api_token(&self, ctx: &Context<'_>, name: String) -> Result<CreatedApiToken> {
        caller(ctx)?;
        let session =
            plain_rs::features::services::sessions::create(ctx.data::<ServerState>()?, name)?;
        Ok(CreatedApiToken {
            client_id: session.client_id.into(),
            token: session.token,
        })
    }

    fn rename_session(&self, ctx: &Context<'_>, client_id: ID, name: String) -> Result<bool> {
        caller(ctx)?;
        Ok(plain_rs::features::services::sessions::rename(
            ctx.data::<ServerState>()?,
            client_id.as_str(),
            &name,
        )?)
    }
    fn logout(&self, ctx: &Context<'_>) -> Result<bool> {
        Ok(plain_rs::features::services::sessions::logout(
            ctx.data::<ServerState>()?,
            caller(ctx)?,
        )?)
    }

    fn revoke_session(&self, ctx: &Context<'_>, client_id: ID) -> Result<bool> {
        if client_id.trim().is_empty() {
            return Ok(false);
        }
        Ok(plain_rs::features::services::sessions::revoke(
            ctx.data::<ServerState>()?,
            caller(ctx)?,
            client_id.as_str(),
        )?)
    }
}
