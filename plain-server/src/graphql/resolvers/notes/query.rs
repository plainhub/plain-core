use super::{GqlResult, Note, to_model};
use plain_graphql::{Context, ID, Object};
use plain_rs::features::notes;
use plain_rs::infrastructure::db::Db;
use std::sync::Arc;

#[derive(Default)]
pub struct NoteQuery;

#[Object]
impl NoteQuery {
    fn export_notes(&self, ctx: &Context<'_>, query: String) -> GqlResult<String> {
        Ok(notes::export(ctx.data::<Arc<Db>>()?, &query)?)
    }

    fn note_count(&self, ctx: &Context<'_>, query: String) -> GqlResult<i32> {
        Ok(notes::count(ctx.data::<Arc<Db>>()?, &query)?)
    }

    fn note(&self, ctx: &Context<'_>, id: ID) -> GqlResult<Option<Note>> {
        notes::get(ctx.data::<Arc<Db>>()?, id.as_str())?
            .map(to_model)
            .transpose()
    }

    fn notes(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> GqlResult<Vec<Note>> {
        notes::search(ctx.data::<Arc<Db>>()?, &query, limit, offset)?
            .into_iter()
            .map(to_model)
            .collect()
    }
}
