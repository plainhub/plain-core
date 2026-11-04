use plain_graphql::Object;
use plain_rs::features::services::bookmarks::PageMetadata;
use plain_rs::features::services::bookmarks::fetch_page_metadata;

#[derive(Default)]
pub struct PageMetadataQuery;

#[Object]
impl PageMetadataQuery {
    async fn page_metadata(&self, url: String) -> plain_graphql::Result<PageMetadata> {
        fetch_page_metadata(&url)
            .await
            .map_err(|error| plain_graphql::Error::new(error.to_string()))
    }
}
