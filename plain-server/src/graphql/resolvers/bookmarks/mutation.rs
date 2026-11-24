use super::emit_bookmark_updated;
use crate::graphql::resolvers::bookmarks::bookmark_types::{
    Bookmark, BookmarkGroup, BookmarkInput,
};
use crate::graphql::types::ActionResult;
use plain_graphql::{Context, ID, Object};
use plain_rs::infrastructure::db::bookmark::DBookmarkGroup;
use plain_rs::infrastructure::db::{Db, bookmark as bookmark_db, now_iso};
use std::sync::Arc;

#[derive(Default)]
pub struct BookmarkMutation;

#[Object]
impl BookmarkMutation {
    fn import_bookmarks(
        &self,
        ctx: &Context<'_>,
        content: String,
    ) -> plain_graphql::Result<ActionResult> {
        let db = ctx.data_unchecked::<Arc<Db>>();
        Ok(ActionResult {
            affected_count: plain_rs::features::services::bookmarks::import_bookmarks(
                db, &content,
            )?,
        })
    }

    fn add_bookmarks(
        &self,
        ctx: &Context<'_>,
        urls: Vec<String>,
        group_id: ID,
    ) -> plain_graphql::Result<Vec<Bookmark>> {
        let db = ctx.data_unchecked::<Arc<Db>>();
        let created =
            plain_rs::features::services::bookmarks::add_bookmarks(db, urls, group_id.as_str())?;
        plain_rs::features::services::bookmarks::start_metadata_update(
            ctx.data_unchecked::<crate::server::ServerState>()
                .core
                .clone(),
            created.clone(),
        );
        Ok(created.into_iter().map(Bookmark::from).collect())
    }

    async fn update_bookmark(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: BookmarkInput,
    ) -> Result<Bookmark, plain_graphql::Error> {
        let db = ctx.data_unchecked::<Arc<Db>>();
        plain_rs::features::services::bookmarks::parse_bookmark_url(&input.url)?;
        let mut bookmark = bookmark_db::get_bookmark_by_id(db, id.as_str())?
            .ok_or_else(|| plain_graphql::Error::new("bookmark not found"))?;
        bookmark.url = input.url;
        bookmark.title = input.title;
        bookmark.group_id = (!input.group_id.is_empty()).then(|| input.group_id.to_string());
        bookmark.pinned = input.pinned;
        bookmark.sort_order = input.sort_order;
        let assets = ctx.data_unchecked::<Arc<plain_rs::features::services::feeds::FeedAssets>>();
        let favicon_path = match input.favicon_url.as_deref() {
            None => None,
            Some("") => Some(String::new()),
            Some(url) => Some(assets.cache_bookmark_icon(url).await?),
        };
        let saved = bookmark_db::update_fields(db, &bookmark, favicon_path.as_deref())
            .map_err(plain_graphql::Error::from)
            .and_then(|saved| saved.ok_or_else(|| plain_graphql::Error::new("bookmark not found")));
        let (bookmark, previous_favicon) = match saved {
            Ok(saved) => saved,
            Err(error) => {
                if let Some(path) = &favicon_path {
                    assets.release(path);
                }
                return Err(error);
            }
        };
        if favicon_path.is_some() {
            assets.release(&previous_favicon);
        }
        emit_bookmark_updated(ctx, std::slice::from_ref(&bookmark));
        Ok(Bookmark::from(bookmark))
    }

    fn delete_bookmarks(
        &self,
        ctx: &Context<'_>,
        ids: Vec<ID>,
    ) -> plain_graphql::Result<ActionResult> {
        let db = ctx.data_unchecked::<Arc<Db>>();
        let ids = ids.into_iter().map(|id| id.to_string()).collect::<Vec<_>>();
        let removed = bookmark_db::delete_bookmarks_with_rows(db, &ids)?;
        let count = removed.len() as i32;
        if let Ok(assets) = ctx.data::<Arc<plain_rs::features::services::feeds::FeedAssets>>() {
            for item in removed {
                assets.release(&item.favicon_path);
            }
        }
        Ok(ActionResult {
            affected_count: count,
        })
    }

    fn record_bookmark_click(&self, ctx: &Context<'_>, id: ID) -> plain_graphql::Result<bool> {
        bookmark_db::record_click(ctx.data_unchecked::<Arc<Db>>(), id.as_str())?;
        Ok(true)
    }

    fn create_bookmark_group(
        &self,
        ctx: &Context<'_>,
        name: String,
    ) -> plain_graphql::Result<BookmarkGroup> {
        let db = ctx.data_unchecked::<Arc<Db>>();
        let group = DBookmarkGroup::new(name.trim());
        bookmark_db::insert_bookmark_group(db, &group)?;
        Ok(BookmarkGroup::from_group(group, 0))
    }

    fn update_bookmark_group(
        &self,
        ctx: &Context<'_>,
        id: ID,
        name: String,
        collapsed: bool,
        sort_order: i32,
    ) -> Result<BookmarkGroup, plain_graphql::Error> {
        let db = ctx.data_unchecked::<Arc<Db>>();
        let mut group = bookmark_db::get_bookmark_group_by_id(db, id.as_str())
            .ok_or_else(|| plain_graphql::Error::new("bookmark group not found"))?;
        group.name = name;
        group.collapsed = collapsed;
        group.sort_order = sort_order;
        group.updated_at = now_iso();
        bookmark_db::update_bookmark_group(db, &group)?;
        let item_count = bookmark_db::get_bookmarks_by_group_id(db, &group.id).len() as i32;
        Ok(BookmarkGroup::from_group(group, item_count))
    }

    fn delete_bookmark_group(&self, ctx: &Context<'_>, id: ID) -> plain_graphql::Result<bool> {
        let db = ctx.data_unchecked::<Arc<Db>>();
        let updated = bookmark_db::delete_bookmark_group(db, id.as_str())?;
        emit_bookmark_updated(ctx, &updated);
        Ok(true)
    }
}
