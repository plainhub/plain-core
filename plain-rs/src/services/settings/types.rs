use crate::types::Instant;
#[derive(Clone, Debug)]
#[cfg_attr(feature = "graphql", derive(async_graphql::SimpleObject))]
pub struct AppUpdate {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub release_name: String,
    pub published_at: Option<Instant>,
    pub has_update: bool,
    pub url: Option<String>,
}
