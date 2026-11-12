use plain_graphql::{ID, SimpleObject};

use plain_rs::infrastructure::types::{Instant, Long};
use serde::Deserialize;

#[derive(SimpleObject, Deserialize)]
pub struct MirrorState {
    pub running: bool,
}
#[derive(SimpleObject, Deserialize)]
pub struct LibraryChange {
    pub types: Vec<i32>,
}
#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedSyncChange {
    pub feed_id: ID,
    pub error: Option<String>,
    pub cancelled: Option<bool>,
}
#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadMergeResult {
    pub file_id: ID,
    pub ok: bool,
    pub value: Option<String>,
    pub merged_size: Option<Long>,
    pub error: Option<String>,
}
#[derive(SimpleObject, Deserialize)]
pub struct SmsProviderChange {
    pub uris: Vec<String>,
}
#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmsSendResult {
    pub request_id: Option<ID>,
    pub success: bool,
    pub result_code: Option<i32>,
}
#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MmsSendResult {
    pub pending_id: ID,
    pub success: bool,
    pub result_code: Option<i32>,
}
#[derive(SimpleObject, Deserialize)]
#[graphql(complex)]
pub struct ClipboardChange {
    pub text: String,
    pub sensitive: bool,
    #[graphql(skip)]
    pub time: Long,
}

#[derive(SimpleObject, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorCodecConfiguration {
    pub annex_b: String,
}

#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelInvite {
    pub channel_id: ID,
    pub channel_name: String,
    pub owner_peer_id: ID,
    pub owner_peer_name: String,
}
#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelInviteCancellation {
    pub channel_id: ID,
    pub owner_peer_id: ID,
}
#[derive(SimpleObject, Deserialize)]
pub struct ChannelChange {
    pub changed: bool,
    pub invite: Option<ChannelInvite>,
    pub cancel: Option<ChannelInviteCancellation>,
}
#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageModelsUpdate {
    pub version: Long,
    pub status: plain_rs::features::services::images::status::ImageSearchStatusType,
    pub download_progress: i32,
    pub error_message: String,
    pub model_size: Long,
    pub model_dir: String,
    pub is_indexing: bool,
    pub total_images: i32,
    pub indexed_images: i32,
}
#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingRequestNotice {
    pub to_id: ID,
    pub target_public_key: String,
    pub from_id: ID,
    pub from_name: String,
    pub port: i32,
    pub device_type: plain_rs::features::services::device::types::DeviceType,
    pub ecdh_public_key: String,
    pub signature_public_key: String,
    pub timestamp: Long,
    pub ips: Vec<String>,
    pub signature: String,
    pub from_ip: String,
    pub aware_supported: bool,
    pub verification_code: String,
    pub identity_changed: bool,
}
#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingCompletedNotice {
    pub device_id: ID,
    pub device_name: String,
    pub ip: String,
    pub port: Option<i32>,
    pub device_type: Option<plain_rs::features::services::device::types::DeviceType>,
    pub request_id: Option<String>,
}
#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingProgressNotice {
    pub device_id: ID,
    pub device_name: String,
    pub error: Option<String>,
}
#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroAction {
    pub today: plain_rs::features::services::pomodoro::PomodoroToday,
    pub completed_state: Option<plain_rs::features::services::pomodoro::PomodoroState>,
}

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/graphql/subscriptions/models.rs"]
mod tests;

#[plain_graphql::ComplexObject]
impl ClipboardChange {
    fn time(&self) -> plain_graphql::Result<Instant> {
        crate::graphql::types::instant_from_millis(self.time.0)
    }
}
