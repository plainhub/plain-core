use serde_json::json;

#[tokio::test]
async fn conversation_reads_prepare_once_in_background_and_skip_searches_and_pages() {
    use plain_rs::chat::enums::{DeviceType, PeerStatus};
    use std::sync::Arc;
    let directory = tempfile::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let core = crate::server::ContentServer::start(
        directory.path(),
        &plain_rs::utils::base64::base64_encode(&[9; 32]),
        prefs,
    )
    .unwrap();
    let state = core.runtime_state();
    let mut peer = plain_rs::db::DPeer::new("peer", "Fixture", "", Some(1), DeviceType::Phone);
    peer.status = PeerStatus::Paired;
    peer.key = plain_rs::utils::base64::base64_encode(&[7; 32]);
    peer.public_key = plain_rs::utils::base64::base64_encode(&[8; 32]);
    state.db.upsert_peer(&peer);
    let (generation, mut requests) = state.host.connect();
    let executor = core.graphql_executor();
    for (query, offset, limit) in [
        ("", 0, 20),
        ("peer:local", 0, 20),
        ("channel:group", 0, 20),
        ("peer:peer text:needle", 0, 20),
        ("peer:peer", 20, 20),
        ("peer:peer", 0, 0),
        ("peer:peer latest:true", 0, 20),
        ("peer:unknown", 0, 20),
    ] {
        let response = executor
            .execute(async_graphql::Request::new(format!(
                "{{chatRecords(query:{},offset:{offset},limit:{limit}){{id}}}}",
                json!(query),
            )))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
    }
    tokio::task::yield_now().await;
    assert!(requests.try_recv().is_err());
    let response = executor
        .execute(async_graphql::Request::new(
            r#"{chatCount(query:"peer:peer") chatConversations{id}}"#,
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(requests.try_recv().is_err());
    for index in 0..20 {
        let document = if index % 2 == 0 {
            r#"{chatRecords(query:"peer:peer",offset:0,limit:20){id}}"#
        } else {
            r#"{chatItems(target:"peer:peer",query:"",offset:0,limit:20){id}}"#
        };
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            executor.execute(async_graphql::Request::new(document)),
        )
        .await
        .unwrap();
        assert!(response.errors.is_empty(), "{:?}", response.errors);
    }
    let request = tokio::time::timeout(std::time::Duration::from_secs(1), requests.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(request["method"], "peerTransportCapabilities");
    tokio::task::yield_now().await;
    assert!(requests.try_recv().is_err());
    state
        .host
        .reply(generation, json!({"id":request["id"],"result":[]}))
        .unwrap();
    tokio::task::yield_now().await;
    assert!(requests.try_recv().is_err());
    core.shutdown().await;
}
