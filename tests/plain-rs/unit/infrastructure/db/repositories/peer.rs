use super::*;
use crate::features::chat::enums::DeviceType;
use crate::features::chat::enums::PeerStatus;
use crate::infrastructure::db::chat_tests::unique_tmp_dir;

fn seed_peer(db: &Db, id: &str, status: PeerStatus, key: &str) {
    let mut peer = DPeer::new(id, id, "198.51.100.1", Some(12345), DeviceType::Phone);
    peer.status = status;
    peer.key = key.to_string();
    db.upsert_peer(&peer);
}

#[test]
fn update_peer_status_flips_status_and_bumps_updated_at() {
    let db = Db::open(&unique_tmp_dir("status-flip").join("plain.db")).expect("open db");
    seed_peer(&db, "p1", PeerStatus::Paired, &plain_libs::utils::base64::base64_encode(&[1; 32]));
    let before = db.get_peer_by_id("p1").expect("peer exists");
    assert_eq!(before.status, PeerStatus::Paired);

    db.update_peer_status("p1", PeerStatus::Unpaired);

    let after = db.get_peer_by_id("p1").expect("peer still exists");
    assert_eq!(after.status, PeerStatus::Unpaired);
    assert_eq!(after.key, plain_libs::utils::base64::base64_encode(&[1; 32]));
    assert!(after.updated_at >= before.updated_at);
}

#[test]
fn update_peer_status_and_key_clears_key_for_channel_demotion() {
    let db = Db::open(&unique_tmp_dir("key-clear").join("plain.db")).expect("open db");
    seed_peer(&db, "p1", PeerStatus::Paired, &plain_libs::utils::base64::base64_encode(&[2; 32]));

    db.update_peer_status_and_key("p1", PeerStatus::Channel, "");

    let after = db.get_peer_by_id("p1").expect("peer demoted, not deleted");
    assert_eq!(after.status, PeerStatus::Channel);
    assert_eq!(after.key, "");
}

#[test]
fn delete_peer_removes_row() {
    let db = Db::open(&unique_tmp_dir("delete").join("plain.db")).expect("open db");
    seed_peer(&db, "p1", PeerStatus::Paired, &plain_libs::utils::base64::base64_encode(&[1; 32]));
    assert!(db.get_peer_by_id("p1").is_some());

    db.delete_peer("p1");
    assert!(db.get_peer_by_id("p1").is_none());
}

fn seed_logged_in_peer(db: &Db, name: &str) {
    let encode = plain_libs::utils::base64::base64_encode;
    let mut peer = DPeer::new("p1", name, "203.0.113.10", Some(8443), DeviceType::Phone);
    peer.token = encode(&[4; 32]);
    peer.public_key = encode(&[6; 32]);
    db.upsert_peer(&peer);
}

#[test]
fn logout_peer_clears_token_and_drops_from_login_list() {
    let db = Db::open(&unique_tmp_dir("logout").join("plain.db")).expect("open db");
    seed_logged_in_peer(&db, "Fixture");
    assert_eq!(db.get_login_peers().len(), 1);

    db.logout_peer("p1").unwrap();

    let peer = db
        .get_peer_by_id("p1")
        .expect("row kept, only token cleared");
    assert_eq!(peer.token, "");
    assert!(db.get_login_peers().is_empty());
}

#[test]
fn update_peer_name_renames_only() {
    let db = Db::open(&unique_tmp_dir("rename").join("plain.db")).expect("open db");
    seed_logged_in_peer(&db, "old");

    db.with_conn(|connection| {
        crate::infrastructure::db::chat_store::peers::update_name(connection, "p1", "new-name")
    })
    .unwrap();

    let peer = db.get_peer_by_id("p1").expect("peer exists");
    assert_eq!(peer.name, "new-name");
    assert_eq!(peer.token, plain_libs::utils::base64::base64_encode(&[4; 32]));
    assert_eq!(peer.ip, "203.0.113.10");
}

