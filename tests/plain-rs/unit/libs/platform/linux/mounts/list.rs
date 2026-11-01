use super::*;

#[test]
fn block_usage_preserves_available_blocks_and_rejects_unrepresentable_sizes() {
    assert_eq!(block_usage(100, 30, 4096), Some((409600, 286720, 122880)));
    assert_eq!(block_usage(0, 0, 4096), Some((0, 0, 0)));
    assert_eq!(block_usage(10, 20, 1), Some((10, -10, 20)));
    assert_eq!(block_usage(u64::MAX, 0, 1), None);
    assert_eq!(block_usage(0, u64::MAX, 1), None);
    assert_eq!(block_usage(i64::MAX as u64, 0, 2), None);
    assert_eq!(block_usage(0, i64::MAX as u64, 2), None);
}

#[cfg(target_os = "linux")]
#[test]
fn statfs_usage_reads_native_statistics_and_reports_invalid_paths() {
    let directory = tempfile::tempdir().unwrap();
    let (total, used, free) = statfs_usage(directory.path().to_str().unwrap()).unwrap();
    assert!(total > 0);
    assert_eq!(total, used + free);
    assert_eq!(statfs_usage("/fixture\0suffix"), None);
    assert_eq!(
        statfs_usage(directory.path().join("missing").to_str().unwrap()),
        None
    );
}

#[test]
fn classify_keeps_block_mount() {
    let ml = classify_mount_line("/dev/sda1 / ext4 rw,relatime 0 0").unwrap();
    assert_eq!(ml.src, "/dev/sda1");
    assert_eq!(ml.target, "/");
    assert!(ml.is_block);
    assert!(!ml.is_remote);
}

#[test]
fn classify_skips_pseudo_filesystems() {
    for fstype in [
        "proc", "sysfs", "tmpfs", "devtmpfs", "cgroup2", "overlay", "squashfs",
    ] {
        assert!(
            classify_mount_line(&format!("/dev/anything /mnt/x {fstype} rw 0 0")).is_none(),
            "{fstype} should be skipped"
        );
    }
}

#[test]
fn classify_skips_system_mountpoints() {
    for target in ["/boot", "/boot/efi", "/efi", "/boot/firmware"] {
        assert!(
            classify_mount_line(&format!("/dev/sda1 {target} vfat rw 0 0")).is_none(),
            "{target} should be skipped"
        );
    }
}

#[test]
fn classify_skips_bind_mounts() {
    assert!(classify_mount_line("/dev/sda1 /mnt/bind ext4 rw,bind 0 0").is_none());
    assert!(classify_mount_line("/dev/sda1 /mnt/bind ext4 rw,relatime,bind 0 0").is_none());
    assert!(classify_mount_line("/dev/sda1 /mnt/x ext4 rw,relatime 0 0").is_some());
}

#[test]
fn classify_skips_non_storage_sources() {
    assert!(classify_mount_line("ahlsec /mnt/x ahlsec rw 0 0").is_none());
}

#[test]
fn classify_skips_zram_and_loop() {
    assert!(classify_mount_line("/dev/zram0 /swap swap rw 0 0").is_none());
    assert!(classify_mount_line("/dev/loop0 /mnt/x squashfs ro 0 0").is_none());
}

#[test]
fn classify_detects_remote_mounts() {
    let ml = classify_mount_line("server:/export /mnt/nfs nfs4 rw 0 0").unwrap();
    assert!(ml.is_remote);
    assert!(!ml.is_block);
    let ml = classify_mount_line("//192.0.2.5/share /mnt/smb cifs rw 0 0").unwrap();
    assert!(ml.is_remote);
    assert!(!ml.is_block);
    let ml = classify_mount_line("user@host:/path /mnt/ssh sshfs rw 0 0").unwrap();
    assert!(ml.is_remote);
}

#[test]
fn classify_short_lines_dropped() {
    assert!(classify_mount_line("").is_none());
    assert!(classify_mount_line("/dev/sda1 /mnt").is_none());
}

#[test]
fn non_empty_helper() {
    assert_eq!(non_empty("  "), None);
    assert_eq!(non_empty(""), None);
    assert_eq!(non_empty("photos"), Some("photos".to_string()));
}

fn usb_disk(mountpoint: Option<&str>) -> LsblkDevice {
    serde_json::from_value(serde_json::json!({
        "name":"sdfixture", "path":"/dev/sdfixture", "type":"disk",
        "children":[{"name":"sdfixture1", "path":"/dev/sdfixture1", "type":"part",
            "fstype":"exfat", "uuid":"usb-volume", "label":"USB Data", "partn":1,
            "size":64000000000_i64, "mountpoint":mountpoint}]
    }))
    .unwrap()
}

#[test]
fn detected_usb_partition_is_listed_before_mounting() {
    let mounts = partition_mounts(&[usb_disk(None)]);
    assert_eq!(mounts.len(), 1);
    let mount = &mounts[0];
    assert_eq!(mount.id, "part:usb-volume");
    assert_eq!(mount.path.as_deref(), Some("/dev/sdfixture1"));
    assert_eq!(mount.label.as_deref(), Some("USB Data"));
    assert_eq!(mount.fs_type.as_deref(), Some("exfat"));
    assert_eq!(mount.partition_num, Some(1));
    assert_eq!(mount.total_bytes, 64000000000);
    assert!(mount.mount_point.is_none());
    assert!(mount.used_bytes.is_none());
    assert!(mount.free_bytes.is_none());
}

#[test]
fn usb_mount_changes_preserve_partition_identity_and_absent_devices_disappear() {
    let unmounted = partition_mounts(&[usb_disk(None)]);
    let mounted = partition_mounts(&[usb_disk(Some("/mnt/usb1"))]);
    assert_eq!(mounted[0].id, unmounted[0].id);
    assert_eq!(mounted[0].mount_point.as_deref(), Some("/mnt/usb1"));
    assert!(partition_mounts(&[]).is_empty());
    let reconnected = partition_mounts(&[usb_disk(None)]);
    assert_eq!(reconnected[0].id, mounted[0].id);
    assert!(reconnected[0].mount_point.is_none());
}

#[test]
fn usb_partition_without_uuid_retains_device_path_and_nullable_metadata() {
    let mut disk = usb_disk(None);
    let partition = &mut disk.children[0];
    partition.uuid.clear();
    partition.label.clear();
    partition.path.clear();
    partition.partn = None;
    let mounts = partition_mounts(&[disk]);
    assert_eq!(mounts[0].id, "part:/dev/sdfixture1");
    assert_eq!(mounts[0].path.as_deref(), Some("/dev/sdfixture1"));
    assert!(mounts[0].uuid.is_none());
    assert!(mounts[0].label.is_none());
    assert!(mounts[0].partition_num.is_none());
}

#[test]
fn system_partitions_and_virtual_disks_are_not_presented_as_usb_data() {
    let mut disk = usb_disk(None);
    let mut boot = disk.children[0].clone();
    boot.mountpoint = Some("/boot".into());
    let mut lvm = disk.children[0].clone();
    lvm.fstype = "LVM2_member".into();
    let mut reserved = disk.children[0].clone();
    reserved.fstype.clear();
    reserved.size = Some(1024 * 1024);
    disk.children = vec![boot, lvm, reserved];
    let mut loop_disk = usb_disk(None);
    loop_disk.name = "loop0".into();
    assert!(partition_mounts(&[disk, loop_disk]).is_empty());
}
