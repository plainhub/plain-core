#[cfg(feature = "core")]
pub mod content_common;

use plain_graphql::{Context, Error, Result};
use plain_rs::features::prefs::{Prefs, pref_keys::DEVELOPER_MODE};
use std::sync::Arc;

pub(crate) fn require_developer_mode(ctx: &Context<'_>) -> Result<()> {
    if !ctx
        .data::<Arc<Prefs>>()?
        .get::<bool>(DEVELOPER_MODE)?
        .unwrap_or(false)
    {
        return Err(Error::new("No permission: developer mode is disabled"));
    }
    Ok(())
}

pub(crate) fn require_remote_developer_mode(ctx: &Context<'_>) -> Result<()> {
    if ctx.data_opt::<crate::graphql::executor::RequestOrigin>()
        != Some(&crate::graphql::executor::RequestOrigin::Local)
    {
        require_developer_mode(ctx)?;
    }
    Ok(())
}

pub(crate) fn validate_pagination(offset: i32, limit: i32) -> Result<()> {
    if offset < 0 || limit < 0 {
        return Err(Error::new("invalid pagination"));
    }
    Ok(())
}
