//! Favorite folders (file-browser pins) — backed by the shared plain-rs
//! library core (`the shared SQLite database`), same behavior as the server.
//! The mutations return the whole updated list (phone contract).

pub mod mutation;
pub mod types;

pub mod query;

use crate::graphql::resolvers::favorites::types::FavoriteFolder;
use plain_graphql::Result;
use plain_rs::features::favorites as favorite_folders;
use plain_rs::infrastructure::db::Db;
use std::sync::Arc;

fn to_gql(f: favorite_folders::FavoriteFolder) -> FavoriteFolder {
    FavoriteFolder {
        full_path: favorite_folders::full_path_of(&f),
        root_path: f.root_path,
        alias: f.alias,
    }
}

fn list_gql(db: &Arc<Db>) -> Result<Vec<FavoriteFolder>> {
    Ok(favorite_folders::list(db)?
        .into_iter()
        .map(to_gql)
        .collect())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/favorites/mod.rs"]
mod tests;
