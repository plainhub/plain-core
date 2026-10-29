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
