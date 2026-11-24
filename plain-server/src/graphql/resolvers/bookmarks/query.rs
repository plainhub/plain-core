use crate::graphql::resolvers::bookmarks::bookmark_types::{Bookmark, BookmarkGroup};
use plain_graphql::{Context, ID, Object};
use plain_rs::infrastructure::db::{Db, bookmark as bookmark_db};
use std::sync::Arc;

#[derive(Default)]
pub struct BookmarkQuery;

#[Object]
impl BookmarkQuery {
    fn bookmark(&self, ctx: &Context<'_>, id: ID) -> plain_graphql::Result<Option<Bookmark>> {
        let db = ctx.data_unchecked::<Arc<Db>>();
        Ok(bookmark_db::get_bookmark_by_id(db, id.as_str())?.map(Bookmark::from))
    }

    fn bookmarks(&self, ctx: &Context<'_>) -> Vec<Bookmark> {
        let db = ctx.data_unchecked::<Arc<Db>>();
        bookmark_db::get_bookmarks(db)
            .into_iter()
            .map(Bookmark::from)
            .collect()
    }

    fn bookmark_groups(&self, ctx: &Context<'_>) -> Vec<BookmarkGroup> {
        let db = ctx.data_unchecked::<Arc<Db>>();
        let counts = bookmark_db::get_bookmarks(db).into_iter().fold(
            std::collections::HashMap::new(),
            |mut acc, b| {
                if let Some(group_id) = b.group_id {
                    *acc.entry(group_id).or_insert(0i32) += 1;
                }
                acc
            },
        );
        bookmark_db::get_bookmark_groups(db)
            .into_iter()
            .map(|g| {
                let count = counts.get(&g.id).copied().unwrap_or(0);
                BookmarkGroup::from_group(g, count)
            })
            .collect()
    }
}
