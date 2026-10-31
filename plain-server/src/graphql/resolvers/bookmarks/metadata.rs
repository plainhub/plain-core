use async_graphql::Object;
use plain_rs::services::bookmarks::PageMetadata;
use plain_rs::services::bookmarks::fetch_page_metadata;

#[derive(Default)]
pub struct PageMetadataQuery;

#[Object]
impl PageMetadataQuery {
    async fn page_metadata(&self, url: String) -> async_graphql::Result<PageMetadata> {
        fetch_page_metadata(&url)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))
    }
}
