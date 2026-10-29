use super::*;

#[test]
fn smb_identity_is_share_scoped_and_does_not_store_mount_credentials() {
    assert_eq!(
        share_identity("//user@SERVER/share/").unwrap(),
        "SHARE:server/share"
    );
    assert_eq!(
        share_identity("//server/share").unwrap(),
        "SHARE:server/share"
    );
    assert_eq!(
        share_identity("//account@realm@SERVER/share@team/").unwrap(),
        "SHARE:server/share@team"
    );
    assert_ne!(
        share_identity("//server/other").unwrap(),
        share_identity("//server/share").unwrap()
    );
    assert!(share_identity("//server").is_err());
    assert!(share_identity("//server//").is_err());
}

#[test]
fn windows_extended_paths_use_the_same_frame_as_volume_roots() {
    assert_eq!(
        windows_path(r"\\?\C:\folder\file.txt"),
        "C:/folder/file.txt"
    );
    assert_eq!(
        windows_path(r"\\?\UNC\server\share\file.txt"),
        "//server/share/file.txt"
    );
    assert_eq!(windows_path(r"C:\folder\file.txt"), "C:/folder/file.txt");
}

#[test]
fn a_native_location_has_a_stable_volume_and_normalizes_missing_entry_paths() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("file.txt");
    std::fs::write(&file, b"file").unwrap();
    let before = locate(&file).unwrap();
    std::fs::remove_file(&file).unwrap();
    let after = locate(&file).unwrap();
    assert_eq!(before, after);
    assert!(after.path.starts_with(&after.volume.path));
    assert!(!after.volume.identity.is_empty());
    assert!(locate(Path::new("relative/file.txt")).is_err());
}
