use serde::Serialize;
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default, Serialize)]
#[cfg_attr(feature = "graphql", derive(async_graphql::Enum))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ImageSearchStatusType {
    #[default]
    Unavailable,
    Downloading,
    Loading,
    Ready,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "graphql", derive(async_graphql::SimpleObject))]
#[serde(rename_all = "camelCase")]
pub struct ImageSearchStatus {
    pub status: ImageSearchStatusType,
    #[cfg_attr(feature = "graphql", graphql(name = "downloadProgress"))]
    pub download_progress: i32,
    #[cfg_attr(feature = "graphql", graphql(name = "errorMessage"))]
    pub error_message: String,
    #[cfg_attr(feature = "graphql", graphql(name = "modelSize"))]
    pub model_size: plain_rs::types::Long,
    #[cfg_attr(feature = "graphql", graphql(name = "modelDir"))]
    pub model_dir: String,
    #[cfg_attr(feature = "graphql", graphql(name = "isIndexing"))]
    pub is_indexing: bool,
    #[cfg_attr(feature = "graphql", graphql(name = "totalImages"))]
    pub total_images: i32,
    #[cfg_attr(feature = "graphql", graphql(name = "indexedImages"))]
    pub indexed_images: i32,
}
