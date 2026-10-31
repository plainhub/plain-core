use crate::server::ServerState;
use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema, SimpleObject};
use plain_rs::files::shares::Service;
use plain_rs::types::Long;

#[derive(SimpleObject)]
struct SharedFile {
    name: String,
    relative_path: String,
    is_dir: bool,
    size: Long,
    mime_type: String,
    has_thumb: bool,
}
#[derive(SimpleObject)]
struct SharedInfo {
    name: String,
    read_only: bool,
    requires_password: bool,
    expires_at: Option<Long>,
    url_token: String,
    entries: Vec<SharedFile>,
}
struct Guest {
    state: ServerState,
    id: String,
}
pub(crate) struct Query;
#[Object]
impl Query {
    async fn shared_info(
        &self,
        ctx: &Context<'_>,
        relative_path: Option<String>,
    ) -> async_graphql::Result<SharedInfo> {
        let guest = ctx.data::<Guest>()?;
        let service = Service::new(guest.state.db.clone(), guest.state.prefs.clone());
        let id = guest.id.clone();
        let path = relative_path.unwrap_or_default();
        let (share, roots) = tokio::task::spawn_blocking(move || service.browse(&id, &path))
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        let mut entries = Vec::new();
        for root in roots {
            let name = root
                .relative_path
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_owned();
            let ext = name
                .rsplit('.')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let has_thumb = !root.is_dir
                && [
                    "jpg", "png", "jpeg", "bmp", "webp", "heic", "heif", "apng", "avif", "gif",
                    "tiff", "tif", "svg",
                ]
                .contains(&ext.as_str());
            let (size, mime_type) = if root.is_dir {
                (0, String::new())
            } else {
                let metadata = tokio::fs::metadata(&root.real_path)
                    .await
                    .map_err(|error| async_graphql::Error::new(error.to_string()))?;
                let size = i64::try_from(metadata.len())
                    .map_err(|error| async_graphql::Error::new(error.to_string()))?;
                (
                    size,
                    plain_rs::utils::mime::mime_from_ext(&root.real_path).to_owned(),
                )
            };
            entries.push(SharedFile {
                name,
                relative_path: root.relative_path,
                is_dir: root.is_dir,
                size: Long(size),
                mime_type,
                has_thumb,
            });
        }
        Ok(SharedInfo {
            name: share.name,
            read_only: share.read_only,
            requires_password: !share.password.is_empty(),
            expires_at: share
                .expires_at
                .as_deref()
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .map(|t| Long(t.timestamp_millis())),
            url_token: share.url_token,
            entries,
        })
    }
}
fn schema_builder() -> async_graphql::SchemaBuilder<Query, EmptyMutation, EmptySubscription> {
    Schema::build(Query, EmptyMutation, EmptySubscription)
        .extension(crate::graphql::enum_literals::EnumLiteralValidation)
}

pub(crate) fn schema(
    state: ServerState,
    id: String,
) -> Schema<Query, EmptyMutation, EmptySubscription> {
    schema_builder().data(Guest { state, id }).finish()
}

#[cfg(test)]
#[path = "../../../tests/unit/graphql/schemas/guest.rs"]
mod tests;
