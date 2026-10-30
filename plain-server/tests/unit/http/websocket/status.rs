use super::*;
use crate::assembly::server::ContentServer;
use futures_util::{SinkExt, StreamExt};
use plain_rs::chat::enums::DeviceType;
use plain_rs::chat::enums::PeerStatus;
use plain_rs::db::DPeer;
use plain_rs::db::Db;
use plain_rs::db::chat_store::SaveMode;
use plain_rs::db::chat_store::peers;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[tokio::test]
async fn public_status_auth_scoped_connections_key_rotation_and_stop_use_rust_without_host() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("background_service_enabled", true).unwrap();
    let path = dir.path().join("plain.db");
    let db = Db::open(&path).unwrap();
    let key = [8; 32];
    let (kp, pk) = plain_rs::crypto::ed25519_generate();
    let mut peer = DPeer::new("peer", "phone", "127.0.0.1", Some(443), DeviceType::Phone);
    peer.status = PeerStatus::Paired;
    peer.key = plain_rs::utils::base64::base64_encode(&key);
    peer.public_key = plain_rs::utils::base64::base64_encode(&pk);
    peers::save(&db, &[peer.clone()], SaveMode::Insert).unwrap();
    let token = plain_rs::utils::base64::base64_encode(&[1; 32]);
    let server = ContentServer::start(path.parent().unwrap(), &token, prefs.clone()).unwrap();
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let (http, _) = server
        .start_public(
            0,
            0,
            cert.cert.pem().into_bytes(),
            cert.key_pair.serialize_pem().into_bytes(),
        )
        .await
        .unwrap();
    let client = reqwest::Client::new();
    for action in ["NOPE", "SNAPSHOT"] {
        assert!(control(&server, action).await.is_err());
    }
    fn snapshot(server: &ContentServer) -> serde_json::Value {
        let state = server.runtime_state();
        state.peer_status.snapshot(&state.db)
    }
    async fn wire(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        key: &[u8],
        kp: &[u8],
        id: &str,
    ) -> Vec<u8> {
        let text = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let challenge: serde_json::Value = serde_json::from_str(&text).unwrap();
        let body = plain_rs::peers::status::handshake(
            key,
            kp,
            id,
            challenge["challenge"].as_str().unwrap(),
        )
        .unwrap();
        socket
            .send(Message::Binary(body.clone().into()))
            .await
            .unwrap();
        body
    }
    let url = format!("ws://127.0.0.1:{http}/status?cid=peer");
    let (mut bad, _) = connect_async(&url).await.unwrap();
    wire(&mut bad, &key, &kp, "other").await;
    assert!(!matches!(bad.next().await,Some(Ok(Message::Text(t))) if t=="ok"));
    assert_eq!(snapshot(&server)["online"], json!([]));
    let (mut first, _) = connect_async(&url).await.unwrap();
    let original = wire(&mut first, &key, &kp, "peer").await;
    assert_eq!(
        first.next().await.unwrap().unwrap().into_text().unwrap(),
        "ok"
    );
    let (mut replay, _) = connect_async(&url).await.unwrap();
    let challenge = replay.next().await.unwrap().unwrap().into_text().unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(&challenge).unwrap()["challenge"].is_string()
    );
    replay.send(Message::Binary(original.into())).await.unwrap();
    assert!(!matches!(replay.next().await, Some(Ok(Message::Text(t))) if t == "ok"));
    assert_eq!(snapshot(&server)["online"], json!(["peer"]));
    let (mut second, _) = connect_async(&url).await.unwrap();
    wire(&mut second, &key, &kp, "peer").await;
    assert_eq!(
        second.next().await.unwrap().unwrap().into_text().unwrap(),
        "ok"
    );
    first.close(None).await.unwrap();
    assert_eq!(snapshot(&server)["online"], json!(["peer"]));
    let new_key = [9; 32];
    let (new_kp, new_pk) = plain_rs::crypto::ed25519_generate();
    peer.key = plain_rs::utils::base64::base64_encode(&new_key);
    peer.public_key = plain_rs::utils::base64::base64_encode(&new_pk);
    peers::save(&db, &[peer], SaveMode::Update).unwrap();
    assert_eq!(snapshot(&server)["online"], json!([]));
    let (mut replacement, _) = connect_async(&url).await.unwrap();
    wire(&mut replacement, &new_key, &new_kp, "peer").await;
    assert_eq!(
        replacement
            .next()
            .await
            .unwrap()
            .unwrap()
            .into_text()
            .unwrap(),
        "ok"
    );
    let _ = tokio::time::timeout(Duration::from_secs(3), second.next())
        .await
        .unwrap();
    assert_eq!(snapshot(&server)["online"], json!(["peer"]));
    let mut stale = server.runtime_state().events.subscribe();
    server.stop_public().await;
    let _ = tokio::time::timeout(Duration::from_secs(3), replacement.next())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if snapshot(&server)["online"] == json!([]) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if stale.recv().await.unwrap().event_type == "PEER_CONNECTIONS_UPDATED" {
                break;
            }
        }
    })
    .await
    .unwrap();
    server.shutdown().await;
}

