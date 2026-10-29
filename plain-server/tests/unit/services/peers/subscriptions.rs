use super::*;
use crate::assembly::server::ContentServer;
use plain_rs::{
    chat::enums::{DeviceType, PeerStatus},
    db::chat_store::SaveMode,
};

fn fixture_peer() -> DPeer {
    let mut peer = DPeer::new("peer", "Fixture", "127.0.0.1", Some(9), DeviceType::Phone);
    peer.status = PeerStatus::Paired;
    peer.public_key = "fixture-signing-identity".into();
    peer.token = plain_rs::utils::base64::base64_encode(&[7; 32]);
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
    let directory = tempfile::tempdir().unwrap();
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
    let directory = tempfile::tempdir().unwrap();
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
