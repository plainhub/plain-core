use crate::infrastructure::error_code::ErrorCode;
use crate::features::sessions::SessionType;
use crate::infrastructure::types::Long;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "graphql", derive(plain_graphql::Enum))]
pub enum EventType {
    Login,
    LoginFailed,
    Logout,
    RevokeSession,
    UpdateDeviceName,
    Mount,
    MountFailed,
    Unmount,
    FormatDisk,
    FormatDiskFailed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "graphql", derive(plain_graphql::SimpleObject))]
pub struct LoginEventData {
    pub session_type: SessionType,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "graphql", derive(plain_graphql::SimpleObject))]
pub struct LoginFailedEventData {
    pub reason: ErrorCode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "graphql", derive(plain_graphql::SimpleObject))]
pub struct EventSession {
    pub client_id: String,
    pub client_name: String,
    pub r#type: SessionType,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "graphql", derive(plain_graphql::SimpleObject))]
pub struct SessionEventData {
    pub session: EventSession,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "graphql", derive(plain_graphql::SimpleObject))]
pub struct DeviceNameEventData {
    pub old_name: String,
    pub new_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "graphql", derive(plain_graphql::SimpleObject))]
pub struct EventVolume {
    pub id: Option<String>,
    pub name: Option<String>,
    pub device_path: Option<String>,
    pub mount_path: String,
    pub file_system: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "graphql", derive(plain_graphql::SimpleObject))]
pub struct EventFailure {
    pub reason: ErrorCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "graphql", derive(plain_graphql::SimpleObject))]
pub struct VolumeEventData {
    pub volume: EventVolume,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "graphql", derive(plain_graphql::SimpleObject))]
pub struct EventDisk {
    pub id: Option<String>,
    pub name: Option<String>,
    pub path: String,
    pub size_bytes: Option<Long>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "graphql", derive(plain_graphql::SimpleObject))]
pub struct DiskFormatEventData {
    pub disk: EventDisk,
    pub file_system: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "graphql", derive(plain_graphql::SimpleObject))]
pub struct VolumeFailedEventData {
    pub volume: EventVolume,
    pub failure: EventFailure,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "graphql", derive(plain_graphql::SimpleObject))]
pub struct DiskFormatFailedEventData {
    pub disk: EventDisk,
    pub file_system: String,
    pub failure: EventFailure,
}
