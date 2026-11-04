use crate::graphql::resolvers::tags::domain_types::Tag;
use crate::graphql::types::facts::rows;
use crate::http::auth::public_gate;
use async_graphql::Context;
use plain_graphql_macros::{SimpleObject, object as Object};
use plain_rs::infrastructure::types::Instant;
use plain_rs::infrastructure::types::Long;
use serde_json::json;
use std::path::Path;

use crate::graphql::resolvers::media::public::STORAGE;
use crate::graphql::resolvers::media::public::media_count;
use crate::graphql::resolvers::media::public::media_rows;
use crate::graphql::resolvers::media::public::prefs;
use crate::graphql::resolvers::media::public::row;

#[derive(SimpleObject, Clone, Debug)]
#[graphql(getters = false)]
#[graphql(complex)]
pub struct Doc {
    pub duration_ms: Long,
    #[graphql(getter = true)]
    pub id: async_graphql::ID,
    #[graphql(getter = true)]
    pub title: String,
    #[graphql(getter = true)]
    pub path: String,
    pub extension: String,
    #[graphql(getter = true)]
    pub size: Long,
    #[graphql(name = "bucketId")]
    #[graphql(getter = true)]
    pub bucket_id: async_graphql::ID,
    #[graphql(name = "createdAt")]
    #[graphql(getter = true)]
    pub created_at: Instant,
    #[graphql(name = "updatedAt")]
    #[graphql(getter = true)]
    pub updated_at: Instant,
}

#[async_graphql::ComplexObject]
impl Doc {
    async fn tags(&self, ctx: &async_graphql::Context<'_>) -> async_graphql::Result<Vec<Tag>> {
        crate::graphql::resolvers::tags::loader::load(
            ctx,
            plain_rs::features::enums::DataType::Doc,
            self.id.as_str(),
        )
        .await
    }
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(getters = false)]
pub struct DocExtGroup {
    pub ext: String,
    pub count: i32,
}

#[derive(Default)]
pub struct DocsQuery;

#[Object]
impl DocsQuery {
    async fn doc_count(&self, ctx: &Context<'_>, query: String) -> async_graphql::Result<i32> {
        if !public_gate::require_granted(ctx, &[STORAGE]).await? {
            return Ok(0);
        }
        media_count(ctx, "DOC", query).await
    }

    async fn docs(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
        sort_by: plain_rs::features::prefs::settings::FileSortBy,
    ) -> async_graphql::Result<Vec<Doc>> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let items = media_rows(
            ctx,
            json!({
                "dataType": "DOC", "query": query,
                "offset": offset, "limit": limit, "sortBy": sort_by.as_str(),
            }),
        )
        .await?;
        Ok(rows(&items, |item| {
            let row = row(item);
            Doc {
                duration_ms: Long(crate::graphql::types::facts::integer(item, "durationMs")),
                id: row.id.into(),
                title: row.title,
                // Derived from the path, exactly as plain-app's lazy
                // `DDoc.extension` does — the store does not carry it.
                extension: Path::new(&row.path)
                    .extension()
                    .map(|value| value.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path: row.path,
                size: row.size,
                bucket_id: row.bucket_id,
                created_at: row.created_at,
                updated_at: row.updated_at,
            }
        }))
    }

    async fn doc_ext_groups(
        &self,
        ctx: &Context<'_>,
        #[graphql(default)] query: String,
    ) -> async_graphql::Result<Vec<DocExtGroup>> {
        if !public_gate::require_granted(ctx, &[STORAGE]).await? {
            return Ok(Vec::new());
        }
        Ok(crate::graphql::resolvers::media::public::catalog(ctx)
            .document_extensions(&query)
            .await
            .map_err(async_graphql::Error::new)?
            .into_iter()
            .map(|(ext, count)| DocExtGroup { ext, count })
            .collect())
    }
}
