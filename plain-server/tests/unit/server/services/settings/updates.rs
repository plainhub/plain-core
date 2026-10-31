use crate::server::ContentServer;
use async_graphql::Request;
use plain_rs::prefs::pref_keys::AUTO_CHECK_UPDATE;
use plain_rs::prefs::pref_keys::SKIP_VERSION;
use plain_rs::services::settings::updates::*;
use serde_json::json;

#[tokio::test]
async fn native_update_check_uses_capabilities_and_skips_network_when_automatic_checks_are_disabled()
 {
    for supported in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let core = ContentServer::open(dir.path()).unwrap();
        let state = core.runtime_state().core;
        state.prefs.set(AUTO_CHECK_UPDATE, false).unwrap();
        let (generation, mut requests) = state.host.connect();
        let host = state.host.clone();
        let worker = tokio::spawn(async move {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "systemCapabilities");
            host.reply(generation,json!({"id":request["id"],"result":if supported {vec!["APP_UPDATES"]} else {vec![]}})).unwrap();
            requests
        });
        let response = core
            .native_management()
            .unwrap()
            .check_app_update(true)
            .await;
        if supported {
            assert_eq!(response.unwrap(), AppUpdateStatus::NotDue);
        } else {
            assert!(
                response
                    .err()
                    .unwrap()
                    .contains("unsupported capability APP_UPDATES")
            );
        }
        assert!(worker.await.unwrap().try_recv().is_err());
        assert_eq!(
            plain_rs::services::settings::updates::info(&state)
                .unwrap()
                .check_update_time,
            0
        );
        core.stop_core().await;
    }
}

#[tokio::test]
async fn main_settings_derive_update_availability_from_current_version_and_skip() {
    let dir = tempfile::tempdir().unwrap();
    let core = ContentServer::open(dir.path()).unwrap();
    let state = core.runtime_state().core;
    let (_generation, mut requests) = state.host.connect();
    for (skip, available) in [("", true), ("999.0.0", false)] {
        update(&state, json!({"newVersion":"999.0.0"})).unwrap();
        state.prefs.set(SKIP_VERSION, skip).unwrap();
        let value = core.native_management().unwrap().preferences().unwrap();
        assert_eq!(value["app_update_available"], available);
    }
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn main_update_snapshot_reuses_cached_result_and_projects_release_without_host_business_calls()
 {
    let dir = tempfile::tempdir().unwrap();
    let core = ContentServer::open(dir.path()).unwrap();
    let state = core.runtime_state().core;
    update(&state, json!({"latestRelease":{"version":"999.0.0","name":"Release name","url":"https://example.com/releases/latest","publishedAt":"2026-01-02T03:04:05.000Z"}})).unwrap();
    state
        .update_checks
        .seed_cache_for_test(AppUpdateStatus::Available)
        .await;
    let (generation, mut requests) = state.host.connect();
    let host = state.host.clone();
    let worker = tokio::spawn(async move {
        for _ in 0..2 {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "systemCapabilities");
            host.reply(
                generation,
                json!({"id":request["id"],"result":["APP_UPDATES"]}),
            )
            .unwrap();
        }
        requests
    });
    for (skip, expected) in [("", true), ("999.0.0", false)] {
        state.prefs.set(SKIP_VERSION, skip).unwrap();
        let response = core
            .native_graphql()
            .unwrap()
            .execute(Request::new(
                "{ appUpdate { latestVersion hasUpdate url releaseName publishedAt } }",
            ))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(
            response.data.into_json().unwrap()["appUpdate"],
            json!({"latestVersion":"999.0.0","hasUpdate":expected,"url":"https://example.com/releases/latest","releaseName":"Release name","publishedAt":"2026-01-02T03:04:05.000Z"})
        );
    }
    assert!(worker.await.unwrap().try_recv().is_err());
    core.stop_core().await;
}
