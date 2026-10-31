use crate::server::ServerState;
use async_graphql::{Context, Object, Result, SimpleObject};
use plain_rs::db::Db;
use plain_rs::sessions::SessionInfo;
use plain_rs::types::Instant;
use std::sync::Arc;

#[derive(SimpleObject)]
struct Session {
    client_id: String,
    client_name: String,
    r#type: String,
    #[graphql(guard = "crate::http::auth::public_gate::LocalOwner")]
    token: String,
    #[graphql(guard = "crate::http::auth::public_gate::LocalOwner")]
    url_token: String,
    client_ip: String,
    os_name: String,
    os_version: String,
    browser_name: String,
    browser_version: String,
    last_seen_at: Option<Instant>,
    last_active: Instant,
    created_at: Instant,
    updated_at: Instant,
}

impl TryFrom<plain_rs::db::SessionRow> for Session {
    type Error = async_graphql::Error;
    fn try_from(row: plain_rs::db::SessionRow) -> Result<Self> {
        let value = SessionInfo::try_from(row.clone())?;
        Ok(Self {
            client_id: value.client_id,
            client_name: value.client_name,
            r#type: row.r#type,
            token: value.token,
            url_token: row.url_token,
            client_ip: row.client_ip,
            os_name: value.os_name,
            os_version: value.os_version,
            browser_name: value.browser_name,
            browser_version: value.browser_version,
            last_seen_at: row
                .last_seen_at
                .map(|at| {
                    chrono::DateTime::parse_from_rfc3339(&at)
                        .map(|at| Instant(at.with_timezone(&chrono::Utc)))
                })
                .transpose()?,
            last_active: Instant(value.last_active),
            created_at: Instant(value.created_at),
            updated_at: Instant(value.updated_at),
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
    async fn sessions(&self, ctx: &Context<'_>) -> Result<Vec<Session>> {
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
    async fn rename_session(
        &self,
        ctx: &Context<'_>,
        client_id: String,
        name: String,
    ) -> Result<bool> {
        caller(ctx)?;
        Ok(plain_rs::services::sessions::rename(
            ctx.data::<ServerState>()?,
            &client_id,
            &name,
        )?)
    }
    async fn logout(&self, ctx: &Context<'_>) -> Result<bool> {
        revoke(ctx, caller(ctx)?, "logout")
    }

    async fn revoke_session(&self, ctx: &Context<'_>, client_id: String) -> Result<bool> {
        if client_id.trim().is_empty() {
            return Ok(false);
        }
        caller(ctx)?;
        revoke(ctx, &client_id, "revoke")
    }
}

fn revoke(ctx: &Context<'_>, client_id: &str, action: &str) -> Result<bool> {
    let state = ctx.data::<ServerState>()?;
    let name = state
        .db
        .session_get(client_id)?
        .map(|row| row.name)
        .unwrap_or_default();
    plain_rs::services::sessions::delete(state, client_id)?;
    state.db.audit_event_add(action, &name, Some(client_id))?;
    Ok(true)
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/device/sessions.rs"]
mod tests;