#[tokio::test]
async fn rust_outgoing_reconnects_same_address_without_native_and_stops_without_revival() {
    use plain_rs::db::chat_store::SaveMode;
    use plain_rs::db::chat_store::peers;
    async fn wait_online(server: &ContentServer, id: &str, online: bool) {
        tokio::time::timeout(Duration::from_secs(12), async {
            loop {
                let row = server
                    .runtime_state()
                    .peer_status
                    .connections
                    .snapshot(&server.runtime_state().db);
                if row["online"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value.as_str() == Some(id))
                    == online
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
    }
    fn configured(dir: &std::path::Path, id: &str) -> (Arc<plain_rs::prefs::Prefs>, Vec<u8>) {
        let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.join("system.json")).unwrap());
        let (kp, pk) = plain_rs::crypto::ed25519_generate();
        prefs.set("background_service_enabled", true).unwrap();
        prefs.set("client_id", id).unwrap();
        prefs
            .set(
                "signature_key_pair",
                plain_rs::utils::base64::base64_encode(&kp),
            )
            .unwrap();
        (prefs, pk.to_vec())
    }
    let a_dir = tempfile::tempdir().unwrap();
    let b_dir = tempfile::tempdir().unwrap();
    let (a_prefs, a_pk) = configured(a_dir.path(), "a");
    let (b_prefs, b_pk) = configured(b_dir.path(), "b");
    let shared = plain_rs::utils::base64::base64_encode(&[17; 32]);
    let a_path = a_dir.path().join("plain.db");
    let b_path = b_dir.path().join("plain.db");
    let a_db = Db::open(&a_path).unwrap();
    let b_db = Db::open(&b_path).unwrap();
    let mut a_peer = DPeer::new("a", "a", "127.0.0.1", Some(443), DeviceType::Phone);
    a_peer.key = shared.clone();
    a_peer.public_key = plain_rs::utils::base64::base64_encode(&a_pk);
    a_peer.status = PeerStatus::Paired;
    peers::save(&b_db, &[a_peer], SaveMode::Insert).unwrap();
    let b = ContentServer::start(
        b_path.parent().unwrap(),
        &plain_rs::utils::base64::base64_encode(&[12; 32]),
        b_prefs.clone(),
    )
    .unwrap();
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let (_, b_https) = b
        .start_public(
            0,
            0,
            cert.cert.pem().into_bytes(),
            cert.key_pair.serialize_pem().into_bytes(),
        )
        .await
        .unwrap();
    let mut b_peer = DPeer::new("b", "b", "127.0.0.1", Some(b_https), DeviceType::Phone);
    b_peer.key = shared;
    b_peer.public_key = plain_rs::utils::base64::base64_encode(&b_pk);
    b_peer.status = PeerStatus::Paired;
    peers::save(&a_db, &[b_peer.clone()], SaveMode::Insert).unwrap();
    let a_token = plain_rs::utils::base64::base64_encode(&[13; 32]);
    let a = ContentServer::start(a_path.parent().unwrap(), &a_token, a_prefs).unwrap();
    a.start_public(
        0,
        0,
        cert.cert.pem().into_bytes(),
        cert.key_pair.serialize_pem().into_bytes(),
    )
    .await
    .unwrap();
    wait_online(&a, "b", true).await;
    wait_online(&b, "a", true).await;
    b.stop_public().await;
    wait_online(&a, "b", false).await;
    b.start_public(
        0,
        b_https,
        cert.cert.pem().into_bytes(),
        cert.key_pair.serialize_pem().into_bytes(),
    )
    .await
    .unwrap();
    wait_online(&a, "b", true).await;
    b_peer.status = PeerStatus::Unpaired;
    peers::save(&a_db, &[b_peer.clone()], SaveMode::Update).unwrap();
    wait_online(&a, "b", false).await;
    wait_online(&b, "a", false).await;
    b_peer.status = PeerStatus::Paired;
    peers::save(&a_db, &[b_peer], SaveMode::Update).unwrap();
    wait_online(&a, "b", true).await;
    a.stop_public().await;
    wait_online(&a, "b", false).await;
    wait_online(&b, "a", false).await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert_eq!(
        a.runtime_state()
            .peer_status
            .connections
            .snapshot(&a.runtime_state().db)["online"],
        json!([])
    );
    a.start_public(
        0,
        0,
        cert.cert.pem().into_bytes(),
        cert.key_pair.serialize_pem().into_bytes(),
    )
    .await
    .unwrap();
    wait_online(&a, "b", true).await;
    a.stop_public().await;
    assert_eq!(control(&a, "RECONNECT").await, Ok(true));
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert_eq!(
        a.runtime_state()
            .peer_status
            .connections
            .snapshot(&a.runtime_state().db)["online"],
        json!([])
    );
    wait_online(&a, "b", false).await;
    a.shutdown().await;
    b.shutdown().await;
}

async fn control(server: &ContentServer, action: &str) -> Result<bool, String> {
    match action {
        "RECONNECT" => server.native_management()?.reconnect_peers().await,
        _ => Err("Unsupported peer control".into()),
    }
}
