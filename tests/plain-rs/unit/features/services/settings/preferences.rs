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
                plain_libs::serde_json::json!("Synthetic")
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
        plain_libs::serde_json::to_string(&crate::features::prefs::settings::Settings::default().device_name)
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
                plain_libs::serde_json::json!(70000)
            )
            .await
            .is_err()
    );
    assert!(
        settings
            .set(
                crate::features::prefs::pref_keys::REMOTE_ACCESS,
                plain_libs::serde_json::json!("false")
            )
            .await
            .is_err()
    );
    assert_eq!(prefs.entries(), before);
    assert!(receiver.try_recv().is_err());
}

#[tokio::test]
async fn media_exclusions_validate_paths_and_refresh_media_without_scanning() {
    let prefs = Prefs::in_memory();
    let (events, mut receiver) = broadcast::channel(16);
    let settings = Preferences {
        prefs: &prefs,
        events: Some(&events),
        server: None,
    };
    for value in [
        plain_libs::serde_json::json!("/hidden"),
        plain_libs::serde_json::json!(["relative"]),
        plain_libs::serde_json::json!([1]),
    ] {
        assert!(settings.set("media_excluded_dirs", value).await.is_err());
        assert!(
            prefs
                .get::<Vec<String>>("media_excluded_dirs")
                .unwrap()
                .is_none()
        );
        assert!(receiver.try_recv().is_err());
    }
    assert!(
        settings
            .set(
                "media_excluded_dirs",
                plain_libs::serde_json::json!(["/hidden"])
            )
            .await
            .unwrap()
    );
    assert_eq!(receiver.try_recv().unwrap().event_type, "PREFS_UPDATED");
    let event = receiver.try_recv().unwrap();
    assert_eq!(
        event.event_type,
        crate::infrastructure::events::MEDIA_UPDATED
    );
    for kind in [
        crate::features::enums::DataType::Image,
        crate::features::enums::DataType::Video,
        crate::features::enums::DataType::Audio,
        crate::features::enums::DataType::Doc,
    ] {
        assert!(crate::features::services::files::catalog_events::includes(
            &event.payload,
            kind
        ));
    }
    assert!(receiver.try_recv().is_err());
    assert!(
        !settings
            .set(
                "media_excluded_dirs",
                plain_libs::serde_json::json!(["/hidden"])
            )
            .await
            .unwrap()
    );
    assert!(receiver.try_recv().is_err());
    assert!(settings.remove("media_excluded_dirs").await.unwrap());
    assert_eq!(receiver.try_recv().unwrap().event_type, "PREFS_UPDATED");
    assert_eq!(
        receiver.try_recv().unwrap().event_type,
        crate::infrastructure::events::MEDIA_UPDATED
    );
    assert!(receiver.try_recv().is_err());
}
