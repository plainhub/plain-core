use plain_graphql::{ID, SimpleObject, Union};
use plain_rs::features::downloads::{self, Source, Status};
use plain_rs::infrastructure::types::Long;

#[derive(SimpleObject)]
struct ResourceDownloadSource {
    urls: Vec<String>,
}
#[derive(SimpleObject)]
struct ChatDownloadSource {
    message_id: ID,
    peer_id: ID,
    file_ids: Vec<String>,
}
#[derive(Union)]
enum DownloadSource {
    Resources(ResourceDownloadSource),
    Chat(ChatDownloadSource),
}
#[derive(SimpleObject)]
struct DownloadFailure {
    path: String,
    error: String,
}
#[derive(SimpleObject)]
pub(super) struct Download {
    id: ID,
    title: String,
    source: DownloadSource,
    target_dir: String,
    zip_name: Option<String>,
    status: Status,
    downloaded_bytes: Long,
    total_bytes: Long,
    bytes_per_second: Long,
    total_files: i32,
    done_files: i32,
    current_file: String,
    error: String,
    failures: Vec<DownloadFailure>,
}
impl From<downloads::Download> for Download {
    fn from(download: downloads::Download) -> Self {
        Self {
            id: download.id.into(),
            title: download.title,
            source: match download.source {
                Source::Resources { urls } => {
                    DownloadSource::Resources(ResourceDownloadSource { urls })
                }
                Source::Chat {
                    message_id,
                    peer_id,
                    file_ids,
                } => DownloadSource::Chat(ChatDownloadSource {
                    message_id: message_id.into(),
                    peer_id: peer_id.into(),
                    file_ids,
                }),
            },
            target_dir: download.target_dir,
            zip_name: download.zip_name,
            status: download.status,
            downloaded_bytes: Long(download.downloaded_bytes),
            total_bytes: Long(download.total_bytes),
            bytes_per_second: Long(download.bytes_per_second),
            total_files: download.total_files,
            done_files: download.done_files,
            current_file: download.current_file,
            error: download.error,
            failures: download
                .failures
                .into_iter()
                .map(|failure| DownloadFailure {
                    path: failure.path,
                    error: failure.error,
                })
                .collect(),
        }
    }
}
