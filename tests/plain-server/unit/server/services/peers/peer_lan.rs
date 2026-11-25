use crate::server::ContentServer;
use plain_libs::serde_json::json;
use plain_rs::features::chat::enums::DeviceType;
use plain_rs::features::chat::enums::PeerStatus;
use plain_rs::features::services::peers::peer_lan::*;
use plain_rs::infrastructure::db::DPeer;
use plain_rs::infrastructure::db::chat_store::SaveMode;
use plain_rs::infrastructure::db::chat_store::peers;
use std::sync::Arc;
fn root(
    id: &str,
) -> (
    plain_libs::temporary::TempDir,
    ContentServer,
    Vec<u8>,
    String,
) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", id).unwrap();
    let (kp, _pk) = plain_libs::crypto::ed25519_generate();
    prefs
        .set(
            "signature_private_key",
            plain_libs::utils::base64::base64_encode(&kp[..32]),
        )
        .unwrap();
    let token = plain_libs::utils::base64::base64_encode(&[3; 32]);
    let server = ContentServer::start(dir.path(), &token, prefs).unwrap();
    (dir, server, kp.to_vec(), token)
}
fn peer(id: &str, port: u16, pk: &[u8]) -> DPeer {
    let mut peer = DPeer::new(id, id, "127.0.0.1", Some(port), DeviceType::Phone);
    peer.status = PeerStatus::Paired;
    peer.key = plain_libs::utils::base64::base64_encode(&[7; 32]);
    peer.public_key = plain_libs::utils::base64::base64_encode(pk);
    peer
}
#[tokio::test]
async fn direct_tls_peer_exchange_uses_xchacha_and_current_identity_without_host() {
    let (_a, a, kp, _) = root("706c6169-fdbb-865c-ab5a-2260e9b9f142");
    let (_b, b, bkp, _) = root("706c6169-e28a-8959-996d-cc20540e4c47");
    let (cert_pem, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (_, https) = b
        .start_public(0, 0, cert_pem.clone(), cert_key.clone())
        .await
        .unwrap();
    let remote = peer("706c6169-e28a-8959-996d-cc20540e4c47", https, &bkp[32..]);
    peers::save(
        &a.runtime_state().core.db,
        &[remote.clone()],
        SaveMode::Insert,
    )
    .unwrap();
    peers::save(
        &b.runtime_state().core.db,
        &[peer("706c6169-fdbb-865c-ab5a-2260e9b9f142", 443, &kp[32..])],
        SaveMode::Insert,
    )
    .unwrap();
    let wire = plain_rs::infrastructure::transport::contract::peer_request(
        "query { __typename }",
        json!({}),
    )
    .unwrap();
    let result = plain_rs::features::services::peers::peer_transport::send(
        &a.runtime_state().core,
        &remote,
        "",
        &[7; 32],
        &wire,
    )
    .await
    .unwrap();
    assert_eq!(result["data"]["__typename"], "Query");
    let mut changed = remote.clone();
    changed.key = plain_libs::utils::base64::base64_encode(&[9; 32]);
    peers::save(&a.runtime_state().core.db, &[changed], SaveMode::Update).unwrap();
    assert!(matches!(
        send(
            &a.runtime_state().core,
            &plain_rs::infrastructure::transport::contract::PeerDispatch {
                peer: &remote, client_id: &a.runtime_state().native_client_id().unwrap(),
                channel_id: "", key: &[7; 32], signing_key: &kp, message_id: None,
            },
            &wire,
            || Ok(())
        )
        .await,
        Err(Failure::Fatal(_))
    ));
    assert_eq!(a.runtime_state().core.lan.capacity.available_permits(), 4);
    a.shutdown().await;
    b.shutdown().await;
}

#[tokio::test]
async fn queued_peer_send_uses_refreshed_address_with_unchanged_pairing() {
    let (_a, a, kp, _) = root("706c6169-fdbb-865c-ab5a-2260e9b9f142");
    let (_b, b, bkp, _) = root("706c6169-e28a-8959-996d-cc20540e4c47");
    let (cert_pem, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (_, https) = b
        .start_public(0, 0, cert_pem.clone(), cert_key.clone())
        .await
        .unwrap();
    let state = a.runtime_state().core;
    let remote = peer("706c6169-e28a-8959-996d-cc20540e4c47", 1, &bkp[32..]);
    peers::save(&state.db, &[remote.clone()], SaveMode::Insert).unwrap();
    peers::save(
        &b.runtime_state().core.db,
        &[peer("706c6169-fdbb-865c-ab5a-2260e9b9f142", 443, &kp[32..])],
        SaveMode::Insert,
    )
    .unwrap();
    let wire = plain_rs::infrastructure::transport::contract::peer_request(
        "query { __typename }",
        json!({}),
    )
    .unwrap();
    let capacity = state
        .lan
        .capacity
        .clone()
        .acquire_many_owned(4)
        .await
        .unwrap();
    let call = plain_rs::features::services::peers::peer_transport::send(
        &state, &remote, "", &[7; 32], &wire,
    );
    tokio::pin!(call);
    assert!(futures_util::poll!(&mut call).is_pending());
    let mut refreshed = remote.clone();
    refreshed.ip = "127.0.0.1".into();
    refreshed.port = Some(https);
    peers::save(&state.db, &[refreshed], SaveMode::Update).unwrap();
    drop(capacity);
    let response = call.await.unwrap();
    assert_eq!(response["data"]["__typename"], "Query");
    let mut without_address = remote.clone();
    without_address.ip.clear();
    let wire = plain_rs::infrastructure::transport::contract::peer_request(
        "query { __typename }",
        json!({}),
    )
    .unwrap();
    let response = plain_rs::features::services::peers::peer_transport::send(
        &state,
        &without_address,
        "",
        &[7; 32],
        &wire,
    )
    .await
    .unwrap();
    assert_eq!(response["data"]["__typename"], "Query");
    assert!(state.transport.active().is_empty());
    assert_eq!(state.lan.capacity.available_permits(), 4);
    a.shutdown().await;
    b.shutdown().await;
}
#[path = "peer_lan/body.rs"]
mod body;
