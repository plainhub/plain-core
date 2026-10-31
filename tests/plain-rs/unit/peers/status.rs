use super::*;
use crate::chat::enums::{DeviceType, PeerStatus};
use crate::db::chat_store::{SaveMode, peers};

#[test]
fn status_proof_is_bound_to_connection_and_rejects_old_signed_content() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let key = [9; 32];
    let (keypair, public_key) = crate::crypto::ed25519_generate();
    let mut peer = DPeer::new("peer", "fixture", "127.0.0.1", None, DeviceType::Phone);
    peer.status = PeerStatus::Paired;
    peer.key = crate::utils::base64::base64_encode(&key);
    peer.public_key = crate::utils::base64::base64_encode(&public_key);
    peers::save(&db, &[peer], SaveMode::Insert).unwrap();
    let first = uuid::Uuid::new_v4().to_string();
    let second = uuid::Uuid::new_v4().to_string();
    let body = handshake(&key, &keypair, "peer", &first).unwrap();
    let connections = Connections::default();
    assert!(connections.open(&db, "peer", &second, &body).is_err());
    assert_eq!(connections.snapshot(&db)["online"], json!([]));
    let plain = crate::xchacha_decrypt_raw(&key, &body).unwrap();
    let reencrypted = crate::xchacha_encrypt_raw(&key, &plain).unwrap();
    assert!(
        connections
            .open(&db, "peer", &second, &reencrypted)
            .is_err()
    );
    let timestamp = chrono::Utc::now().timestamp_millis();
    let signature = crate::ed25519_sign(
        &keypair,
        peer_auth::status_signature_payload(timestamp, "peer").as_bytes(),
    );
    let old = crate::xchacha_encrypt_raw(&key, format!("{signature}|{timestamp}|peer").as_bytes())
        .unwrap();
    assert!(connections.open(&db, "peer", &first, &old).is_err());
    assert!(connections.open(&db, "peer", &first, &body).is_ok());
    let fresh = handshake(&key, &keypair, "peer", &second).unwrap();
    assert!(connections.open(&db, "peer", &second, &fresh).is_ok());
    assert_eq!(connections.snapshot(&db)["online"], json!(["peer"]));
    assert!(handshake(&key, &keypair, "peer", "").is_err());
    assert!(handshake(&key, &keypair, "", &first).is_err());
}

#[test]
fn peer_list_snapshot_uses_supplied_rows_without_any_database_access() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let mut peer = DPeer::new(
        "snapshot",
        "Fixture",
        "127.0.0.1",
        Some(8443),
        crate::chat::enums::DeviceType::Phone,
    );
    peer.key = crate::base64_encode(&[1; 32]);
    peer.public_key = crate::base64_encode(&[2; 32]);
    peer.status = crate::chat::enums::PeerStatus::Paired;
    crate::db::chat_store::peers::save(
        &db,
        &[peer.clone()],
        crate::db::chat_store::SaveMode::Insert,
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
        rotated.key = crate::base64_encode(&[3; 32]);
        assert_eq!(connections.snapshot_peers(&[rotated])["online"], json!([]));
        assert_eq!(connections.snapshot_peers(&[])["online"], json!([]));
    });
}
