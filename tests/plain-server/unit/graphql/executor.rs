use super::*;
use crate::server::ContentServer;
use futures_util::StreamExt;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn quoted_enum_literals_are_rejected_before_local_or_remote_mutations_execute() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("auto_check_update", true).unwrap();
    let token = plain_rs::utils::base64::base64_encode(&[8; 32]);
    let server = ContentServer::start(dir.path(), &token, prefs.clone()).unwrap();
    let state = server.runtime_state();
    let query = r#"mutation { setPref(key: "auto_check_update", value: false) updateScreenMirrorQuality(mode: "HD") }"#;
    for local in [true, false] {
        let response = if local {
            server.graphql_executor().execute(Request::new(query)).await
        } else {
            execute_graphql(
                &state.public,
                Request::new(query),
                RequestContext::remote(Some(state.clone())),
            )
            .await
        };
        assert!(!response.errors.is_empty(), "{response:?}");
        assert_eq!(response.data, async_graphql::Value::Null);
        assert_eq!(prefs.get::<bool>("auto_check_update").unwrap(), Some(true));
    }
    server.shutdown().await;
}

#[tokio::test]
async fn local_and_remote_execute_the_same_envelope_and_preserve_errors() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs
        .set("api_permissions", vec!["WRITE_EXTERNAL_STORAGE"])
        .unwrap();
    let token = plain_rs::utils::base64::base64_encode(&[8; 32]);
    let server = ContentServer::start(dir.path(), &token, prefs).unwrap();
    let state = server.runtime_state();
    let document = json!({"query":"query Count($q: String!) { noteCount(query: $q) } query Other { __typename }", "operationName":"Count", "variables":{"q":""}}).to_string();
    let local = server
        .graphql_executor()
        .execute(serde_json::from_str(&document).unwrap())
        .await;
    let remote = execute_graphql(
        &state.public,
        serde_json::from_str(&document).unwrap(),
        RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert_eq!(
        serde_json::to_value(&local).unwrap(),
        serde_json::to_value(&remote).unwrap()
    );
    assert!(local.errors.is_empty());
    for query in ["{ nonexistentField }", "{ audioHostQueueItems { path } }"] {
        let local = server.graphql_executor().execute(Request::new(query)).await;
        let remote = execute_graphql(
            &state.public,
            Request::new(query),
            RequestContext::remote(Some(state.clone())),
        )
        .await;
        assert!(!local.errors.is_empty());
        assert_eq!(
            serde_json::to_value(local).unwrap(),
            serde_json::to_value(remote).unwrap()
        );
    }
    let query = r#"{ audioQueueItems(query: "", offset: 0, limit: 10) { mediaId } }"#;
    let local = server.graphql_executor().execute(Request::new(query)).await;
    let remote = execute_graphql(
        &state.public,
        Request::new(query),
        RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(local.errors.is_empty(), "{:?}", local.errors);
    assert_eq!(
        serde_json::to_value(local).unwrap(),
        serde_json::to_value(remote).unwrap()
    );
    #[cfg(feature = "http_transport")]
    server.stop_public().await;
    let response = server
        .graphql_executor()
        .execute(Request::new(r#"{ noteCount(query: "") }"#))
        .await;
    assert!(
        response.errors.is_empty(),
        "native execution must survive HTTP shutdown"
    );
    server.shutdown().await;
}

#[test]
fn service_owned_media_notifications_are_not_repeated_by_aliases_or_fragments() {
    for query in [
        "mutation { renamed: deleteMediaItems { affectedCount } }",
        "mutation { ... on Mutation { deleteMediaItems { affectedCount } } }",
        "mutation { ...One ...One } fragment One on Mutation { deleteMediaItems { affectedCount } }",
        "mutation { ...One } fragment One on Mutation { ...Two } fragment Two on Mutation { moveMediaItems { affectedCount } }",
    ] {
        assert!(!content_changes(&Request::new(query)), "{query}");
    }
    assert!(content_changes(&Request::new(
        "mutation { deleteMediaItems { affectedCount } createNote { id } }"
    )));
    assert!(content_changes(&Request::new(
        "mutation { ...One } fragment One on Mutation { createNote { id } }"
    )));
    assert!(!content_changes(&Request::new(
        "mutation { ...One } fragment One on Mutation { ...One }"
    )));
    assert!(!content_changes(
        &Request::new(
            "mutation Media { deleteMediaItems { affectedCount } } mutation Other { createNote { id } }"
        )
        .operation_name("Media")
    ));
}

#[test]
fn cache_only_and_task_owned_mutations_skip_the_generic_content_broadcast() {
    for query in [
        "mutation { preview: cacheSharedFile }",
        "mutation { deleted: deleteClipboardItems(query: \"all:true\") { affectedCount } }",
        "mutation { cacheSharedFile cacheZipEntry cacheAudioCover exportAppLogs }",
        "mutation { ...Cache } fragment Cache on Mutation { cacheSharedFile }",
        "mutation { ... on Mutation { copyFileToDirectory copyFile moveFile renameFile } }",
        "mutation { createFileTask }",
        "mutation { ...Copy } fragment Copy on Mutation { copied: setClipboard(text: \"fixture\") }",
    ] {
        assert!(!content_changes(&Request::new(query)), "{query}");
    }
    assert!(content_changes(&Request::new(
        "mutation { cacheSharedFile createNote }"
    )));
    assert!(content_changes(&Request::new("mutation { clearAppLogs }")));
    assert!(content_changes(&Request::new("mutation { clearAppLogs }")));
}

#[tokio::test]
async fn backup_uses_native_management_and_has_no_remote_graphql_entry() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Arc::new(
        plain_rs::prefs::Prefs::load(&plain_rs::prefs::default_path(directory.path())).unwrap(),
    );
    let token = plain_rs::utils::base64::base64_encode(&[8; 32]);
    let server = ContentServer::start(directory.path(), &token, prefs).unwrap();
    let state = server.runtime_state();
    let remote = execute_graphql(
        &state.public,
        Request::new("mutation { exportAppBackup }"),
        RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(!remote.errors.is_empty());
    let restore = execute_graphql(
        &state.public,
        Request::new("mutation { stageAppRestore(path:\"/not-accessible/backup.zip\") }"),
        RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(!restore.errors.is_empty());
    assert!(
        !plain_rs::data_paths::DataPaths::new(directory.path())
            .pending_restore()
            .exists()
    );
    assert!(
        std::fs::read_dir(state.cache.root())
            .unwrap()
            .filter(|entry| entry.as_ref().unwrap().file_type().unwrap().is_file())
            .next()
            .is_none()
    );
    let path = server
        .native_management()
        .unwrap()
        .export_app_backup()
        .await
        .unwrap();
    assert!(std::path::Path::new(&path).exists());
    server.shutdown().await;
}

#[test]
fn runtime_controls_and_viewer_exports_do_not_trigger_generic_content_refreshes() {
    for document in [
        r#"mutation{acquireDiscovery(token:"token") releaseDiscovery(token:"token")}"#,
        "mutation{...Control} fragment Control on Mutation{alias:releaseDiscovery(token:\"token\")}",
    ] {
        assert!(!content_changes(&Request::new(document)), "{document}");
    }
    assert!(content_changes(&Request::new(
        "mutation{releaseDiscovery(token:\"token\") createNote}"
    )));
}

#[test]
fn versioned_audio_selection_owns_its_content_notification() {
    assert!(!content_changes(&Request::new(
        r#"mutation { advanceAudioSelection(action:NEXT,expectedPath:"song",expectedRevision:1) { revision } }"#
    )));
}

#[test]
fn playback_fact_reports_do_not_trigger_generic_content_refresh() {
    assert!(!content_changes(&Request::new(
        r#"mutation { saveMediaPlayProgress(mediaId:"song",type:AUDIO,positionMs:0) }"#
    )));
}

#[test]
fn shared_batch_chat_and_share_flows_own_their_events() {
    for query in [
        r#"mutation { sendChatItems(targets:["local"],content:"{}") { id } }"#,
        r#"mutation { forwardChatItems(id:"source",targets:["local"]) { id } }"#,
        r#"mutation { sendShare(targets:["local"],shareId:"share") { id } }"#,
        r#"mutation { sendFolderShare(targets:["local"],path:"/folder",name:"Folder") { id } }"#,
    ] {
        assert!(!content_changes(&Request::new(query)));
    }
}

#[test]
fn timer_controls_do_not_invalidate_unrelated_domains() {
    for query in [
        "mutation { startPomodoro(durationSec:30) }",
        "mutation { named: adjustPomodoro(durationSec:30) {state} }",
        "query {pomodoroToday {state}}",
    ] {
        assert!(!content_changes(&Request::new(query)));
    }
    assert!(content_changes(&Request::new(
        "mutation {adjustPomodoro(durationSec:30) {state} deleteBookmarks(ids:[]) {affectedCount}}"
    )));
}

#[tokio::test]
async fn sms_subscription_replays_results_after_its_receiver_is_ready() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let (generation, mut requests) = state.host.connect();
    let host = state.host.clone();
    let responder = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "systemCapabilities" => json!(["SMS"]),
                "systemPermissionFacts" => json!({"granted":{"SEND_SMS":true}}),
                method => panic!("Unexpected host request {method}"),
            };
            host.reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
    });
    state
        .sms_sends
        .register("send".into(), Some("request".into()), 1)
        .unwrap();
    state
        .sms_sends
        .report(
            serde_json::from_value(json!({
                "requestId":"send", "partIndex":0, "partCount":1, "success":true, "resultCode":-1
            }))
            .unwrap(),
        )
        .unwrap();
    let schema = crate::test_support::schema::mobile(state.core.clone());
    let request = RequestContext::local(state.clone())
        .prepare(Request::new(
            "subscription { smsSendResult { requestId success resultCode } }",
        ))
        .unwrap();
    let mut stream = schema.execute_stream(request);
    let response = tokio::time::timeout(std::time::Duration::from_secs(3), stream.next())
        .await
        .unwrap()
        .unwrap();
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["smsSendResult"],
        json!({"requestId":"request","success":true,"resultCode":-1})
    );
    drop(stream);
    state.host.disconnect(generation);
    responder.abort();
    core.shutdown().await;
}
