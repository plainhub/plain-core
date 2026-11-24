use plain_graphql::Context;

use plain_graphql::Object;

use super::list_gql;
use crate::graphql::resolvers::favorites::types::FavoriteFolder;
use plain_graphql::Result;
use plain_rs::infrastructure::db::Db;
use std::sync::Arc;

#[derive(Default)]
pub struct FavoriteFolderQuery;

#[Object]
impl FavoriteFolderQuery {
    /// List all favorite folders.
    fn favorite_folders(&self, ctx: &Context<'_>) -> Result<Vec<FavoriteFolder>> {
        let db = ctx.data::<Arc<Db>>()?;
        list_gql(db)
    }
}
