#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppFeatureType {
    Docs,
    Notes,
    Feeds,
    Apps,
    Files,
    Images,
    Audio,
    Videos,
    SoundMeter,
    PomodoroTimer,
    Notifications,
    CheckUpdates,
    ImageEditor,
    Chat,
    Sms,
    Calls,
    Contacts,
    ScreenMirror,
    CallPhone,
}

#[cfg(test)]
#[path = "../../../tests/plain-rs/unit/features/app_feature.rs"]
mod tests;
