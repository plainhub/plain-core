use super::*;
use crate::features::chat::enums::DeviceType;
use crate::features::chat::enums::PeerStatus;
use crate::infrastructure::db::chat_store::SaveMode;
use crate::test_runtime::Core as ContentServer;

fn fixture_peer() -> DPeer {
    let mut peer = DPeer::new("peer", "Fixture", "127.0.0.1", Some(9), DeviceType::Phone);
    peer.status = PeerStatus::Paired;
    peer.public_key = "fixture-signing-identity".into();
    peer.token = plain_libs::utils::base64::base64_encode(&[7; 32]);
    peer
}
fn target(peer: &DPeer) -> PeerSubscriptionTarget {
    PeerSubscriptionTarget {
        peer_id: peer.id.clone(),
        public_key: peer.public_key.clone(),
    }
}
#[tokio::test]
async fn rejects_missing_unpaired_chat_only_and_changed_signing_identity_before_connecting() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = ContentServer::open(directory.path()).unwrap();
    let state = server.runtime_state();
    let mut peer = fixture_peer();
    assert!(PeerSubscription::open(state.clone(), target(&peer), Default::default()).is_err());
    peer.status = PeerStatus::Unpaired;
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    assert!(PeerSubscription::open(state.clone(), target(&peer), Default::default()).is_err());
    peer.status = PeerStatus::Paired;
    peers::save(&state.db, &[peer.clone()], SaveMode::Update).unwrap();
    let mut wrong = target(&peer);
    wrong.public_key = "other".into();
    assert!(PeerSubscription::open(state.clone(), wrong, Default::default()).is_err());
    state
        .db
        .with_conn(|connection| connection.execute("UPDATE peers SET token='' WHERE id='peer'", []))
        .unwrap();
    assert!(PeerSubscription::open(state, target(&peer), Default::default()).is_err());
    server.shutdown().await;
}
#[tokio::test]
async fn chat_rekey_preserves_management_but_unpair_revokes_and_close_releases_pending_read() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = ContentServer::open(directory.path()).unwrap();
    let state = server.runtime_state();
    let peer = fixture_peer();
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let session = PeerSubscription::open(state.clone(), target(&peer), Default::default()).unwrap();
    state
        .db
        .with_conn(|connection| {
            connection.execute("UPDATE peers SET key='new-chat-key' WHERE id='peer'", [])
        })
        .unwrap();
    assert!(session.current().is_ok());
    peers::unpair(&state.db, &peer.id).unwrap();
    assert!(
        session
            .send(r#"{"type":"ping"}"#)
            .unwrap_err()
            .contains("revoked")
    );
    session.close();
    assert!(
        tokio::time::timeout(Duration::from_secs(1), session.read())
            .await
            .unwrap()
            .is_err()
    );
    server.shutdown().await;
}

#[tokio::test]
async fn closed_scope_and_bounded_commands_fail_before_opening_or_sending() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = ContentServer::open(directory.path()).unwrap();
    let state = server.runtime_state();
    let peer = fixture_peer();
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let closed = NativeRequestScope::default();
    closed.close();
    assert!(
        PeerSubscription::open_with_pin(state.clone(), target(&peer), closed, Some([1; 32]))
            .is_err()
    );
    let scope = NativeRequestScope::default();
    let session =
        PeerSubscription::open_with_pin(state, target(&peer), scope.clone(), Some([1; 32]))
            .unwrap();
    assert_eq!(session.pin, Some([1; 32]));
    assert_eq!(session.scope_id(), scope.id());
    assert_eq!(
        session.send(&"x".repeat(1024 * 1024)).unwrap_err(),
        "Subscription request too large"
    );
    for _ in 0..64 {
        session.send(r#"{"type":"ping"}"#).unwrap();
    }
    assert_eq!(
        session.send(r#"{"type":"ping"}"#).unwrap_err(),
        "Subscription command backpressure"
    );
    scope.close();
    assert!(session.send(r#"{"type":"ping"}"#).is_err());
    tokio::time::timeout(Duration::from_secs(1), session.wait_closed())
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(1), session.read())
            .await
            .unwrap()
            .is_err()
    );
    server.shutdown().await;
}

#[tokio::test]
async fn failed_peer_connection_marks_the_session_closed_for_handle_cleanup() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = ContentServer::open(directory.path()).unwrap();
    let state = server.runtime_state();
    let mut peer = fixture_peer();
    peer.port = None;
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let session = PeerSubscription::open(state, target(&peer), Default::default()).unwrap();
    tokio::time::timeout(Duration::from_secs(1), session.wait_closed())
        .await
        .unwrap();
    assert!(session.send(r#"{"type":"ping"}"#).is_err());
    server.shutdown().await;
}

#[tokio::test]
async fn request_limit_fills_exactly_one_mib_encrypted_frame() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = ContentServer::open(directory.path()).unwrap();
    let state = server.runtime_state();
    let peer = fixture_peer();
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let session = PeerSubscription::open(state, target(&peer), Default::default()).unwrap();
    let empty = r#"{"type":"ping","payload":""}"#;
    let encoded = empty.replacen(
        "\"\"",
        &format!("\"{}\"", "x".repeat(1024 * 1024 - 48 - empty.len())),
        1,
    );
    session.send(&encoded).unwrap();
    use crate::infrastructure::protocols::websocket::{Role, Stream};
    let mut stream = Stream::new("/graphql", "client", b"init", b"ack", Role::Client).unwrap();
    assert_eq!(
        stream.encode(encoded.as_bytes(), &[7; 32]).unwrap().len(),
        1024 * 1024
    );
    assert_eq!(
        session.send(&(encoded + " ")).unwrap_err(),
        "Subscription request too large"
    );
    session.close();
    server.shutdown().await;
}
