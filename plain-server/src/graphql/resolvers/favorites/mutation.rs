use plain_graphql::Context;

use plain_graphql::Object;

use super::list_gql;
use crate::graphql::resolvers::favorites::types::FavoriteFolder;
use plain_graphql::Result;
use plain_rs::features::favorites as favorite_folders;
use plain_rs::infrastructure::db::Db;
use std::sync::Arc;

#[derive(Default)]
pub struct FavoriteFolderMutation;

#[Object]
impl FavoriteFolderMutation {
    /// Register a favorite folder and return the whole list.
    fn add_favorite_folder(
        &self,
        ctx: &Context<'_>,
        root_path: String,
        full_path: String,
    ) -> Result<Vec<FavoriteFolder>> {
        let db = ctx.data::<Arc<Db>>()?;
        favorite_folders::add_full_path(db, &root_path, &full_path)?;
        list_gql(db)
    }

    /// Remove the favorite folder identified by `fullPath` and return the
    /// whole list.
    fn remove_favorite_folder(
        &self,
        ctx: &Context<'_>,
        full_path: String,
    ) -> Result<Vec<FavoriteFolder>> {
        let db = ctx.data::<Arc<Db>>()?;
        if let Some(f) = favorite_folders::find_by_full_path(db, &full_path)? {
            favorite_folders::remove(db, &f.root_path, &f.relative_path)?;
        }
        list_gql(db)
    }

    /// Set the favorite folder's display alias and return the whole list; error if the favorite does not exist.
    fn set_favorite_folder_alias(
        &self,
        ctx: &Context<'_>,
        full_path: String,
        alias: String,
    ) -> Result<Vec<FavoriteFolder>> {
        let db = ctx.data::<Arc<Db>>()?;
        let favorite = favorite_folders::find_by_full_path(db, &full_path)?
            .ok_or_else(|| plain_graphql::Error::new("Favorite folder not found"))?;
        if !favorite_folders::set_alias(db, &favorite.root_path, &favorite.relative_path, &alias)? {
            return Err(plain_graphql::Error::new("Favorite folder not found"));
        }
        list_gql(db)
    }
}
