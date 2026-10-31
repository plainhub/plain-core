use crate::graphql::resolvers::tags::domain_types::Tag;
use crate::graphql::resolvers::tags::domain_types::TagRelation;
use async_graphql::{Context, Object};
use plain_rs::db::Db;
use plain_rs::db::TagRow;
pub use plain_rs::enums::DataType;
use plain_rs::host::bridge::Host;
use plain_rs::services::capabilities;
use plain_rs::tags;
use serde_json::json;
use std::sync::Arc;

#[derive(Default)]
pub struct TagsQuery;

#[Object]
impl TagsQuery {
    async fn tags(&self, ctx: &Context<'_>, r#type: DataType) -> async_graphql::Result<Vec<Tag>> {
        let db = db(ctx);
        Ok(tags::tags_by_type(db, r#type.kind())
            .map_err(failed)?
            .iter()
            .map(tag)
            .collect())
    }

    async fn tag_relations(
        &self,
        ctx: &Context<'_>,
        r#type: DataType,
        keys: Vec<String>,
    ) -> async_graphql::Result<Vec<TagRelation>> {
        let db = db(ctx);
        Ok(tags::relations_for_keys_of_kind(db, &keys, r#type.kind())
            .map_err(failed)?
            .iter()
            .map(|row| TagRelation {
                tag_id: async_graphql::ID::from(row.tag_id.clone()),
                key: row.target_key.clone(),
            })
            .collect())
    }

    async fn tag_keys(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<Vec<String>> {
        let db = db(ctx);
        tags::keys_for_tag(db, id.as_str()).map_err(failed)
    }
}

#[derive(Default)]
pub struct TagsMutation;

#[Object]
impl TagsMutation {
    async fn create_tag(
        &self,
        ctx: &Context<'_>,
        r#type: DataType,
        name: String,
    ) -> async_graphql::Result<Tag> {
        let row = tags::create_tag(db(ctx), r#type.kind(), &name).map_err(failed)?;
        Ok(tag(&row))
    }

    async fn update_tag(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
        name: String,
    ) -> async_graphql::Result<Tag> {
        let row = tags::update_tag(db(ctx), id.as_str(), &name).map_err(failed)?;
        row.as_ref()
            .map(tag)
            .ok_or_else(|| async_graphql::Error::new(format!("Tag {} not found", id.as_str())))
    }

    async fn delete_tag(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
        tags::delete_tag(db(ctx), id.as_str()).map_err(failed)?;
        Ok(true)
    }

    async fn add_to_tags(
        &self,
        ctx: &Context<'_>,
        r#type: DataType,
        tag_ids: Vec<async_graphql::ID>,
        query: String,
    ) -> async_graphql::Result<bool> {
        let keys = tag_query_keys(ctx, r#type, &query).await?;
        let ids = tag_ids
            .into_iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>();
        tags::update_relations(db(ctx), r#type.kind(), &keys, &ids, &[]).map_err(failed)?;
        Ok(true)
    }

    async fn remove_from_tags(
        &self,
        ctx: &Context<'_>,
        r#type: DataType,
        tag_ids: Vec<async_graphql::ID>,
        query: String,
    ) -> async_graphql::Result<bool> {
        let keys = tag_query_keys(ctx, r#type, &query).await?;
        let ids = tag_ids
            .into_iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>();
        tags::update_relations(db(ctx), r#type.kind(), &keys, &[], &ids).map_err(failed)?;
        Ok(true)
    }

    async fn update_tag_relations(
        &self,
        ctx: &Context<'_>,
        r#type: DataType,
        keys: Vec<String>,
        add_tag_ids: Vec<async_graphql::ID>,
        remove_tag_ids: Vec<async_graphql::ID>,
    ) -> async_graphql::Result<bool> {
        let add = add_tag_ids
            .into_iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>();
        let remove = remove_tag_ids
            .into_iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>();
        tags::update_relations(db(ctx), r#type.kind(), &keys, &add, &remove).map_err(failed)?;
        Ok(true)
    }
}

async fn tag_query_keys(
    ctx: &Context<'_>,
    r#type: DataType,
    query: &str,
) -> async_graphql::Result<Vec<String>> {
    require_queryable(r#type, query)?;
    if r#type == DataType::Note {
        return Ok(db(ctx).note_ids(query, None)?);
    }
    if r#type == DataType::FeedEntry {
        return Ok(db(ctx).feed_entry_ids(query)?);
    }
    if matches!(r#type, DataType::Call | DataType::Contact | DataType::Sms) {
        return provider_query_keys(ctx, r#type, query).await;
    }
    crate::graphql::resolvers::media::public::library_keys(ctx, r#type, query.to_owned()).await
}

async fn provider_query_keys(
    ctx: &Context<'_>,
    kind: DataType,
    query: &str,
) -> async_graphql::Result<Vec<String>> {
    if kind == DataType::Contact {
        capabilities::require(ctx.data_unchecked::<Arc<Host>>(), "CONTACTS")
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    }
    if kind == DataType::Sms {
        let facts = plain_rs::services::sms::query::execute(
            db(ctx),
            ctx.data_unchecked::<Arc<Host>>(),
            plain_rs::services::sms::query::Request::Ids {
                query: query.to_owned(),
                include_trashed: false,
            },
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        return serde_json::from_value(facts["ids"].clone())
            .map_err(|error| async_graphql::Error::new(error.to_string()));
    }
    let method = if kind == DataType::Call {
        capabilities::require(ctx.data_unchecked::<Arc<Host>>(), "CALLS")
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        "systemCallIds"
    } else {
        "systemContactIds"
    };
    let provider = plain_rs::services::files::provider_query::media_provider(kind)
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    let params =
        crate::graphql::types::facts::provider_params(ctx, provider, query, json!({})).await?;
    let facts = crate::graphql::types::facts::host_json(ctx, method, params).await?;
    Ok(facts
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_owned)
        .collect())
}

/// A blank query would match every row of the kind. That is never what a
/// client meant, and the desktop UI has no way to express "tag everything",
/// so it is refused rather than silently applied.
fn require_queryable(r#type: DataType, query: &str) -> async_graphql::Result<()> {
    if !tags_queryable(r#type) {
        return Err(async_graphql::Error::new(format!(
            "Unsupported tag query type: {}",
            data_type_name(r#type)
        )));
    }
    if query.trim().is_empty() {
        return Err(async_graphql::Error::new("explicit query required"));
    }
    Ok(())
}

fn tags_queryable(kind: DataType) -> bool {
    matches!(
        kind,
        DataType::Audio
            | DataType::Video
            | DataType::Image
            | DataType::Doc
            | DataType::Call
            | DataType::Contact
            | DataType::Sms
            | DataType::Note
            | DataType::FeedEntry
    )
}

fn data_type_name(kind: DataType) -> &'static str {
    match kind {
        DataType::Default => "DEFAULT",
        DataType::Audio => "AUDIO",
        DataType::Video => "VIDEO",
        DataType::Image => "IMAGE",
        DataType::Sms => "SMS",
        DataType::Contact => "CONTACT",
        DataType::Note => "NOTE",
        DataType::FeedEntry => "FEED_ENTRY",
        DataType::Call => "CALL",
        DataType::Package => "PACKAGE",
        DataType::File => "FILE",
        DataType::AppFile => "APP_FILE",
        DataType::Doc => "DOC",
    }
}

fn tag(row: &TagRow) -> Tag {
    Tag {
        id: async_graphql::ID::from(row.id.clone()),
        name: row.name.clone(),
        r#type: row.kind,
        count: row.count,
    }
}

fn failed(error: plain_rs::error::DomainError) -> async_graphql::Error {
    async_graphql::Error::new(error.to_string())
}

fn db<'a>(ctx: &'a Context<'_>) -> &'a Arc<Db> {
    ctx.data_unchecked::<Arc<Db>>()
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/tags/public.rs"]
mod tests;
