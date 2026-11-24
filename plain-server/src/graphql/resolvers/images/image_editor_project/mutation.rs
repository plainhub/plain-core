use super::{ImageEditorProject, ImageEditorProjectInput, to_project};
use plain_graphql::{Context, ID, Object};
use plain_rs::features::media::images::editor::Updates;
use plain_rs::infrastructure::db::{Db, image_editor_project as store};
use std::sync::Arc;

#[derive(Default)]
pub struct ImageEditorProjectMutation;

#[Object]
impl ImageEditorProjectMutation {
    fn save_image_editor_project(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: ImageEditorProjectInput,
    ) -> plain_graphql::Result<ImageEditorProject> {
        if id.as_str().trim().is_empty()
            || input.canvas_width < 0
            || input.canvas_height < 0
            || input.state_b64.len() > plain_rs::features::media::images::editor::MAX_STATE_BYTES
            || input
                .thumbnail
                .as_ref()
                .is_some_and(|s| s.len() > 1024 * 1024)
        {
            return Err(plain_graphql::Error::new("invalid image editor project"));
        }
        plain_rs::features::media::images::editor::decode(&input.state_b64)
            .map_err(plain_graphql::Error::new)?;
        let row = store::save(
            ctx.data::<Arc<Db>>()?,
            id.as_str(),
            &input.state_b64,
            input.thumbnail.as_deref(),
            input.canvas_width,
            input.canvas_height,
        )?;
        to_project(row)
    }

    fn delete_image_editor_project(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> plain_graphql::Result<bool> {
        store::delete(ctx.data::<Arc<Db>>()?, id.as_str())?;
        Ok(true)
    }

    fn broadcast_image_editor_update(
        &self,
        ctx: &Context<'_>,
        id: ID,
        update: String,
    ) -> plain_graphql::Result<bool> {
        ctx.data::<Arc<Updates>>()?
            .publish(id.as_str(), &update)
            .map_err(plain_graphql::Error::new)?;
        Ok(true)
    }
}
