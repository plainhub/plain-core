use super::*;
fn device(id: &str, method: DiscoveryMethod) -> Device {
    Device {
        id: id.into(),
        name: "Fixture".into(),
        ips: vec!["10.0.0.1".into()],
        port: 2443,
        device_type: crate::features::chat::enums::DeviceType::Phone,
        version: "1".into(),
        platform: "test".into(),
        last_seen_at: crate::infrastructure::db::now_iso(),
        remote_access: None,
        discovery_methods: vec![method],
    }
}
#[test]
fn merges_methods_and_ips_with_monotonic_event_throttle() {
    let devices = Devices::default();
    let t = Instant::now();
    assert!(
        devices
            .seen_at(device("one", DiscoveryMethod::Ble), t)
            .unwrap()
    );
    let mut next = device("one", DiscoveryMethod::Lan);
    next.ips.push("10.0.0.2".into());
    assert!(
        !devices
            .seen_at(next.clone(), t + Duration::from_millis(999))
            .unwrap()
    );
    assert!(devices.seen_at(next, t + Duration::from_secs(1)).unwrap());
    assert!(
        !devices
            .observe_at(device("two", DiscoveryMethod::Lan), t, false)
            .unwrap()
    );
    assert!(
        devices
            .seen_at(device("two", DiscoveryMethod::Lan), t)
            .unwrap()
    );
    devices.forget("two");
    let snapshot = devices.snapshot();
    assert_eq!(snapshot.devices.len(), 1);
    assert_eq!(snapshot.devices[0].ips, vec!["10.0.0.1", "10.0.0.2"]);
    assert_eq!(
        snapshot.devices[0].discovery_methods,
        vec![DiscoveryMethod::Ble, DiscoveryMethod::Lan]
    );
}
#[test]
fn stale_probe_cannot_remove_a_new_sighting_or_new_cached_version() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = crate::infrastructure::db::Db::open(&dir.path().join("test.db")).unwrap();
    let devices = Devices::default();
    devices.scanning(true, false, vec![]).unwrap();
    let t = Instant::now();
    devices
        .seen_at(device("one", DiscoveryMethod::Lan), t)
        .unwrap();
    let old = devices
        .stale_at(&db, t + Duration::from_secs(61))
        .unwrap()
        .pop()
        .unwrap();
    devices
        .seen_at(
            device("one", DiscoveryMethod::Ble),
            t + Duration::from_secs(62),
        )
        .unwrap();
    assert!(!devices.verified(&db, &old, false).unwrap());
    assert_eq!(devices.snapshot().devices.len(), 1);
    let stale = devices
        .stale_at(&db, t + Duration::from_secs(124))
        .unwrap()
        .pop()
        .unwrap();
    let mut newer = stale.device.cache();
    newer.last_seen_at = "2099-01-01T00:00:00.000Z".into();
    crate::infrastructure::db::chat_store::nearby::save(&db, &newer).unwrap();
    assert!(!devices.verified(&db, &stale, false).unwrap());
    assert_eq!(
        crate::infrastructure::db::chat_store::nearby::all(&db)
            .unwrap()
            .len(),
        1
    );
    crate::infrastructure::db::chat_store::nearby::delete(&db, "one").unwrap();
    let stale = devices
        .stale_at(&db, t + Duration::from_secs(124))
        .unwrap()
        .pop()
        .unwrap();
    assert!(devices.verified(&db, &stale, false).unwrap());
    assert!(devices.snapshot().devices.is_empty());
}
#[test]
fn stopped_scan_cannot_apply_pending_verdict_and_registry_is_bounded() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = crate::infrastructure::db::Db::open(&dir.path().join("test.db")).unwrap();
    let devices = Devices::default();
    devices.scanning(true, false, vec![]).unwrap();
    let t = Instant::now();
    for i in 0..513 {
        devices
            .seen_at(
                device(&i.to_string(), DiscoveryMethod::Ble),
                t + Duration::from_millis(i),
            )
            .unwrap();
    }
    assert_eq!(devices.snapshot().devices.len(), 512);
    assert!(!devices.snapshot().devices.iter().any(|d| d.id == "0"));
    let stale = devices
        .stale_at(&db, t + Duration::from_secs(62))
        .unwrap()
        .pop()
        .unwrap();
    devices.scanning(false, false, vec![]).unwrap();
    assert!(!devices.verified(&db, &stale, false).unwrap());
    devices.scanning(true, false, vec![]).unwrap();
    assert!(!devices.verified(&db, &stale, false).unwrap());
    devices.scanning(false, false, vec![]).unwrap();
    assert!(
        devices
            .stale_at(&db, t + Duration::from_secs(63))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn merged_ble_timestamp_still_allows_checked_lan_cache_cleanup() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = crate::infrastructure::db::Db::open(&dir.path().join("test.db")).unwrap();
    let devices = Devices::default();
    devices.scanning(true, false, vec![]).unwrap();
    let t = Instant::now();
    let lan = device("one", DiscoveryMethod::Lan);
    crate::infrastructure::db::chat_store::nearby::save(&db, &lan.cache()).unwrap();
    devices.seen_at(lan, t).unwrap();
    let mut ble = device("one", DiscoveryMethod::Ble);
    ble.last_seen_at = "2099-01-01T00:00:00.000Z".into();
    devices.seen_at(ble, t + Duration::from_secs(1)).unwrap();
    let stale = devices
        .stale_at(&db, t + Duration::from_secs(62))
        .unwrap()
        .pop()
        .unwrap();
    assert!(devices.verified(&db, &stale, false).unwrap());
    assert!(
        crate::infrastructure::db::chat_store::nearby::all(&db)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn device_types_preserve_wire_names_and_cache_values() {
    use crate::features::chat::enums::DeviceType;
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = crate::infrastructure::db::Db::open(&directory.path().join("nearby.db")).unwrap();
    for kind in [
        DeviceType::Phone,
        DeviceType::Tablet,
        DeviceType::Computer,
        DeviceType::Tv,
        DeviceType::Nas,
        DeviceType::Other,
    ] {
        let mut value = device("typed", DiscoveryMethod::Lan);
        value.device_type = kind;
        let wire = serde_json::to_value(&value).unwrap();
        assert_eq!(wire["deviceType"], kind.as_str());
        assert_eq!(
            serde_json::from_value::<Device>(wire).unwrap().device_type,
            kind
        );
        crate::infrastructure::db::chat_store::nearby::save(&db, &value.cache()).unwrap();
        assert_eq!(
            crate::infrastructure::db::chat_store::nearby::all(&db).unwrap()[0].device_type,
            kind
        );
        assert_eq!(db.get_cached_nearby_devices().unwrap()[0].device_type, kind);
    }
    let mut wire = serde_json::to_value(device("invalid", DiscoveryMethod::Lan)).unwrap();
    wire["deviceType"] = serde_json::json!("FUTURE_DEVICE");
    assert!(serde_json::from_value::<Device>(wire).is_err());
}

#[test]
fn discovery_methods_keep_supported_wire_values_and_reject_unknown_values() {
    for (method, wire, lan) in [
        (DiscoveryMethod::Lan, "LAN", true),
        (DiscoveryMethod::DnsSd, "DNS_SD", true),
        (DiscoveryMethod::Ble, "BLE", false),
        (DiscoveryMethod::Qr, "QR", false),
    ] {
        assert_eq!(method.is_lan(), lan);
        let source = device("typed-method", method);
        source.validate().unwrap();
        let value = serde_json::to_value(source).unwrap();
        assert_eq!(value["discoveryMethods"], serde_json::json!([wire]));
        assert_eq!(
            serde_json::from_value::<Device>(value)
                .unwrap()
                .discovery_methods,
            [method]
        );
    }
    let mut value = serde_json::to_value(device("invalid-method", DiscoveryMethod::Lan)).unwrap();
    value["discoveryMethods"] = serde_json::json!(["FUTURE_METHOD"]);
    assert!(serde_json::from_value::<Device>(value).is_err());
}
