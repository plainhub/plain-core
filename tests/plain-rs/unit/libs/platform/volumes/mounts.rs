use super::*;

#[test]
fn mount_facts_preserve_device_and_decode_escaped_mount_paths() {
    let rows = parse("31 20 8:1 / /mnt/USB\\040Drive rw - ext4 /dev/sda1 rw\n32 20 0:55 / /mnt/share rw - cifs //server/share rw\n").unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, "31");
    assert_eq!(rows[0].device, "8:1");
    assert_eq!(rows[0].root, "/");
    assert_eq!(rows[0].path, Path::new("/mnt/USB Drive"));
    assert_eq!(rows[1].fs_type, "cifs");
    assert_eq!(rows[1].source, "//server/share");
}

#[test]
fn incomplete_mount_data_is_an_error_not_a_complete_empty_scan() {
    assert!(parse("broken input").is_err());
    assert!(parse("31 20 8:1 / /mnt rw - ext4").is_err());
}
