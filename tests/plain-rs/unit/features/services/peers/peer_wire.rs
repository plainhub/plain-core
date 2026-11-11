use super::*;
use crate::features::chat::enums::DeviceType;
use crate::infrastructure::db::DChannel;
use crate::infrastructure::db::chat_store::SaveMode;
use plain_libs::crypto::ed25519_generate;
use plain_libs::utils::base64::base64_encode;
#[test]
fn signing_current_peer_and_channel_keys_and_authenticated_crypto_roundtrip() {
    let dir = std::env::temp_dir().join(format!(
        "plain-peer-wire-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let prefs = Prefs::load(&dir.join("prefs.json")).unwrap();
    let (key, public) = ed25519_generate();
    prefs
        .set("signature_key_pair", base64_encode(&key))
        .unwrap();
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let mut peer = DPeer::new("peer", "peer", "127.0.0.1", Some(1), DeviceType::Phone);
    peer.key = base64_encode(&[7; 32]);
    peers::save(&db, &[peer.clone()], SaveMode::Insert).unwrap();
    let content = r#"{"type":"TEXT","value":{"text":"中文 %_ | quoted"}}"#;
    let first = prepare(
        &db,
        &prefs,
        "peer",
        Operation::Chat {
            content: content.into(),
            channel_id: String::new(),
        },
    )
    .unwrap();
    let split: Vec<_> = first.body.splitn(4, '|').collect();
    assert!(plain_libs::crypto::ed25519_verify(
        &base64_encode(&public),
        crate::infrastructure::transport::auth::signature_payload(
            split[1].parse().unwrap(),
            "",
            split[2],
            split[3]
        )
        .as_bytes(),
        split[0]
    ));
    let gql: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(split[3]).unwrap();
    assert_eq!(gql["variables"]["content"], content);
    assert_eq!(first.key, peer.key);
    let decoded_key = plain_libs::utils::base64::base64_decode(&first.key);
    let encrypted = crate::infrastructure::transport::http_request::encrypt_raw(
        &decoded_key,
        "POST",
        "/peer_graphql",
        first.body.as_bytes(),
    )
    .unwrap();
    assert_eq!(
        crate::infrastructure::transport::http_request::decrypt_raw(
            &decoded_key,
            "POST",
            "/peer_graphql",
            &encrypted
        )
        .unwrap(),
        first.body.as_bytes()
    );
    assert!(
        crate::infrastructure::transport::http_request::decrypt_raw(
            &[8; 32],
            "POST",
            "/peer_graphql",
            &encrypted
        )
        .is_none()
    );
    assert!(
        crate::infrastructure::transport::http_request::decrypt_raw(
            &decoded_key,
            "POST",
            "/peer_graphql",
            b"plaintext"
        )
        .is_none()
    );
    let mut channel = DChannel::new("group", "owner");
    channel.key = base64_encode(&[9; 32]);
    channels::save(&db, &[channel.clone()], SaveMode::Insert).unwrap();
    let group = prepare(
        &db,
        &prefs,
        "peer",
        Operation::Chat {
            content: content.into(),
            channel_id: channel.id.clone(),
        },
    )
    .unwrap();
    assert_eq!(group.key, channel.key);
    assert_eq!(group.channel_id, channel.id);
    peer.key = base64_encode(&[4; 32]);
    peers::save(&db, &[peer.clone()], SaveMode::Update).unwrap();
    let aware = prepare(&db, &prefs, "peer", Operation::StartAware).unwrap();
    assert_eq!(aware.key, peer.key);
    assert_eq!(aware.channel_id, "");
    assert!(prepare(&db, &prefs, "missing", Operation::StartAware).is_err());
    assert!(
        prepare(
            &db,
            &prefs,
            "peer",
            Operation::Chat {
                content: content.into(),
                channel_id: "missing".into()
            }
        )
        .is_err()
    );
    assert!(
        crate::infrastructure::transport::http_request::encrypt_raw(
            b"bad",
            "POST",
            "/peer_graphql",
            b"text"
        )
        .is_none()
    );
    assert!(
        crate::infrastructure::transport::http_request::decrypt_raw(
            b"bad",
            "POST",
            "/peer_graphql",
            &encrypted
        )
        .is_none()
    );
    std::fs::remove_dir_all(dir).unwrap();
}
