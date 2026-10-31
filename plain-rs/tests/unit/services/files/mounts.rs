use super::*;

#[test]
fn mount_facts_preserve_existing_wire_fields() {
    let facts = MountFacts {
        id: "/volume".into(),
        name: "Volume".into(),
        path: "/volume".into(),
        mount_point: "/volume".into(),
        fs_type: "apfs".into(),
        total_bytes: 100,
        used_bytes: 40,
        free_bytes: 60,
        remote: false,
        alias: String::new(),
        drive_type: "USB_STORAGE".into(),
        disk_id: "/volume".into(),
    };
    let value = serde_json::to_value(facts).unwrap();
    assert_eq!(value["mountPoint"], "/volume");
    assert_eq!(value["totalBytes"], 100);
    assert_eq!(value["usedBytes"], 40);
    assert_eq!(value["freeBytes"], 60);
    assert_eq!(value["driveType"], "USB_STORAGE");
    assert_eq!(value["diskId"], "/volume");
    assert!(is_remote_filesystem("SMBFS"));
    assert!(!is_remote_filesystem("apfs"));
}

#[test]
fn macos_data_volume_is_not_listed_as_a_second_internal_drive() {
    assert!(should_include_mount("/"));
    assert!(should_include_mount("/Volumes/External"));
    assert_eq!(
        should_include_mount("/System/Volumes/Data"),
        !cfg!(target_os = "macos")
    );
}

#[test]
fn remote_filesystems_are_case_insensitive() {
    for name in ["nfs4", "SMBFS", "cifs", "sshfs", "webdav"] {
        assert!(is_remote_filesystem(name), "{name}");
    }
    assert!(!is_remote_filesystem("apfs"));
    assert!(!is_remote_filesystem("ext4"));
}

#[cfg(all(feature = "runtime", target_os = "linux"))]
#[test]
fn linux_mount_conversion_retains_partition_identity_and_nullable_metadata() {
    let facts = linux_facts(plain_rs::platform::linux::mounts::list::Mount {
        id: "part:disk:2".into(),
        name: "Partition".into(),
        partition_num: Some(2),
        label: Some("Data".into()),
        uuid: Some("volume-uuid".into()),
        disk_id: Some("disk".into()),
        total_bytes: 100,
        ..Default::default()
    });
    assert_eq!(facts["id"], "part:disk:2");
    assert_eq!(facts["partitionNum"], 2);
    assert_eq!(facts["label"], "Data");
    assert_eq!(facts["uuid"], "volume-uuid");
    assert_eq!(facts["diskId"], "disk");
    assert!(facts["path"].is_null());
    assert!(facts["mountPoint"].is_null());
    assert!(facts["usedBytes"].is_null());
}
