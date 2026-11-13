use crate::graphql::resolvers::tags::domain_types::Tag;
use plain_libs::chrono::{DateTime, Utc};
use plain_graphql::Context;
use plain_rs::infrastructure::types::Instant;
use std::sync::Arc;

use plain_rs::infrastructure::db::Db;

pub fn instant(value: &str) -> plain_graphql::Result<Instant> {
    let date = DateTime::parse_from_rfc3339(value)
        .map_err(|e| plain_graphql::Error::new(format!("invalid stored timestamp: {e}")))?;
    Ok(Instant(date.with_timezone(&Utc)))
}

pub fn tags(ctx: &Context<'_>, id: &str, kind: i32) -> plain_graphql::Result<Vec<Tag>> {
    let library = ctx.data::<Arc<Db>>()?;
    plain_rs::features::tags::tags_for_key_of_kind(library, id, kind)?
        .iter()
        .map(Tag::from_row)
        .collect()
}
