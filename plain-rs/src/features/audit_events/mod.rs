pub use crate::infrastructure::error_code::ErrorCode;
mod types;
use serde::{Deserialize, Serialize};
pub use types::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditEvent {
    Login(LoginEventData),
    LoginFailed(LoginFailedEventData),
    Logout(SessionEventData),
    RevokeSession(SessionEventData),
    UpdateDeviceName(DeviceNameEventData),
    Mount(VolumeEventData),
    MountFailed(VolumeFailedEventData),
    Unmount(VolumeEventData),
    FormatDisk(DiskFormatEventData),
    FormatDiskFailed(DiskFormatFailedEventData),
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "graphql", derive(plain_graphql::Union))]
pub enum EventData {
    Login(LoginEventData),
    LoginFailed(LoginFailedEventData),
    Session(SessionEventData),
    DeviceName(DeviceNameEventData),
    Volume(VolumeEventData),
    VolumeFailed(VolumeFailedEventData),
    DiskFormat(DiskFormatEventData),
    DiskFormatFailed(DiskFormatFailedEventData),
}

impl AuditEvent {
    pub fn kind(&self) -> EventType {
        match self {
            Self::Login(_) => EventType::Login,
            Self::LoginFailed(_) => EventType::LoginFailed,
            Self::Logout(_) => EventType::Logout,
            Self::RevokeSession(_) => EventType::RevokeSession,
            Self::UpdateDeviceName(_) => EventType::UpdateDeviceName,
            Self::Mount(_) => EventType::Mount,
            Self::MountFailed(_) => EventType::MountFailed,
            Self::Unmount(_) => EventType::Unmount,
            Self::FormatDisk(_) => EventType::FormatDisk,
            Self::FormatDiskFailed(_) => EventType::FormatDiskFailed,
        }
    }

    pub fn data(self) -> EventData {
        match self {
            Self::Login(data) => EventData::Login(data),
            Self::LoginFailed(data) => EventData::LoginFailed(data),
            Self::Logout(data) | Self::RevokeSession(data) => EventData::Session(data),
            Self::UpdateDeviceName(data) => EventData::DeviceName(data),
            Self::Mount(data) | Self::Unmount(data) => EventData::Volume(data),
            Self::MountFailed(data) => EventData::VolumeFailed(data),
            Self::FormatDiskFailed(data) => EventData::DiskFormatFailed(data),
            Self::FormatDisk(data) => EventData::DiskFormat(data),
        }
    }
}
