use plain_rs::features::access::AccessFeature;
use crate::graphql::types::facts::id;
use crate::graphql::types::facts::integer;
use crate::graphql::types::facts::optional_instant;
use crate::graphql::types::facts::text;
use crate::http::auth::public_gate;
use chrono::{DateTime, Utc};
use plain_graphql::{Context, Object};

use plain_rs::features::enums::DataType;
use plain_rs::infrastructure::host::bridge::Host;
use plain_rs::infrastructure::types::Instant;
use plain_rs::infrastructure::types::Long;
use serde_json::Value;
use std::sync::Arc;

pub(crate) const STORAGE: AccessFeature = AccessFeature::Files;

pub use super::types::{MediaBucket, MediaDataType};

/// The fields every library row shares. Kept as a helper rather than a
/// GraphQL interface: the contract declares `MediaItem` as an interface, but
/// nothing in it can be queried on its own — the three concrete types each
/// add fields and are what the lists return.
pub(crate) struct Row {
    pub(crate) id: plain_graphql::ID,
    pub(crate) title: String,
    pub(crate) path: String,
    pub(crate) size: Long,
    pub(crate) bucket_id: plain_graphql::ID,
    pub(crate) created_at: Instant,
    pub(crate) updated_at: Instant,
}

pub(crate) fn row(item: &Value) -> Row {
    Row {
        id: id(item, "id"),
        title: text(item, "title"),
        path: text(item, "path"),
        size: Long(integer(item, "size")),
        bucket_id: id(item, "bucketId"),
        created_at: instant(item, "createdAt"),
        updated_at: instant(item, "updatedAt"),
    }
}

#[derive(Default)]
pub struct MediaQuery;

