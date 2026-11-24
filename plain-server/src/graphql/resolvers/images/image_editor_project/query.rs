use super::{ImageEditorProject, ImageEditorProjectSummary, to_project, to_summary};
use plain_graphql::{Context, ID, Object};
use plain_rs::infrastructure::db::{Db, image_editor_project as store};
use std::sync::Arc;

#[derive(Default)]
pub struct ImageEditorProjectQuery;

#[Object]
impl ImageEditorProjectQuery {
    fn image_editor_projects(
        &self,
        ctx: &Context<'_>,
    ) -> plain_graphql::Result<Vec<ImageEditorProjectSummary>> {
        store::summaries(ctx.data::<Arc<Db>>()?, 0, 50, "")?
            .into_iter()
            .map(to_summary)
            .collect()
    }

    fn image_editor_project(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> plain_graphql::Result<Option<ImageEditorProject>> {
        store::get(ctx.data::<Arc<Db>>()?, id.as_str())?
            .map(to_project)
            .transpose()
    }
}
