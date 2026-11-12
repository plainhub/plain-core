use crate::server::ContentServer;
use plain_graphql::Request;
use plain_libs::protocols::http_client;
use plain_rs::features::chat::enums::{DeviceType, PeerStatus};
use plain_rs::infrastructure::db::DChannel;
use plain_rs::infrastructure::db::DPeer;
use plain_rs::infrastructure::db::chat_store::SaveMode;
use plain_rs::infrastructure::db::chat_store::channels;
use plain_rs::infrastructure::db::chat_store::peers;
use std::sync::Arc;

#[tokio::test]
async fn native_main_record_queries_preserve_addresses_keys_members_and_checked_file_urls() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Arc::new(
        plain_rs::features::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
    );
    prefs.set("client_id", "fixture").unwrap();
    let server = ContentServer::start(
        directory.path(),
        &plain_libs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    let mut peer = DPeer::new("peer", "Device", "127.0.0.1", Some(8443), DeviceType::Phone);
    peer.key = plain_libs::utils::base64::base64_encode(&[7; 32]);
    peer.status = PeerStatus::Paired;
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let mut channel = DChannel::new("Channel", "fixture");
    channel.id = "channel".into();
    channel.key = "channel-key".into();
    channel.members = r#"[{"peerId":"peer","status":"JOINED"}]"#.into();
    channel.version = 5000000001;
    channels::save(&state.db, &[channel], SaveMode::Insert).unwrap();
    let native = server.native_graphql().unwrap();
    let response=native.execute(Request::new(r#"{ peers { id key address { bestIp baseUrl apiUrl statusWsUrl } } chatChannels { key ownerId members { peerId status } version } }"#)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["peers"][0]["key"], peer.key);
    assert_eq!(
        data["peers"][0]["address"]["baseUrl"],
        "https://127.0.0.1:8443"
    );
    assert_eq!(data["peers"].as_array().unwrap().len(), 1);
    assert_eq!(data["chatChannels"][0]["version"], 5000000001_i64);
    assert_eq!(data["chatChannels"][0]["key"], "channel-key");
    assert_eq!(data["chatChannels"][0]["members"][0]["peerId"], "peer");
    let request = || Request::new(r#"{ peerFileUrl(id: "peer", fileId: "a +?#%") }"#);
    let response = native.execute(request()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let url = http_client::Url::parse(data["peerFileUrl"].as_str().unwrap()).unwrap();
    assert_eq!(
        url.query_pairs().find(|(key, _)| key == "id").unwrap().1,
        "a +?#%"
    );
    assert_eq!(
        url.query_pairs()
            .find(|(key, _)| key == "action")
            .unwrap()
            .1,
        "download"
    );
    peer.key = plain_libs::utils::base64::base64_encode(&[8; 32]);
    peers::save(&state.db, &[peer], SaveMode::Update).unwrap();
    assert!(native.execute(request()).await.errors.is_empty());
    let response = native
        .execute(Request::new(
            r#"mutation { deletePeer(id:"absent") unpairPeer(id:"absent") }"#,
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["deletePeer"], true);
    assert_eq!(data["unpairPeer"], true);
    state
        .db
        .with_conn(|conn| conn.execute_batch("DROP TABLE chat_channels;"))
        .unwrap();
    let response = native
        .execute(Request::new(r#"{ chatChannels { id } }"#))
        .await;
    assert!(!response.errors.is_empty());
    server.shutdown().await;
}

#[tokio::test]
async fn main_login_and_logout_preserve_pairing_and_live_proxy_addresses() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let executor = core.native_graphql().unwrap();
    let resolve = |id: &str| {
        plain_rs::infrastructure::db::chat_store::peers::get(&state.db, id)
            .unwrap()
            .map(|peer| plain_rs::features::services::peers::peer_address::address(&peer).base_url)
            .filter(|url| !url.is_empty())
    };
    assert_eq!(resolve("peer-a"), None);
    state
        .db
        .login_peer(
            "peer-a",
            "Peer A",
            "127.0.0.1",
            9443,
            DeviceType::Computer,
            "session",
            "public",
            "pairing-key",
        )
        .unwrap();
    assert_eq!(resolve("peer-a"), Some("https://127.0.0.1:9443".into()));
    let query = "{peers{id name port token publicKey online}}";
    let response = executor.execute(plain_graphql::Request::new(query)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let value = response.data.into_json().unwrap();
    assert_eq!(value["peers"][0]["port"], 9443);
    assert_eq!(value["peers"][0]["token"], "session");
    assert!(
        core.native_management()
            .unwrap()
            .logout_peer("peer-a".into())
            .await
            .unwrap()
    );
    let response = executor.execute(plain_graphql::Request::new(query)).await;
    assert_eq!(
        response.data.into_json().unwrap()["peers"][0]["token"],
        plain_libs::serde_json::json!("")
    );
    let mut row = plain_rs::infrastructure::db::chat_store::peers::get(&state.db, "peer-a")
        .unwrap()
        .unwrap();
    assert_eq!(
        row.status,
        plain_rs::features::chat::enums::PeerStatus::Paired
    );
    assert_eq!(row.key, "pairing-key");
    assert!(row.token.is_empty());
    assert_eq!(resolve("peer-a"), Some("https://127.0.0.1:9443".into()));
    row.port = None;
    plain_rs::infrastructure::db::chat_store::peers::save(
        &state.db,
        &[row],
        plain_rs::infrastructure::db::chat_store::SaveMode::Update,
    )
    .unwrap();
    assert_eq!(resolve("peer-a"), None);
    core.stop_core().await;
    assert_eq!(resolve("peer-a"), None);
    assert!(
        !executor
            .execute(plain_graphql::Request::new(query))
            .await
            .errors
            .is_empty()
    );
}

#[tokio::test]
async fn remote_requests_cannot_access_or_change_local_login_sessions() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state
        .db
        .login_peer(
            "fixture",
            "Fixture",
            "127.0.0.1",
            9443,
            DeviceType::Computer,
            "unchanged-token",
            "public",
            "chat-key",
        )
        .unwrap();
    for document in [
        "{peers{token}}",
        r#"mutation{pairDevice(input:{peerId:"peer",mode:MANAGE})}"#,
    ] {
        let response = crate::graphql::executor::execute_graphql(
            &state.public,
            plain_graphql::Request::new(document),
            crate::graphql::executor::RequestContext::remote(Some(state.clone())),
        )
        .await;
        assert_eq!(response.errors.len(), 1);
        assert_eq!(response.errors[0].message, "no_permission");
    }
    let rows = plain_rs::infrastructure::db::chat_store::peers::all(&state.db).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].token, "unchanged-token");
    core.stop_core().await;
}

#[tokio::test]
async fn retired_client_name_mirroring_is_not_available_through_any_main_transport() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let document = r#"mutation{updatePeerName(id:"peer",name:"untrusted")}"#;
    let native = core
        .native_graphql()
        .unwrap()
        .execute(plain_graphql::Request::new(document))
        .await;
    let remote = crate::graphql::executor::execute_graphql(
        &state.public,
        plain_graphql::Request::new(document),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    for response in [native, remote] {
        assert_eq!(response.errors.len(), 1);
        assert!(response.errors[0].message.contains("Unknown field"));
        assert!(response.errors[0].message.contains("updatePeerName"));
    }
    core.stop_core().await;
}
