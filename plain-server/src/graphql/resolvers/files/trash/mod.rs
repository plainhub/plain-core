pub mod mutation;
pub mod query;

use crate::http::auth::public_gate;
use plain_graphql::{Context, Result};
use plain_rs::features::access::AccessFeature;
use plain_rs::features::services::files::file_tasks::FileTasks;
use std::sync::Arc;

fn tasks<'a>(ctx: &'a Context<'_>) -> Result<&'a Arc<FileTasks>> {
    public_gate::require(
        ctx,
        ctx.data::<Arc<plain_rs::features::prefs::Prefs>>()?,
        &[AccessFeature::Files],
    )?;
    ctx.data::<Arc<FileTasks>>()
}
