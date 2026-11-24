use super::caller;
use plain_graphql::{Context, ID, Object, Result, SimpleObject};
use plain_rs::features::sessions::SessionType;
use plain_rs::infrastructure::db::Db;
use plain_rs::infrastructure::types::Instant;
use std::sync::Arc;

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
