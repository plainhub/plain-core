use super::*;
use crate::infrastructure::db::{DNearbyDeviceCache, chat_store::nearby};

#[test]
fn nearby_cache_survives_reopen_and_preserves_newer_sightings() {
    let path = std::env::temp_dir().join(format!(
        "plain-rs-chat-nearby-cache-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    let device = DNearbyDeviceCache {
        id: "phone-1".into(),
        name: "Phone".into(),
        ips: vec!["203.0.113.2".into()],
        port: 8443,
        remote_access: Some(true),
        signature_public_key: plain_libs::utils::base64::base64_encode(&[31; 32]),
        device_type: crate::features::chat::enums::DeviceType::Phone,
        version: "1".into(),
        platform: "android".into(),
        last_seen_at: "1970-01-01T00:00:00.100Z".into(),
    };
    let db = Db::open(&path).unwrap();
    nearby::save(&db, &device).unwrap();
    drop(db);
    let db = Db::open(&path).unwrap();
    assert_eq!(
        nearby::all(&db).unwrap(),
        vec![device.clone()]
    );
    let mut updated = device.clone();
    updated.ips = vec!["203.0.113.3".into()];
    let updated = DNearbyDeviceCache {
        last_seen_at: "1970-01-01T00:00:00.150Z".into(),
        ..updated
    };
    nearby::save(&db, &updated).unwrap();
    assert!(
        !db.delete_cached_nearby_device_if_last_seen_matches(&device.id, &device.last_seen_at)
            .unwrap()
    );
    let latest = nearby::all(&db).unwrap().remove(0);
    assert_eq!(latest, updated);
    assert!(
        db.delete_cached_nearby_device_if_last_seen_matches(&latest.id, &latest.last_seen_at)
            .unwrap()
    );
    assert!(nearby::all(&db).unwrap().is_empty());
    drop(db);
    let _ = std::fs::remove_file(path);
}
