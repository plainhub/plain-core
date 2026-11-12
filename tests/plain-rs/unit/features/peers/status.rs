use super::*;
use crate::features::chat::enums::{DeviceType, PeerStatus};
use crate::infrastructure::db::chat_store::SaveMode;
use crate::infrastructure::db::chat_store::peers;

#[test]
fn status_identity_proof_is_bound_to_client_and_connection() {
    let (keypair, public_key) = plain_libs::crypto::ed25519_generate();
    let mut peer = DPeer::new("peer", "fixture", "127.0.0.1", None, DeviceType::Phone);
    peer.public_key = base64_encode(&public_key);
    let signature = proof(&keypair, "peer", b"init", b"ack").unwrap();
    assert!(verify(&peer, b"init", b"ack", signature.as_bytes()));
    assert!(!verify(&peer, b"other-init", b"ack", signature.as_bytes()));
    assert!(!verify(&peer, b"init", b"other-ack", signature.as_bytes()));
    peer.id = "other".into();
    assert!(!verify(&peer, b"init", b"ack", signature.as_bytes()));
    assert!(proof(&[], "peer", b"init", b"ack").is_err());
}

#[test]
fn peer_list_snapshot_uses_supplied_rows_without_any_database_access() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let mut peer = DPeer::new(
        "snapshot",
        "Fixture",
        "127.0.0.1",
        Some(8443),
        crate::features::chat::enums::DeviceType::Phone,
    );
    peer.key = plain_libs::utils::base64::base64_encode(&[1; 32]);
    peer.public_key = plain_libs::utils::base64::base64_encode(&[2; 32]);
    peer.status = crate::features::chat::enums::PeerStatus::Paired;
    crate::infrastructure::db::chat_store::peers::save(
        &db,
        &[peer.clone()],
        crate::infrastructure::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    let connections = Connections::default();
    connections.hint(&db, &peer.id).unwrap();
    connections.admit(&db, peer.clone()).unwrap();
    db.with_conn(|_| {
        assert_eq!(
            connections.snapshot_peers(&[peer.clone()])["online"],
            json!(["snapshot"])
        );
        let mut rotated = peer.clone();
        rotated.key = plain_libs::utils::base64::base64_encode(&[3; 32]);
        assert_eq!(connections.snapshot_peers(&[rotated])["online"], json!([]));
        assert_eq!(connections.snapshot_peers(&[])["online"], json!([]));
    });
}
