use async_graphql::{ID, SimpleObject};
use plain_rs::types::Long;
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
#[cfg(any(
    test,
    all(feature = "mobile", any(target_os = "android", target_os = "ios"))
))]
#[derive(SimpleObject, Deserialize)]
pub struct SmsProviderChange {
    pub uris: Vec<String>,
}
#[cfg(any(
    test,
    all(feature = "mobile", any(target_os = "android", target_os = "ios"))
))]
#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmsSendResult {
    pub request_id: Option<ID>,
    pub success: bool,
    pub result_code: Option<i32>,
}
#[cfg(any(
    test,
    all(feature = "mobile", any(target_os = "android", target_os = "ios"))
))]
#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MmsSendResult {
    pub pending_id: ID,
    pub success: bool,
    pub result_code: Option<i32>,
}
#[derive(SimpleObject, Deserialize)]
pub struct ClipboardChange {
    pub text: String,
    pub sensitive: bool,
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
    pub status: String,
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
    pub from_id: ID,
    pub from_name: String,
    pub port: i32,
    pub device_type: String,
    pub ecdh_public_key: String,
    pub signature_public_key: String,
    pub timestamp: Long,
    pub ips: Vec<String>,
    pub signature: String,
    pub from_ip: String,
    pub aware_supported: bool,
    pub verification_code: String,
}
#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingCompletedNotice {
    pub device_id: ID,
    pub device_name: String,
    pub ip: String,
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
    pub today: plain_rs::services::pomodoro::PomodoroToday,
    pub completed_state: Option<plain_rs::services::pomodoro::PomodoroState>,
}
