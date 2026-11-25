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
        remote_access: true,
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


#[test]
fn nearby_cache_requires_a_boolean_remote_access_value() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&dir.path().join("nearby.db")).unwrap();
    db.with_conn(|c| {
        let sql = "INSERT INTO nearby_devices (id,name,ips,port,remote_access,device_type,version,platform,last_seen_at) VALUES(?1,'Fixture','[]',8443,?2,'PHONE','','','2026-11-25T00:00:00Z')";
        for invalid in [None, Some(2_i32)] {
            assert!(c.execute(sql, plain_libs::rusqlite::params!["invalid", invalid]).is_err());
        }
        for (id, remote_access) in [("disabled", false), ("enabled", true)] {
            assert_eq!(c.execute(sql, plain_libs::rusqlite::params![id, remote_access]).unwrap(), 1);
        }
        Ok::<_, plain_libs::rusqlite::Error>(())
    }).unwrap();
    let rows = nearby::all(&db).unwrap();
    assert_eq!(rows.iter().map(|row| (row.id.as_str(), row.remote_access)).collect::<Vec<_>>(),
        vec![("disabled", false), ("enabled", true)]);
}
