use crate::http::graphql;
use crate::server::ContentServer;
use crate::server::ServerState;
use plain_libs::axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
};
use plain_rs::features::prefs::Prefs;
use plain_libs::serde_json::{Value, json};
use std::sync::Arc;

fn device() -> (plain_libs::temporary::TempDir, ContentServer, HeaderMap) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    plain_rs::features::prefs::security::bootstrap(&prefs).unwrap();
    let server = ContentServer::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    let session = plain_libs::serde_json::to_value(
        plain_rs::features::services::sessions::create(
            &server.runtime_state(),
            "Controller".into(),
        )
        .unwrap(),
    )
    .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        "c-id",
        session["client_id"].as_str().unwrap().parse().unwrap(),
    );
    headers.insert(
        "authorization",
        format!("Bearer {}", session["token"].as_str().unwrap())
            .parse()
            .unwrap(),
    );
    (dir, server, headers)
}
async fn call(state: ServerState, headers: &HeaderMap, document: &str) -> (StatusCode, Value) {
    let response = graphql::call(
        State(state),
        headers.clone(),
        plain_libs::axum::body::Bytes::from(json!({"query":document}).to_string()),
    )
    .await;
    let status = response.status();
    let body = plain_libs::axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    (status, plain_libs::serde_json::from_slice(&body).unwrap_or(Value::Null))
}
#[tokio::test]
async fn remote_settings_authentication_device_isolation_and_disable_reply() {
    let (_a, first, headers) = device();
    let (_b, second, other) = device();
    let (status, result) = call(first.runtime_state(), &headers, r#"mutation{font:setPref(key:"editor_font_size",value:20) interval:setPref(key:"feed_auto_refresh_interval",value:3600)}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert!(result.get("errors").is_none(), "{result}");
    assert_eq!(result["data"]["font"], true);
    let (_, result) = call(
        first.runtime_state(),
        &headers,
        r#"{prefs(keys:["editor_font_size","feed_auto_refresh_interval"])}"#,
    )
    .await;
    assert_eq!(result["data"]["prefs"]["editor_font_size"], 20);
    assert_eq!(result["data"]["prefs"]["feed_auto_refresh_interval"], 3600);
    let (_, result) = call(
        second.runtime_state(),
        &other,
        r#"{prefs(keys:["editor_font_size","feed_auto_refresh_interval"])} "#,
    )
    .await;
    assert!(result["data"]["prefs"]["editor_font_size"].is_null());
    let (_, result) = call(first.runtime_state(), &headers, "{systemPrefs}").await;
    assert!(
        result["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("Unknown field")
    );
    assert_eq!(
        call(
            second.runtime_state(),
            &headers,
            r#"{prefs(keys:["editor_font_size","feed_auto_refresh_interval"])}"#
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (_, result) = call(
        first.runtime_state(),
        &headers,
        r#"mutation{setPref(key:"remote_access",value:false)}"#,
    )
    .await;
    assert!(result["errors"].is_array(), "{result}");
    assert_eq!(
        call(
            first.runtime_state(),
            &headers,
            r#"{prefs(keys:["editor_font_size","feed_auto_refresh_interval"])}"#
        )
        .await
        .0,
        StatusCode::OK
    );
    let native = first.native_graphql().unwrap();
    let disabled = native
        .execute(plain_graphql::Request::new(
            r#"mutation { setPref(key:"remote_access",value:false) }"#,
        ))
        .await;
    assert!(disabled.errors.is_empty(), "{:?}", disabled.errors);
    assert_eq!(disabled.data.into_json().unwrap()["setPref"], true);
    assert_eq!(
        call(
            first.runtime_state(),
            &headers,
            r#"{prefs(keys:["editor_font_size","feed_auto_refresh_interval"])}"#
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            second.runtime_state(),
            &other,
            r#"{prefs(keys:["editor_font_size","feed_auto_refresh_interval"])}"#
        )
        .await
        .0,
        StatusCode::OK
    );
    let snapshot = native
        .execute(plain_graphql::Request::new(
            r#"{prefs(keys:["remote_access"])}"#,
        ))
        .await;
    assert!(snapshot.errors.is_empty(), "{:?}", snapshot.errors);
    assert_eq!(
        snapshot.data.into_json().unwrap()["prefs"]["remote_access"],
        false
    );
    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn authenticated_developer_apis_follow_mode_changes_without_relogin() {
    let (_directory, server, headers) = device();
    let state = server.runtime_state();
    for enabled in [false, true, false] {
        state.prefs.set("developer_mode", enabled).unwrap();
        for document in ["{prefs}", "{dbPath}", "{dbTables{name}}", "{appLogPath}"] {
            let (status, result) = call(state.clone(), &headers, document).await;
            assert_eq!(status, StatusCode::OK);
            if enabled {
                assert!(result.get("errors").is_none(), "{document}: {result}");
                if document == "{prefs}" {
                    assert_eq!(
                        result["data"]["prefs"],
                        Value::Object(state.prefs.entries().into_iter().collect())
                    );
                }
            } else {
                assert_eq!(
                    result["errors"][0]["message"], "No permission: developer mode is disabled",
                    "{document}: {result}"
                );
            }
        }
        let (_, result) = call(
            state.clone(),
            &headers,
            r#"{prefs(keys:["app_home_features"])}"#,
        )
        .await;
        assert!(result.get("errors").is_none(), "{result}");
        assert_eq!(
            result["data"]["prefs"]["app_home_features"],
            "IMAGES|VIDEOS|AUDIO|DOCS|FILES|NOTES|FEEDS"
        );
    }
    server.shutdown().await;
}