#[test]
fn peer_url_helpers_use_single_ip_and_port() {
    let mut peer = DPeer::new("p1", "p1", "203.0.113.7", Some(8443), DeviceType::Nas);
    assert_eq!(peer.base_url().unwrap(), "https://203.0.113.7:8443");
    assert_eq!(
        peer.peer_graphql_url().unwrap(),
        "https://203.0.113.7:8443/peer_graphql"
    );
    assert_eq!(
        peer.file_url("abc").unwrap(),
        "https://203.0.113.7:8443/fs?id=abc&action=PREVIEW"
    );
    // Default HTTPS port is omitted.
    peer.port = Some(443);
    peer.ip = "203.0.113.7".to_string();
    assert_eq!(peer.base_url().unwrap(), "https://203.0.113.7");
    peer.ip = "::1".into();
    assert_eq!(peer.base_url().unwrap(), "https://[::1]");
    peer.ip = "device.example".into();
    assert_eq!(peer.base_url().unwrap(), "https://device.example");
}

#[test]
fn unknown_port_is_null_and_has_no_connection_urls() {
    let db = crate::infrastructure::db::Db::open(std::path::Path::new(":memory:")).unwrap();
    let peer = DPeer::new(
        "unknown",
        "Unknown",
        "198.51.100.1",
        None,
        DeviceType::Phone,
    );
    crate::infrastructure::db::chat_store::peers::save(
        &db,
        &[peer],
        crate::infrastructure::db::chat_store::SaveMode::Upsert,
    )
    .unwrap();
    let stored = db.get_peer_by_id("unknown").unwrap();
    assert_eq!(stored.port, None);
    assert_eq!(stored.base_url(), None);
    assert_eq!(stored.peer_graphql_url(), None);
    assert_eq!(stored.file_url("file"), None);
    assert!(crate::infrastructure::transport::contract::peer_graphql_urls(&stored).is_empty());
    assert!(
        db.with_conn(|connection| connection.query_row(
            "SELECT port IS NULL FROM peers WHERE id='unknown'",
            [],
            |row| row.get::<_, bool>(0)
        ))
        .unwrap()
    );
    assert!(
        db.with_conn(
            |connection| connection.execute("UPDATE peers SET port=0 WHERE id='unknown'", [])
        )
        .is_err()
    );
    assert!(
        db.with_conn(
            |connection| connection.execute("UPDATE peers SET port=65536 WHERE id='unknown'", [])
        )
        .is_err()
    );
}

#[test]
fn login_credentials_are_raw_blobs_and_logout_clears_the_token() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let encode = plain_libs::utils::base64::base64_encode;
    use crate::features::peers::pairing::peer_store::{self, Facts};
    peer_store::save_confirmed(&db, Facts {
        id: "test-peer".into(), name: "Test peer".into(), ip: String::new(),
        port: 8080, device_type: DeviceType::Phone, key: encode(&[0x80; 32]),
        public_key: encode(&[0xa4; 32]), encryption_public_key: encode(&[0xb5; 32]),
    }, None, Some(&encode(&[0xef; 32]))).unwrap();

    db.with_conn(|conn| -> plain_libs::rusqlite::Result<()> {
        let mut statement = conn.prepare("PRAGMA table_info(peers)")?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        assert!(!columns.iter().any(|name| matches!(name.as_str(), "key" | "token" | "public_key")));
        let encryption_key = conn.query_row(
            "SELECT encryption_public_key FROM peers WHERE id='test-peer'", [],
            |row| row.get::<_, Option<Vec<u8>>>(0),
        )?;
        assert_eq!(encryption_key, Some(vec![0xb5; 32]));
        for invalid in ["zeroblob(31)", "'text key'"] {
            assert!(conn.execute(&format!("UPDATE peers SET encryption_public_key={invalid} WHERE id='test-peer'"), []).is_err());
        }
        Ok(())
    }).unwrap();

    let credentials = || {
        db.with_conn(|c| c.query_row(
            "SELECT chat_token, signature_public_key, manage_token FROM peers WHERE id='test-peer'",
            [],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?, row.get::<_, Option<Vec<u8>>>(2)?)),
        )).unwrap()
    };
    assert_eq!(credentials(), (vec![0x80; 32], vec![0xa4; 32], Some(vec![0xef; 32])));
    assert_eq!(db.get_login_peers()[0].token, encode(&[0xef; 32]));
    assert!(db.with_conn(|c| c.execute("UPDATE peers SET manage_token='not a blob' WHERE id='test-peer'", [])).is_err());

    db.logout_peer("test-peer").unwrap();
    assert_eq!(credentials(), (vec![0x80; 32], vec![0xa4; 32], None));
    assert!(db.get_login_peers().is_empty());
}
