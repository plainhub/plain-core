use super::AppFeatureType;

#[test]
fn feature_names_match_preference_values() {
    use AppFeatureType::*;
    for (feature, name) in [
        (Audio, "AUDIO"),
        (Sms, "SMS"),
        (Files, "FILES"),
        (Chat, "CHAT"),
        (ScreenMirror, "SCREEN_MIRROR"),
        (CallPhone, "CALL_PHONE"),
    ] {
        let json = plain_libs::serde_json::to_value(feature).unwrap();
        assert_eq!(json, name);
        assert_eq!(
            plain_libs::serde_json::from_value::<AppFeatureType>(json).unwrap(),
            feature
        );
    }
}
