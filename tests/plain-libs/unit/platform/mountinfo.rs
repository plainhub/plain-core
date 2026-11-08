//! Unit tests for `src/mountinfo.rs` — moved out-of-line; compiled
//! as the `tests` child module via `#[cfg(test)] #[path]` there.
use super::*;

#[test]
fn mount_matching_uses_path_components() {
    let entries = vec![
        MountInfoEntry {
            mount_point: "/".to_string(),
        },
        MountInfoEntry {
            mount_point: "/Volumes/Data".to_string(),
        },
    ];
    assert_eq!(
        find_best_mount_point(&entries, "/Volumes/Data/photo.jpg"),
        Some("/Volumes/Data".to_string())
    );
    assert_eq!(
        find_best_mount_point(&entries, "/Volumes/Database/photo.jpg"),
        Some("/".to_string())
    );
}

#[test]
fn clean_path_removes_parent_components() {
    assert_eq!(Path::new("/a/../b").clean_path(), PathBuf::from("/b"));
}

#[test]
#[cfg(target_os = "linux")]
fn resolve_root() {
    // Almost any path resolves to some mountpoint.
    let mp = resolve_mount_point("/").unwrap();
    assert!(!mp.is_empty());
}

#[test]
fn invalid_paths_are_rejected() {
    for path in ["", ".", "relative/file"] {
        assert!(resolve_mount_point(path).is_err(), "{path:?}");
    }
}

#[cfg(unix)]
#[test]
fn non_unicode_paths_are_rejected() {
    use std::os::unix::ffi::OsStrExt;
    assert!(resolve_mount_point(Path::new(std::ffi::OsStr::from_bytes(b"/tmp/\xff"))).is_err());
}

#[cfg(not(target_os = "linux"))]
#[test]
fn native_volume_resolution_preserves_missing_descendants_and_symlinks() {
    let directory = crate::temporary::tempdir().unwrap();
    let missing = directory.path().join("missing").join("file");
    let root = resolve_mount_point(directory.path()).unwrap();
    assert_eq!(resolve_mount_point(&missing).unwrap(), root);
    assert!(directory.path().canonicalize().unwrap().starts_with(&root));
    #[cfg(unix)]
    {
        let link = directory.path().join("alias");
        std::os::unix::fs::symlink(directory.path(), &link).unwrap();
        assert_eq!(resolve_mount_point(link.join("missing")).unwrap(), root);
    }
}
