use super::{GqlResult, Note, NoteInput, to_model};
use crate::graphql::types::ActionResult;
use plain_graphql::{Context, ID, Object};
use plain_rs::features::notes;
use plain_rs::infrastructure::db::Db;
use std::sync::Arc;

#[derive(Default)]
pub struct NoteMutation;

#[Object]
impl NoteMutation {
    fn create_note(&self, ctx: &Context<'_>, input: NoteInput) -> GqlResult<Note> {
        to_model(notes::create(
            ctx.data::<Arc<Db>>()?,
            &input.title,
            &input.content,
        )?)
    }

    fn update_note(&self, ctx: &Context<'_>, id: ID, input: NoteInput) -> GqlResult<Note> {
        to_model(notes::update(
            ctx.data::<Arc<Db>>()?,
            id.as_str(),
            &input.title,
            &input.content,
        )?)
    }

    fn save_feed_entries_to_notes(&self, ctx: &Context<'_>, query: String) -> GqlResult<Vec<ID>> {
        Ok(notes::save_feed_entries(ctx.data::<Arc<Db>>()?, &query)?
            .into_iter()
            .map(ID)
            .collect())
    }

    fn trash_notes(&self, ctx: &Context<'_>, query: String) -> GqlResult<ActionResult> {
        Ok(ActionResult {
            affected_count: notes::trash(ctx.data::<Arc<Db>>()?, &query)? as i32,
        })
    }

    fn restore_notes(&self, ctx: &Context<'_>, query: String) -> GqlResult<ActionResult> {
        Ok(ActionResult {
            affected_count: notes::restore(ctx.data::<Arc<Db>>()?, &query)? as i32,
        })
    }

    fn delete_notes(&self, ctx: &Context<'_>, query: String) -> GqlResult<ActionResult> {
        Ok(ActionResult {
            affected_count: notes::delete(ctx.data::<Arc<Db>>()?, &query)? as i32,
        })
    }
}
