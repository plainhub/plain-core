use super::*;
use crate::features::prefs::settings::SettingsPatch;

#[tokio::test]
async fn generic_writes_typed_patches_and_removal_share_derived_events() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&dir.path().join("prefs.json")).unwrap();
    let (events, mut receiver) = broadcast::channel(16);
    let settings = Preferences {
        prefs: &prefs,
        events: Some(&events),
        #[cfg(feature = "runtime")]
        server: None,
    };
    assert!(
        settings
            .set(
                crate::features::prefs::pref_keys::DEVICE_NAME,
                serde_json::json!("Synthetic")
            )
            .await
            .unwrap()
    );
    assert_eq!(receiver.try_recv().unwrap().event_type, "PREFS_UPDATED");
    let event = receiver.try_recv().unwrap();
    assert_eq!(event.event_type, "DEVICE_NAME_UPDATED");
    assert_eq!(event.payload, "\"Synthetic\"");
    assert!(receiver.try_recv().is_err());
    assert!(
        !settings
            .patch(SettingsPatch {
                device_name: Some("Synthetic".into()),
                ..Default::default()
            })
            .await
            .unwrap()
    );
    assert!(receiver.try_recv().is_err());
    assert!(
        settings
            .remove(crate::features::prefs::pref_keys::DEVICE_NAME)
            .await
            .unwrap()
    );
    assert_eq!(receiver.try_recv().unwrap().event_type, "PREFS_UPDATED");
    let event = receiver.try_recv().unwrap();
    assert_eq!(event.event_type, "DEVICE_NAME_UPDATED");
    assert_eq!(
        event.payload,
        serde_json::to_string(&crate::features::prefs::settings::Settings::default().device_name)
            .unwrap()
    );
    assert!(receiver.try_recv().is_err());
}

#[tokio::test]
async fn invalid_generic_values_leave_state_and_events_unchanged() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&dir.path().join("prefs.json")).unwrap();
    let (events, mut receiver) = broadcast::channel(16);
    let settings = Preferences {
        prefs: &prefs,
        events: Some(&events),
        #[cfg(feature = "runtime")]
        server: None,
    };
    let before = prefs.entries();
    assert!(
        settings
            .set(
                crate::features::prefs::pref_keys::HTTP_PORT,
                serde_json::json!(70000)
            )
            .await
            .is_err()
    );
    assert!(
        settings
            .set(
                crate::features::prefs::pref_keys::REMOTE_ACCESS,
                serde_json::json!("false")
            )
            .await
            .is_err()
    );
    assert_eq!(prefs.entries(), before);
    assert!(receiver.try_recv().is_err());
}
