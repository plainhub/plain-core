use plain_graphql::{ID, InputObject};
use plain_rs::features::downloads::Source;

#[derive(InputObject)]
pub(super) struct ResourceDownloadInput {
    urls: Vec<String>,
    target_dir: String,
    zip_name: Option<String>,
}
#[derive(InputObject)]
pub(super) struct ChatDownloadInput {
    message_id: ID,
    peer_id: ID,
    file_ids: Vec<String>,
}
#[derive(InputObject)]
#[graphql(oneof)]
pub(super) struct DownloadInput {
    resources: Option<ResourceDownloadInput>,
    chat: Option<ChatDownloadInput>,
}
impl DownloadInput {
    pub(super) fn source(self) -> plain_graphql::Result<(Source, Option<String>, Option<String>)> {
        match (self.resources, self.chat) {
            (Some(input), None) => Ok((
                Source::Resources { urls: input.urls },
                Some(input.target_dir),
                input.zip_name,
            )),
            (None, Some(input)) => Ok((
                Source::Chat {
                    message_id: input.message_id.0,
                    peer_id: input.peer_id.0,
                    file_ids: input.file_ids,
                },
                None,
                None,
            )),
            _ => Err(plain_graphql::Error::new(
                "Exactly one download source required",
            )),
        }
    }
}
