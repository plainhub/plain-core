use super::*;
use crate::graphql::executor::{RequestContext, execute_graphql};

#[tokio::test]
async fn retired_management_roots_are_unavailable_to_local_and_authenticated_remote_graphql() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::assembly::server::ContentServer::open_with_cache(
        &directory.path().join("data"),
        &directory.path().join("cache"),
    )
    .unwrap();
    let state = core.runtime_state();
    let before = state.db.session_list().unwrap().len();
    let native = core.native_graphql().unwrap();
    for (field, document) in [
        ("resetPassword", r#"mutation{resetPassword}"#),
        ("resetUrlToken", r#"mutation{resetUrlToken}"#),
        ("resetAdbToken", r#"mutation{resetAdbToken}"#),
        ("setNotificationFilterMode", r#"mutation{setNotificationFilterMode(mode:"allowlist"){affectedCount}}"#),
        ("clearNotificationFilterApps", r#"mutation{clearNotificationFilterApps{affectedCount}}"#),
        ("pairingCode", r#"{pairingCode}"#),
        ("isPairingRequestCurrent", r#"{isPairingRequestCurrent(id:"fixture",signature:"stale")}"#),
        ("logoutPeer", r#"mutation{logoutPeer(id:"fixture")}"#),
        ("controlPeerStatus", r#"mutation{controlPeerStatus(action:RECONNECT)}"#),
        ("filePaths", r#"{filePaths(paths:[])}"#),
        ("importNoteImage", r#"mutation{importNoteImage(source:"missing",fileName:"fixture.png")}"#),
        ("importPomodoroSound", r#"mutation{importPomodoroSound(source:"missing")}"#),
        ("dlnaReceiver", r#"{dlnaReceiver}"#),
        ("controlDlnaReceiver", r#"mutation{controlDlnaReceiver(input:{action:RETRY})}"#),
        ("scanHistory", r#"{scanHistory}"#),
        ("addScanHistory", r#"mutation{addScanHistory(text:"fixture")}"#),
        ("removeScanHistory", r#"mutation{removeScanHistory(text:"fixture")}"#),
        ("acquireMdnsDebugSession", r#"mutation{acquireMdnsDebugSession(token:"fixture"){revision}}"#),
        ("releaseMdnsDebugSession", r#"mutation{releaseMdnsDebugSession(token:"fixture"){revision}}"#),
        ("tlsCertificateSignature", "{tlsCertificateSignature}"),
        ("systemSettings", "{systemSettings{password}}"),
        ("userSettings", "{userSettings{service}}"),
        ("systemPrefs", "{systemPrefs}"),
        ("userPrefs", "{userPrefs}"),
        ("setSystemPref", r#"mutation{setSystemPref(key:"locale",value:"en"){affectedCount}}"#),
        ("setUserPref", r#"mutation{setUserPref(key:"locale",value:"en")}"#),
        ("removeSystemPref", r#"mutation{removeSystemPref(key:"locale"){affectedCount}}"#),
        ("removeUserPref", r#"mutation{removeUserPref(key:"locale")}"#),
        (
            "toggleNotificationApp",
            "mutation{toggleNotificationApp(packageName:\"fixture\"){affectedCount}}",
        ),
        ("cycleAudioPlayMode", "mutation{cycleAudioPlayMode}"),
        ("httpServer", "{httpServer{revision}}"),
        ("verifyAdbToken", "{verifyAdbToken(token:\"fixture\")}"),
        (
            "restartHttpServer",
            "mutation{restartHttpServer(httpPort:0){revision}}",
        ),
        (
            "regenerateTlsCertificate",
            "mutation{regenerateTlsCertificate}",
        ),
        (
            "importTlsCertificatePem",
            "mutation{importTlsCertificatePem(certificatePem:\"invalid\",privateKeyPem:\"invalid\")}",
        ),
        (
            "importTlsCertificatePkcs12",
            "mutation{importTlsCertificatePkcs12(data:\"invalid\",password:\"fixture\")}",
        ),
        (
            "createSession",
            "mutation{createSession(name:\"Fixture\"){clientId}}",
        ),
        (
            "confirmWebLogin",
            "mutation{confirmWebLogin(requestId:\"fixture\",allowed:true)}",
        ),
        (
            "stageAppRestore",
            "mutation{stageAppRestore(path:\"fixture\")}",
        ),
        ("exportAppBackup", "mutation{exportAppBackup}"),
        (
            "createCrashReport",
            "mutation{createCrashReport(report:\"fixture\")}",
        ),
        (
            "startAppUpdateDownload",
            "mutation{startAppUpdateDownload{taskId}}",
        ),
        (
            "cancelAppUpdateDownload",
            "mutation{cancelAppUpdateDownload{taskId}}",
        ),
        ("checkAppUpdate", "mutation{checkAppUpdate}"),
    ] {
        let local = native.execute(async_graphql::Request::new(document)).await;
        assert!(
            local
                .errors
                .iter()
                .any(|error| error.message.contains("Unknown field")
                    && error.message.contains(field)),
            "{document}: {:?}",
            local.errors
        );
        let remote = execute_graphql(
            &state.public,
            async_graphql::Request::new(document),
            RequestContext::authenticated_remote(state.clone(), "fixture".into()),
        )
        .await;
        assert!(
            remote
                .errors
                .iter()
                .any(|error| error.message.contains("Unknown field")
                    && error.message.contains(field)),
            "{document}: {:?}",
            remote.errors
        );
    }
    assert_eq!(state.db.session_list().unwrap().len(), before);
    assert_eq!(state.http_server.snapshot().revision.0, 0);
    assert!(
        !plain_rs::data_paths::DataPaths::new(&directory.path().join("data"))
            .pending_restore()
            .exists()
    );
    core.stop_core().await;
}

#[tokio::test]
async fn native_management_handles_reject_every_operation_after_core_shutdown() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::assembly::server::ContentServer::open_with_cache(
        &directory.path().join("data"),
        &directory.path().join("cache"),
    )
    .unwrap();
    let management = core.native_management().unwrap();
    macro_rules! stopped {
        ($result:expr) => {
            assert_eq!($result.err().as_deref(), Some("Rust core stopped"));
        };
    }
    core.stop_core().await;
    stopped!(core.native_management());
    stopped!(management.restart_http_server(Some(0), Some(0)).await);
    stopped!(management.reset_password());
    stopped!(management.reset_url_token());
    stopped!(management.reset_adb_token());
    stopped!(management.set_notification_filter_mode("allowlist".into()).await);
    stopped!(management.clear_notification_filter_apps().await);
    stopped!(management.scan_history());
    stopped!(management.add_scan_history("stopped".into()));
    stopped!(management.remove_scan_history("stopped".into()));
    stopped!(management.file_paths(vec!["stopped".into()]));
    stopped!(management.import_note_image("stopped".into(), "image.png".into()).await);
    stopped!(management.import_pomodoro_sound("stopped".into()).await);
    stopped!(management.pairing_code().await);
    stopped!(management.is_pairing_request_current("stopped".into(), "signature".into()));
    stopped!(management.logout_peer("stopped".into()).await);
    stopped!(management.reconnect_peers().await);
    stopped!(management.dlna_receiver().await);
    stopped!(management.retry_dlna_receiver().await);
    stopped!(management.remove_dlna_rule("stopped".into(), true).await);
    stopped!(management.accept_dlna_cast(false).await);
    stopped!(management.reject_dlna_cast(false).await);
    stopped!(management.acquire_mdns_debug_session("stopped".into()).await);
    stopped!(management.release_mdns_debug_session("stopped".into()).await);
    stopped!(management.regenerate_tls_certificate().await);
    stopped!(
        management
            .import_tls_certificate_pem("invalid".into(), "invalid".into())
            .await
    );
    stopped!(
        management
            .import_tls_certificate_pkcs12(vec![], "fixture".into())
            .await
    );
    stopped!(management.create_session("Fixture".into()));
    stopped!(management.confirm_web_login("fixture".into(), true).await);
    stopped!(management.web_login_requests(None));
    stopped!(management.stage_app_restore("fixture".into()).await);
    stopped!(management.export_app_backup().await);
    stopped!(management.create_crash_report("fixture".into()).await);
    stopped!(management.verify_adb_token("fixture".into()));
    stopped!(management.preferences());
    stopped!(management.preferences());
    stopped!(management.preferences());
    stopped!(management.toggle_notification_app("fixture".into()).await);
    stopped!(management.start_app_update_download().await);
    stopped!(management.cancel_app_update_download().await);
    stopped!(management.check_app_update(false).await);
}

#[tokio::test]
async fn native_preference_reads_and_notification_changes_preserve_shared_state_and_events() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::assembly::server::ContentServer::open_with_cache(
        &directory.path().join("data"),
        &directory.path().join("cache"),
    )
    .unwrap();
    let state = core.runtime_state();
    let management = core.native_management().unwrap();
    state.prefs.set("password", "fixture-password").unwrap();
    state
        .prefs
        .set("url_token", "fixture-resource-token")
        .unwrap();
    let system = management.preferences().unwrap();
    assert_eq!(system["password"], "fixture-password");
    assert_eq!(system["url_token"], "fixture-resource-token");
    for key in system.keys() {
        assert!(
            plain_rs::prefs::pref_keys::access(key).is_some()
                || matches!(key.as_str(), plain_rs::prefs::pref_keys::SIGNATURE_PUBLIC_KEY | plain_rs::prefs::pref_keys::UPDATE_INFO | plain_rs::prefs::pref_keys::APP_UPDATE_AVAILABLE | plain_rs::prefs::pref_keys::ONBOARDING_COMPLETED),
            "Unexpected native preference key: {key}"
        );
    }
    for private in ["signature_key_pair", "master_secret", "signatureKeyPair", "masterSecret"] {
        assert!(
            system.get(private)
                .is_none()
        );
    }
    let mut events = state.events.subscribe();
    for expected in [serde_json::json!(["fixture.app"]), serde_json::json!([])] {
        assert!(
            management
                .toggle_notification_app("fixture.app".into())
                .await
                .unwrap()
        );
        assert_eq!(
            management.preferences().unwrap()["notification_filter"]["apps"],
            expected
        );
        let mut changed = 0;
        let mut refreshed = 0;
        while let Ok(event) = events.try_recv() {
            changed += usize::from(event.event_type == "PREFS_UPDATED");
            refreshed += usize::from(event.event_type == "NOTIFICATION_REFRESHED");
        }
        assert_eq!(changed, 1);
        assert_eq!(refreshed, 1);
    }
    core.stop_core().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn native_certificate_rotation_uses_the_server_identity_and_invalid_import_preserves_it() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::assembly::server::ContentServer::open_with_cache(
        &directory.path().join("data"),
        &directory.path().join("cache"),
    )
    .unwrap();
    let state = core.runtime_state();
    let certificate = || {
        let (certificate, _) = state.tls_identity.read().unwrap().identity().unwrap();
        certificate
    };
    let first = certificate();
    let management = core.native_management().unwrap();
    let rotated = management.regenerate_tls_certificate().await.unwrap();
    assert!(!rotated.is_empty());
    let current = certificate();
    assert_ne!(first, current);
    use rustls::pki_types::pem::PemObject;
    let der = rustls::pki_types::CertificateDer::pem_slice_iter(&current)
        .next()
        .unwrap()
        .unwrap();
    assert!(der.as_ref().ends_with(&rotated));
    assert!(
        management
            .import_tls_certificate_pem("invalid".into(), "invalid".into())
            .await
            .is_err()
    );
    assert_eq!(certificate(), current);
    core.stop_core().await;
}
