pub mod mutation;
pub mod query;

use plain_graphql::{ID, InputObject, SimpleObject};
use plain_rs::infrastructure::db::image_editor_project as store;
use plain_rs::infrastructure::types::Instant;

#[derive(SimpleObject)]
pub struct ImageEditorProject {
    pub id: ID,
    pub state_b64: String,
    pub thumbnail: Option<String>,
    pub canvas_width: i32,
    pub canvas_height: i32,
    pub created_at: Instant,
    pub updated_at: Instant,
}

#[derive(SimpleObject)]
pub struct ImageEditorProjectSummary {
    pub id: ID,
    pub thumbnail: Option<String>,
    pub canvas_width: i32,
    pub canvas_height: i32,
    pub updated_at: Instant,
}

#[derive(InputObject)]
pub struct ImageEditorProjectInput {
    pub state_b64: String,
    pub thumbnail: Option<String>,
    pub canvas_width: i32,
    pub canvas_height: i32,
}

fn instant(value: &str) -> plain_graphql::Result<Instant> {
    plain_libs::chrono::DateTime::parse_from_rfc3339(value)
        .map(|value| Instant(value.with_timezone(&plain_libs::chrono::Utc)))
        .map_err(|error| plain_graphql::Error::new(format!("invalid stored timestamp: {error}")))
}

fn to_project(row: store::ImageEditorProjectRow) -> plain_graphql::Result<ImageEditorProject> {
    Ok(ImageEditorProject {
        id: ID(row.id),
        state_b64: row.state_b64,
        thumbnail: row.thumbnail,
        canvas_width: row.canvas_width,
        canvas_height: row.canvas_height,
        created_at: instant(&row.created_at)?,
        updated_at: instant(&row.updated_at)?,
    })
}

pub(crate) fn to_summary(
    row: store::ImageEditorProjectRow,
) -> plain_graphql::Result<ImageEditorProjectSummary> {
    Ok(ImageEditorProjectSummary {
        id: ID(row.id),
        thumbnail: row.thumbnail,
        canvas_width: row.canvas_width,
        canvas_height: row.canvas_height,
        updated_at: instant(&row.updated_at)?,
    })
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-server/unit/graphql/resolvers/images/image_editor_project/mod.rs"]
mod tests;
