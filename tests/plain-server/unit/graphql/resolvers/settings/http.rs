use crate::http::graphql;
use crate::server::ContentServer;
use crate::server::ServerState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
};
use plain_rs::prefs::Prefs;
use serde_json::{Value, json};
use std::sync::Arc;

fn device() -> (tempfile::TempDir, ContentServer, HeaderMap) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    plain_rs::prefs::security::bootstrap(&prefs).unwrap();
    let server =
        ContentServer::start(dir.path(), &plain_rs::base64_encode(&[3; 32]), prefs).unwrap();
    let session = serde_json::to_value(
        plain_rs::services::sessions::create(&server.runtime_state(), "Controller".into()).unwrap(),
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
        axum::body::Bytes::from(json!({"query":document}).to_string()),
    )
    .await;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}
#[tokio::test]
async fn remote_settings_authentication_device_isolation_and_disable_reply() {
    let (_a, first, headers) = device();
    let (_b, second, other) = device();
    let (status, result) = call(first.runtime_state(), &headers, r#"mutation{font:setPref(key:"editor_font_size",value:20) interval:setPref(key:"feed_auto_refresh_interval",value:3600)}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert!(result.get("errors").is_none(), "{result}");
    assert_eq!(result["data"]["font"], true);
    let (_, result) = call(first.runtime_state(), &headers, "{prefs}").await;
    assert_eq!(result["data"]["prefs"]["editor_font_size"], 20);
    assert_eq!(result["data"]["prefs"]["feed_auto_refresh_interval"], 3600);
    let (_, result) = call(second.runtime_state(), &other, "{prefs} ").await;
    assert!(result["data"]["prefs"]["editor_font_size"].is_null());
    let (_, result) = call(first.runtime_state(), &headers, "{systemPrefs}").await;
    assert!(
        result["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("Unknown field")
    );
    assert_eq!(
        call(second.runtime_state(), &headers, "{prefs}").await.0,
        StatusCode::UNAUTHORIZED
    );
    let (_, result) = call(
        first.runtime_state(),
        &headers,
        r#"mutation{setPref(key:"desktop_access",value:false)}"#,
    )
    .await;
    assert!(result["errors"].is_array(), "{result}");
    assert_eq!(
        call(first.runtime_state(), &headers, "{prefs}").await.0,
        StatusCode::OK
    );
    let native = first.native_graphql().unwrap();
    let disabled = native
        .execute(async_graphql::Request::new(
            r#"mutation { setPref(key:"desktop_access",value:false) }"#,
        ))
        .await;
    assert!(disabled.errors.is_empty(), "{:?}", disabled.errors);
    assert_eq!(disabled.data.into_json().unwrap()["setPref"], true);
    assert_eq!(
        call(first.runtime_state(), &headers, "{prefs}").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(second.runtime_state(), &other, "{prefs}").await.0,
        StatusCode::OK
    );
    let snapshot = native.execute(async_graphql::Request::new("{prefs}")).await;
    assert!(snapshot.errors.is_empty(), "{:?}", snapshot.errors);
    assert_eq!(
        snapshot.data.into_json().unwrap()["prefs"]["desktop_access"],
        false
    );
    first.shutdown().await;
    second.shutdown().await;
}
