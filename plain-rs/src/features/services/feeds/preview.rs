use crate::features::feeds::{error, require_url};
use crate::features::services::feeds::fetch_text;
use crate::features::services::feeds::parse_feed;
use crate::libs::db::Db;
use crate::libs::error::DomainResult;

pub async fn title(db: &Db, url: &str) -> DomainResult<String> {
    require_url(url)?;
    if db.feed_get_by_url(url)?.is_some() {
        return Err(error("feed_already_exists"));
    }
    Ok(parse_feed(&fetch_text(url, 4 * 1024 * 1024).await?)?.title)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/feeds/preview.rs"]
mod tests;
