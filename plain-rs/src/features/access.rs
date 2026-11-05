#[derive(serde::Serialize, serde::Deserialize, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "graphql", derive(plain_graphql::Enum))]
pub enum AccessFeature {
    Images,
    Videos,
    Audio,
    Docs,
    Files,
    Contacts,
    Sms,
    CallLogs,
    CallPhone,
    PhoneNumber,
    Apps,
    Notifications,
    Clipboard,
    Adb,
}
impl AccessFeature {
    pub const FILE_FEATURES: [Self; 5] = [Self::Images, Self::Videos, Self::Audio, Self::Docs, Self::Files];
    pub const ALL: [Self; 14] = [
        Self::Images,
        Self::Videos,
        Self::Audio,
        Self::Docs,
        Self::Files,
        Self::Contacts,
        Self::Sms,
        Self::CallLogs,
        Self::CallPhone,
        Self::PhoneNumber,
        Self::Apps,
        Self::Notifications,
        Self::Clipboard,
        Self::Adb,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Images => "IMAGES",
            Self::Videos => "VIDEOS",
            Self::Audio => "AUDIO",
            Self::Docs => "DOCS",
            Self::Files => "FILES",
            Self::Contacts => "CONTACTS",
            Self::Sms => "SMS",
            Self::CallLogs => "CALL_LOGS",
            Self::CallPhone => "CALL_PHONE",
            Self::PhoneNumber => "PHONE_NUMBER",
            Self::Apps => "APPS",
            Self::Notifications => "NOTIFICATIONS",
            Self::Clipboard => "CLIPBOARD",
            Self::Adb => "ADB",
        }
    }
    pub fn capability(self) -> Option<&'static str> {
        Some(match self {
            Self::Contacts | Self::Sms | Self::CallLogs | Self::CallPhone | Self::PhoneNumber => {
                "TELEPHONY"
            }
            Self::Apps => "PACKAGES",
            Self::Notifications => "NOTIFICATIONS",
            Self::Clipboard => "CLIPBOARD",
            Self::Adb => "ADB",
            Self::Images
            | Self::Videos
            | Self::Audio
            | Self::Docs
            | Self::Files => return None,
        })
    }
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|feature| feature.as_str() == value)
    }
}
