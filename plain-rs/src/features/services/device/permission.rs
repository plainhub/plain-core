#[derive(serde::Serialize, serde::Deserialize, Copy, Clone, Eq, PartialEq, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "graphql", derive(async_graphql::Enum))]
pub enum Permission {
    WriteExternalStorage,
    ReadSms,
    SendSms,
    ReadContacts,
    WriteContacts,
    ReadCallLog,
    WriteCallLog,
    CallPhone,
    PostNotifications,
    NearbyWifiDevices,
    AccessFineLocation,
    Camera,
    SystemAlertWindow,
    RecordAudio,
    ReadMediaImages,
    ReadMediaVideos,
    ReadMediaAudio,
    NotificationListener,
    ReadPhoneState,
    ReadPhoneNumbers,
    ScheduleExactAlarm,
    QueryAllPackages,
    Adb,
    Clipboard,
}

impl Permission {
    pub fn parse(value: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(value.into())).ok()
    }
}
