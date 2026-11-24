pub mod mutation;
pub mod query;

use crate::graphql::resolvers::common::content_common;
use crate::graphql::resolvers::tags::domain_types::Tag;
use plain_graphql::{ComplexObject, Context, ID, InputObject, SimpleObject};
use plain_rs::features::enums::DataType;
use plain_rs::infrastructure::db::notes_feeds::NoteRow;
use plain_rs::infrastructure::types::Instant;
use std::sync::Arc;

type GqlResult<T> = plain_graphql::Result<T>;

#[derive(SimpleObject, Clone)]
#[graphql(complex)]
pub struct Note {
    pub id: ID,
    pub title: String,
    pub content: String,
    pub deleted_at: Option<Instant>,
    pub created_at: Instant,
    pub updated_at: Instant,
}

#[ComplexObject]
impl Note {
    async fn tags(&self, ctx: &Context<'_>) -> GqlResult<Vec<Tag>> {
        #[cfg(feature = "core")]
        if ctx
            .data_opt::<Arc<crate::graphql::resolvers::tags::loader::TagsLoader>>()
            .is_some()
        {
            return crate::graphql::resolvers::tags::loader::load(
                ctx,
                DataType::Note,
                self.id.as_str(),
            )
            .await;
        }
        content_common::tags(ctx, self.id.as_str(), DataType::Note)
    }
}

#[derive(InputObject)]
pub struct NoteInput {
    pub title: String,
    pub content: String,
}

fn to_model(row: NoteRow) -> GqlResult<Note> {
    Ok(Note {
        id: ID(row.id),
        title: row.title,
        content: row.content,
        deleted_at: row
            .deleted_at
            .as_deref()
            .map(content_common::instant)
            .transpose()?,
        created_at: content_common::instant(&row.created_at)?,
        updated_at: content_common::instant(&row.updated_at)?,
    })
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/notes/mod.rs"]
mod tests;
