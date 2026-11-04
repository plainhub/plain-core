use plain_graphql_macros::SimpleObject;

#[derive(SimpleObject, Clone, Debug)]
#[graphql(getters = false)]
pub struct FavoriteFolder {
    pub root_path: String,
    pub full_path: String,
    pub alias: Option<String>,
}
