use super::*;

fn mount(source: &str) -> Mount {
    Mount {
        id: "42".into(),
        device: device_number(&std::fs::metadata("/dev/null").unwrap()),
        root: "/".into(),
        path: "/media/test-drive".into(),
        fs_type: "ext4".into(),
        source: source.into(),
    }
}

#[test]
fn disconnected_block_mount_never_becomes_a_new_session_volume() {
    let root = crate::temporary::tempdir().unwrap();
    let uuids = root.path().join("by-uuid");
    let boot = root.path().join("boot_id");
    std::fs::create_dir(&uuids).unwrap();
    std::fs::write(&boot, "generated-boot").unwrap();
    let uuid = uuids.join("generated-filesystem-uuid");
    std::os::unix::fs::symlink("/dev/null", &uuid).unwrap();
    let before = volume(mount("/dev/test-drive"), &uuids, &boot).unwrap();
    assert_eq!(before.identity, "UUID:GENERATED-FILESYSTEM-UUID");
    std::fs::remove_file(&uuid).unwrap();
    let error = volume(mount("/dev/test-drive"), &uuids, &boot).unwrap_err();
    assert!(error.is::<Unavailable>());
    std::os::unix::fs::symlink("/dev/null", &uuid).unwrap();
    assert_eq!(
        volume(mount("/dev/test-drive"), &uuids, &boot).unwrap(),
        before
    );
}

#[test]
fn block_mount_without_a_uuid_directory_is_unavailable_without_reading_boot_id() {
    let root = crate::temporary::tempdir().unwrap();
    let error = volume(
        mount("/dev/test-drive"),
        &root.path().join("missing-by-uuid"),
        &root.path().join("missing-boot-id"),
    )
    .unwrap_err();
    assert!(error.is::<Unavailable>());
}

#[test]
fn uuid_lookup_errors_are_not_downgraded_to_unavailability() {
    let root = crate::temporary::tempdir().unwrap();
    let file = root.path().join("not-a-directory");
    std::fs::write(&file, []).unwrap();
    let error = volume(mount("/dev/test-drive"), &file, &file).unwrap_err();
    assert!(!error.is::<Unavailable>());
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::NotADirectory
    );
}

#[test]
fn replacing_a_disk_at_the_same_mount_does_not_reuse_its_identity() {
    let root = crate::temporary::tempdir().unwrap();
    let uuids = root.path().join("by-uuid");
    std::fs::create_dir(&uuids).unwrap();
    let old_uuid = uuids.join("first-generated-uuid");
    std::os::unix::fs::symlink("/dev/null", &old_uuid).unwrap();
    let first = volume(mount("/dev/test-drive"), &uuids, root.path()).unwrap();
    std::fs::remove_file(old_uuid).unwrap();
    std::os::unix::fs::symlink("/dev/null", uuids.join("replacement-generated-uuid")).unwrap();
    let replacement = volume(mount("/dev/test-drive"), &uuids, root.path()).unwrap();
    assert_eq!(first.path, replacement.path);
    assert_ne!(first.identity, replacement.identity);
}

#[test]
fn anonymous_mount_device_uses_the_source_device_uuid_and_keeps_subvolume_root() {
    let root = crate::temporary::tempdir().unwrap();
    let uuids = root.path().join("by-uuid");
    std::fs::create_dir(&uuids).unwrap();
    std::os::unix::fs::symlink("/dev/null", uuids.join("generated-btrfs-uuid")).unwrap();
    let mut entry = mount("/dev/null");
    entry.device = "0:42".into();
    entry.fs_type = "btrfs".into();
    entry.root = "/generated-subvolume".into();
    let result = volume(entry, &uuids, &root.path().join("missing-boot-id")).unwrap();
    assert_eq!(
        result.identity,
        "UUID:GENERATED-BTRFS-UUID:ROOT:/generated-subvolume"
    );
}

#[test]
fn non_block_mounts_keep_session_identity_without_udev() {
    let root = crate::temporary::tempdir().unwrap();
    let boot = root.path().join("boot_id");
    std::fs::write(&boot, "generated-boot\n").unwrap();
    let mut entry = mount("tmpfs");
    entry.fs_type = "tmpfs".into();
    let result = volume(entry, &root.path().join("absent-by-uuid"), &boot).unwrap();
    assert_eq!(result.identity, "SESSION:generated-boot:42");
}
