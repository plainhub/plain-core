use crate::server::ServerState;
use plain_graphql::{Context, ID, Object, Result, SimpleObject};

use plain_rs::features::sessions::SessionType;
use plain_rs::infrastructure::db::Db;
use plain_rs::infrastructure::types::Instant;
use std::sync::Arc;

#[derive(SimpleObject)]
struct CreatedApiToken {
    client_id: ID,
    token: String,
}

#[derive(SimpleObject)]
struct Session {
    client_id: ID,
    client_name: String,
    r#type: SessionType,
    client_ip: String,
    os_name: String,
    os_version: String,
    app_name: String,
    app_version: String,
    last_seen_at: Option<Instant>,
    last_active: Instant,
    created_at: Instant,
    updated_at: Instant,
}

impl TryFrom<plain_rs::infrastructure::db::SessionRow> for Session {
    type Error = plain_graphql::Error;
    fn try_from(row: plain_rs::infrastructure::db::SessionRow) -> Result<Self> {
        let created_at = crate::graphql::types::parse_instant(&row.created_at)?;
        let updated_at = crate::graphql::types::parse_instant(&row.updated_at)?;
        let last_seen_at = row
            .last_seen_at
            .as_deref()
            .map(crate::graphql::types::parse_instant)
            .transpose()?;
        Ok(Self {
            client_id: row.client_id.into(),
            client_name: row.name,
            r#type: row.r#type,
            client_ip: row.client_ip,
            os_name: row.os_name,
            os_version: row.os_version,
            app_name: row.app_name,
            app_version: row.app_version,
            last_seen_at,
            last_active: last_seen_at.unwrap_or(updated_at),
            created_at,
            updated_at,
        })
    }
}

fn caller<'a>(ctx: &'a Context<'_>) -> Result<&'a str> {
    ctx.data::<String>().map(String::as_str).and_then(|id| {
        if id.is_empty() {
            Err("authenticated caller required".into())
        } else {
            Ok(id)
        }
    })
}

#[derive(Default)]
pub struct SessionsQuery;

#[Object]
impl SessionsQuery {
    fn sessions(&self, ctx: &Context<'_>) -> Result<Vec<Session>> {
        caller(ctx)?;
        let db = ctx.data::<Arc<Db>>()?;
        db.session_list()?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }
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

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/device/sessions.rs"]
mod tests;