#[Object]
impl MediaQuery {
    async fn media_buckets(
        &self,
        ctx: &Context<'_>,
        r#type: MediaDataType,
    ) -> plain_graphql::Result<Vec<MediaBucket>> {
        let feature = access_feature(r#type.data_type());
        if !public_gate::require_granted(ctx, &[feature]).await? {
            return Ok(Vec::new());
        }
        let data_type = data_type_of(r#type.as_str())?;
        let facts = catalog(ctx, data_type)
            .await?
            .buckets(data_type)
            .await
            .map_err(plain_graphql::Error::new)?;
        let items = serde_json::from_value(facts)
            .map_err(|error| plain_graphql::Error::new(error.to_string()))?;
        let buckets =
            plain_rs::features::services::files::media_buckets::aggregate(r#type.as_str(), items)
                .map_err(|error| plain_graphql::Error::new(error.to_string()))?;
        buckets
            .into_iter()
            .map(|bucket| {
                Ok(MediaBucket {
                    id: plain_graphql::ID::from(bucket.id),
                    name: bucket.name,
                    item_count: i32::try_from(bucket.item_count)
                        .map_err(|error| plain_graphql::Error::new(error.to_string()))?,
                    size: Long(bucket.size),
                    top_item_paths: bucket.top_items,
                })
            })
            .collect()
    }
}

/// The DSL's bare-token name filter, pulled out on its own: plain-app splits
/// `text` off the query and hands the rest to the platform as the
/// structured filter, so the platform never sees the text twice.
pub(crate) fn text_field(query: &str) -> String {
    plain_libs::utils::search_dsl::parse(query)
        .into_iter()
        .find(|field| field.name == "text")
        .map(|field| field.value)
        .unwrap_or_default()
}

/// One page of library rows, still in the host's raw JSON shape.
pub(crate) async fn media_rows(ctx: &Context<'_>, params: Value) -> plain_graphql::Result<Value> {
    let request: RowsRequest = serde_json::from_value(params)?;
    Ok(Value::Array(
        library_rows(
            ctx,
            request.data_type,
            request.query,
            request.sort_by,
            request.offset,
            request.limit,
        )
        .await?,
    ))
}

pub(crate) async fn media_count(
    ctx: &Context<'_>,
    data_type: &str,
    query: String,
) -> plain_graphql::Result<i32> {
    let count = catalog(ctx, data_type_of(data_type)?)
        .await?
        .count(data_type_of(data_type)?, &query)
        .await
        .map_err(plain_graphql::Error::new)?;
    i32::try_from(count).map_err(|error| plain_graphql::Error::new(error.to_string()))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RowsRequest {
    data_type: DataType,
    query: String,
    sort_by: plain_rs::features::prefs::settings::FileSortBy,
    limit: u32,
    offset: u32,
}

fn data_type_of(value: &str) -> plain_graphql::Result<DataType> {
    Ok(match value {
        "AUDIO" => DataType::Audio,
        "VIDEO" => DataType::Video,
        "IMAGE" => DataType::Image,
        "DOC" => DataType::Doc,
        other => {
            return Err(plain_graphql::Error::new(format!(
                "Unsupported media catalog type: {other}"
            )));
        }
    })
}

pub(crate) async fn catalog(
    ctx: &Context<'_>,
    data_type: DataType,
) -> plain_graphql::Result<plain_rs::features::services::files::catalog::MediaCatalog> {
    #[cfg(target_os = "ios")]
    if matches!(data_type, DataType::Image | DataType::Video) {
        let platform = ctx
            .data_unchecked::<Arc<plain_rs::features::services::files::platform::FilePlatform>>();
        platform
            .photos
            .sync(
                ctx.data_unchecked::<Arc<plain_rs::infrastructure::db::Db>>(),
                ctx.data_unchecked::<Arc<Host>>(),
            )
            .await
            .map_err(plain_graphql::Error::new)?;
    }
    #[cfg(not(target_os = "ios"))]
    let _ = data_type;
    Ok(
        plain_rs::features::services::files::catalog::MediaCatalog::new(
            ctx.data_unchecked::<Arc<plain_rs::infrastructure::db::Db>>()
                .clone(),
            ctx.data_unchecked::<Arc<Host>>().clone(),
            ctx.data_unchecked::<Arc<plain_rs::features::prefs::Prefs>>()
                .clone(),
        ),
    )
}

pub(crate) async fn library_rows(
    ctx: &Context<'_>,
    data_type: DataType,
    query: String,
    sort_by: plain_rs::features::prefs::settings::FileSortBy,
    offset: u32,
    limit: u32,
) -> plain_graphql::Result<Vec<Value>> {
    let mut rows = catalog(ctx, data_type)
        .await?
        .rows(data_type, &query, sort_by, limit, offset)
        .await
        .map_err(plain_graphql::Error::new)?;
    ctx.data::<Arc<plain_rs::features::services::files::duration_updates::Runtime>>()?
        .enrich(data_type, &mut rows)
        .await
        .map_err(plain_graphql::Error::new)?;
    Ok(rows)
}

pub(crate) async fn library_keys(
    ctx: &Context<'_>,
    data_type: DataType,
    query: String,
) -> plain_graphql::Result<Vec<String>> {
    catalog(ctx, data_type)
        .await?
        .ids(data_type, &query, false)
        .await
        .map_err(plain_graphql::Error::new)
}

pub(crate) fn instant(value: &Value, key: &str) -> Instant {
    optional_instant(value, key)
        .unwrap_or_else(|| Instant(DateTime::<Utc>::from_timestamp(0, 0).unwrap_or_default()))
}

pub(crate) fn prefs<'a>(ctx: &'a Context<'_>) -> &'a Arc<plain_rs::features::prefs::Prefs> {
    ctx.data_unchecked::<Arc<plain_rs::features::prefs::Prefs>>()
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/media/public.rs"]
mod tests;

pub(crate) fn access_feature(data_type: DataType) -> AccessFeature {
    match data_type {
        DataType::Image => AccessFeature::Images,
        DataType::Video => AccessFeature::Videos,
        DataType::Audio => AccessFeature::Audio,
        DataType::Doc => AccessFeature::Docs,
        _ => AccessFeature::Files,
    }
}
