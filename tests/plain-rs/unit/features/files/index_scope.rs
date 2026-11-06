use super::rules::resolve_defaults;
use super::*;

fn volume(id: &str, path: &str, mode: IndexMode) -> VolumeRow {
    VolumeRow {
        id: id.into(),
        identity: id.into(),
        path: path.into(),
        is_available: true,
        index_mode: mode,
        included_dirs: Vec::new(),
        excluded_dirs: Vec::new(),
    }
}

#[test]
fn nested_volumes_apply_their_own_mode_and_retain_inactive_lists() {
    let mut parent = volume("parent", "/", IndexMode::Blocklist);
    parent.excluded_dirs = vec!["data/private".into()];
    let mut child = volume("child", "/data/disk", IndexMode::Allowlist);
    child.included_dirs = vec!["Photos".into()];
    child.excluded_dirs = vec!["Photos".into()];
    let mut scope = Scope {
        directories: source_directories(&[parent.clone(), child.clone()]),
        exclusions: Vec::new(),
        defaults: Defaults {
            excluded_directories: Vec::new(),
            excluded_names: Vec::new(),
        },
        volumes: vec![parent, child],
    };
    assert!(scope.contains_entry(Path::new("/data/public/file.txt"), false));
    assert!(!scope.contains_entry(Path::new("/data/private/file.txt"), false));
    assert!(scope.contains_entry(Path::new("/data/disk/Photos/image.jpg"), false));
    assert!(!scope.contains_entry(Path::new("/data/disk/Documents/file.txt"), false));
    scope.volumes[1].index_mode = IndexMode::Disabled;
    assert!(!scope.contains_entry(Path::new("/data/disk/Photos/image.jpg"), false));
    scope.volumes[1].index_mode = IndexMode::Allowlist;
    assert!(scope.contains_entry(Path::new("/data/disk/Photos/image.jpg"), false));
    scope.volumes[1].included_dirs.clear();
    assert!(!scope.contains_entry(Path::new("/data/disk/Photos/image.jpg"), false));
    scope.volumes[1].index_mode = IndexMode::Blocklist;
    assert!(scope.contains_entry(Path::new("/data/disk/Documents/file.txt"), false));
    assert!(!scope.contains_entry(Path::new("/data/disk/Photos/image.jpg"), false));
}

#[test]
fn offline_volume_rules_do_not_override_a_different_online_volume_at_the_same_path() {
    let mut old = volume("old", "/data/disk", IndexMode::Disabled);
    old.is_available = false;
    let current = volume("new", "/data/disk", IndexMode::Blocklist);
    let scope = Scope {
        directories: source_directories(&[old.clone(), current.clone()]),
        exclusions: Vec::new(),
        defaults: Defaults {
            excluded_directories: Vec::new(),
            excluded_names: Vec::new(),
        },
        volumes: vec![current, old],
    };
    assert!(scope.contains_entry(Path::new("/data/disk/file.txt"), false));
}

#[test]
fn relative_lists_preserve_root_and_reject_escape_paths() {
    #[cfg(windows)]
    assert!(relative_directories(vec!["C:/Photos".into()]).is_err());
    #[cfg(unix)]
    assert_eq!(
        relative_directories(vec!["Photos:2026".into()]).unwrap(),
        vec!["Photos:2026"]
    );

    assert_eq!(
        relative_directories(vec![
            "Photos/Trips".into(),
            "Photos".into(),
            "Photos".into()
        ])
        .unwrap(),
        vec!["Photos"]
    );
    assert_eq!(
        relative_directories(vec!["Photos".into(), "".into()]).unwrap(),
        vec![""]
    );
    for path in [
        "/Photos",
        "../Photos",
        "Photos/../Music",
        "./Photos",
        "Photos//Trips",
        "Photos\\Trips",
        "Photos/",
    ] {
        assert!(relative_directories(vec![path.into()]).is_err(), "{path}");
    }
}

fn scope() -> Scope {
    Scope {
        volumes: Vec::new(),
        directories: vec!["/users/example".into(), "/media/selected".into()],
        exclusions: vec!["/users/example/private".into()],
        defaults: Defaults {
            excluded_directories: vec!["/system".into()],
            excluded_names: vec![".git".into(), "node_modules".into(), "target".into()],
        },
    }
}

#[test]
fn exclusions_win_and_match_whole_path_components() {
    let scope = scope();
    for path in [
        "/users/example/private",
        "/users/example/private/photo.jpg",
        "/users/example/project/.git/objects/one",
        "/users/example/project/node_modules/module.js",
        "/users/example/project/target/debug/app",
        "/media/unselected/photo.jpg",
    ] {
        assert!(!scope.contains(Path::new(path)), "{path}");
    }
    for path in [
        "/users/example/private-notes/note.txt",
        "/users/example/project/src/main.rs",
        "/users/example/project/README.md",
        "/media/selected/photo.jpg",
    ] {
        assert!(scope.contains(Path::new(path)), "{path}");
    }
}

#[test]
fn removing_an_exclusion_restores_only_paths_in_the_scan_scope() {
    let mut scope = scope();
    scope.exclusions.clear();
    assert!(scope.contains(Path::new("/users/example/private/note.txt")));
    assert!(!scope.contains(Path::new("/media/unselected/note.txt")));
    assert!(!scope.contains(Path::new("/users/example/project/.git/config")));
}

#[test]
fn source_selection_cannot_override_a_default_exclusion() {
    let mut scope = scope();
    scope.directories.push("/system".into());
    assert!(!scope.contains(Path::new("/system/file.txt")));
}

#[test]
fn path_validation_rejects_relative_roots_and_retains_offline_absolute_roots() {
    assert!(validate_directories(vec!["relative".into()]).is_err());
    let root = std::env::current_dir()
        .unwrap()
        .join("offline-index-fixture");
    assert_eq!(
        validate_directories(vec![normalized(&root), normalized(&root)]).unwrap(),
        vec![normalized(&root)]
    );
}

#[cfg(any(feature = "library", feature = "filesystem", feature = "chat"))]
#[test]
fn directory_walk_skips_excluded_subtrees_before_visiting_them() {
    let fixture = tempfile::Builder::new()
        .prefix("index-scope-")
        .tempdir()
        .unwrap();
    let root = normalized(fixture.path());
    for path in [
        "project/.git/objects",
        "project/.internal/cache/deep",
        "project/lost+found/recovery",
        "project/node_modules/library",
        "project/target/debug",
        "project/src",
    ] {
        std::fs::create_dir_all(fixture.path().join(path)).unwrap();
    }
    std::fs::write(fixture.path().join("project/src/main.rs"), b"fn main() {}").unwrap();
    std::fs::write(
        fixture
            .path()
            .join("project/.internal/cache/deep/private.txt"),
        b"hidden",
    )
    .unwrap();
    std::fs::write(fixture.path().join("project/src/.notes"), b"visible file").unwrap();
    let scope = Scope {
        volumes: Vec::new(),
        directories: vec![root],
        exclusions: vec![],
        defaults: Defaults {
            excluded_names: vec![
                ".git".into(),
                "node_modules".into(),
                "target".into(),
                "lost+found".into(),
            ],
            ..scope().defaults
        },
    };
    let mut visited = Vec::new();
    plain_libs::file_walk::walk_entries(
        fixture.path(),
        |path| !scope.contains(path),
        || false,
        |path| {
            visited.push(path.to_owned());
            Ok(())
        },
    )
    .unwrap();
    assert!(visited.contains(&fixture.path().join("project/src/main.rs")));
    assert!(visited.contains(&fixture.path().join("project/src/.notes")));
    assert!(!visited.contains(&fixture.path().join("project/.internal")));
    assert!(!visited.contains(&fixture.path().join("project/lost+found")));
    assert!(!visited.iter().any(|path| path.components().any(|part| {
        [".git", "node_modules", "target"]
            .iter()
            .any(|name| part.as_os_str() == *name)
    })));
}

#[cfg(any(feature = "library", feature = "filesystem", feature = "chat"))]
#[test]
fn plain_trash_is_pruned_while_regular_trash_remains_indexable() {
    let fixture = tempfile::Builder::new()
        .prefix("trash-scope-")
        .tempdir()
        .unwrap();
    for path in [".plain-trash/nested", "trash/nested", "other"] {
        std::fs::create_dir_all(fixture.path().join(path)).unwrap();
    }
    for path in [
        ".plain-trash/nested/deleted.txt",
        "trash/nested/kept.txt",
        "other/trash",
    ] {
        std::fs::write(fixture.path().join(path), b"data").unwrap();
    }
    let scope = Scope {
        volumes: Vec::new(),
        directories: vec![normalized(fixture.path())],
        exclusions: Vec::new(),
        defaults: Defaults {
            excluded_directories: Vec::new(),
            excluded_names: resolve_defaults().excluded_names,
        },
    };
    let mut visited = Vec::new();
    plain_libs::file_walk::walk_entries(
        fixture.path(),
        |path| !scope.contains(path),
        || false,
        |path| {
            visited.push(path.to_owned());
            Ok(())
        },
    )
    .unwrap();
    assert!(!visited.contains(&fixture.path().join(".plain-trash")));
    assert!(!visited.contains(&fixture.path().join(".plain-trash/nested/deleted.txt")));
    assert!(visited.contains(&fixture.path().join("trash/nested/kept.txt")));
    assert!(visited.contains(&fixture.path().join("other/trash")));
}

#[test]
fn excluded_names_apply_to_directories_and_preserve_same_named_regular_files() {
    assert!(
        resolve_defaults()
            .excluded_names
            .iter()
            .any(|name| name == "lost+found")
    );
    let scope = Scope {
        volumes: Vec::new(),
        directories: vec!["/users/example".into()],
        exclusions: Vec::new(),
        defaults: Defaults {
            excluded_directories: Vec::new(),
            excluded_names: vec!["build".into()],
        },
    };
    assert!(scope.contains_entry(Path::new("/users/example/build"), false));
    assert!(!scope.contains_entry(Path::new("/users/example/build"), true));
    assert!(!scope.contains_entry(Path::new("/users/example/build/source.rs"), false));
    assert!(!scope.contains_entry(Path::new("/users/example/.internal"), true));
    assert!(!scope.contains_entry(Path::new("/users/example/.internal/file"), false));
    assert!(scope.contains_entry(Path::new("/users/example/.notes"), false));
}

#[test]
fn directory_settings_reject_files_and_parent_traversal_but_allow_offline_sources() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("file.txt");
    std::fs::write(&file, b"original").unwrap();
    assert!(validate_directories(vec![file.to_string_lossy().into_owned()]).is_err());
    assert!(
        validate_directories(vec![
            directory
                .path()
                .join("missing/../other")
                .to_string_lossy()
                .into_owned()
        ])
        .is_err()
    );
    assert!(
        validate_directories(vec![
            directory
                .path()
                .join("offline")
                .to_string_lossy()
                .into_owned()
        ])
        .is_ok()
    );
}

#[test]
fn drive_roots_remain_absolute_and_match_the_whole_drive() {
    assert_eq!(normalized(Path::new(r"C:\")), "C:/");
    assert_eq!(normalized(Path::new(r"\\?\C:\")), "C:/");
    assert!(under("C:/", "C:/"));
    assert!(under("C:/Users/example/file.txt", "C:/"));
    assert!(!under("D:/Users/example/file.txt", "C:/"));
    assert!(!under("C:relative.txt", "C:/"));
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn defaults_cover_all_users_and_mounted_data_disks() {
    let scope = Scope {
        volumes: Vec::new(),
        directories: vec!["/".into()],
        exclusions: vec!["/mnt/usb2/private".into()],
        defaults: resolve_defaults(),
    };
    for path in [
        "/home/first/Pictures/photo.jpg",
        "/home/second/Music/song.mp3",
        "/root/Documents/report.pdf",
        "/Users/first/Pictures/photo.jpg",
        "/Users/second/Music/song.mp3",
        "/mnt/usb1/photo.jpg",
        "/mnt/usb2/song.mp3",
        "/media/data/report.pdf",
        "/Volumes/Archive/photo.jpg",
    ] {
        assert!(scope.contains_entry(Path::new(path), false), "{path}");
    }
    for path in [
        "/mnt/usb2/private/photo.jpg",
        "/home/second/project/.git/config",
        "/mnt/usb1/project/node_modules/package/index.js",
        "/Users/first/project/target/debug/app",
    ] {
        assert!(!scope.contains_entry(Path::new(path), false), "{path}");
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn default_scope_prunes_system_directories_but_keeps_user_and_mounted_data() {
    let defaults = resolve_defaults();
    let scope = Scope {
        volumes: Vec::new(),
        directories: vec!["/".into()],
        exclusions: Vec::new(),
        defaults,
    };
    for path in [
        "/dev",
        "/usr",
        "/var",
        "/tmp",
        "/lost+found",
        "/mnt/disk/lost+found",
    ] {
        assert!(!scope.contains_entry(Path::new(path), true), "{path}");
    }
    #[cfg(target_os = "linux")]
    for path in ["/proc", "/sys", "/run", "/etc", "/boot", "/opt"] {
        assert!(!scope.contains_entry(Path::new(path), true), "{path}");
    }
    for path in ["/home", "/root", "/mnt", "/mnt/disk/Documents"] {
        assert!(scope.contains_entry(Path::new(path), true), "{path}");
    }
}

#[cfg(target_os = "macos")]
#[test]
fn all_user_libraries_exclude_application_data_and_keep_cloud_documents() {
    let scope = Scope {
        volumes: Vec::new(),
        directories: vec!["/".into()],
        exclusions: Vec::new(),
        defaults: resolve_defaults(),
    };
    for user in ["first", "second"] {
        assert!(!scope.contains_entry(
            Path::new(&format!("/Users/{user}/Library/Caches/photo.jpg")),
            false
        ));
        for suffix in [
            "Library",
            "Library/Mobile Documents",
            "Library/Mobile Documents/com~apple~CloudDocs",
        ] {
            assert!(scope.contains_entry(Path::new(&format!("/Users/{user}/{suffix}")), true));
        }
        assert!(scope.contains_entry(
            Path::new(&format!(
                "/Users/{user}/Library/Mobile Documents/com~apple~CloudDocs/report.pdf"
            )),
            false
        ));
    }
}

#[test]
fn selected_volume_keeps_its_rules_and_excludes_other_mounted_volumes() {
    let root = volume("root", "/", IndexMode::Blocklist);
    let mut disk = volume("disk", "/data/disk", IndexMode::Allowlist);
    disk.included_dirs = vec!["Photos".into()];
    let scope = Scope {
        directories: source_directories(&[root.clone(), disk.clone()]),
        exclusions: vec!["/private-data".into()],
        defaults: Defaults {
            excluded_directories: Vec::new(),
            excluded_names: Vec::new(),
        },
        volumes: vec![root, disk],
    };
    let selected = scope.for_volume("root").unwrap();
    assert_eq!(selected.directories, vec!["/"]);
    assert!(selected.contains_entry(Path::new("/documents/file.txt"), false));
    assert!(!selected.contains_entry(Path::new("/data/disk"), true));
    assert!(!selected.contains_entry(Path::new("/data/disk/Photos/file.jpg"), false));
    assert!(!selected.contains_entry(Path::new("/private-data/file.txt"), false));
    let selected = scope.for_volume("disk").unwrap();
    assert_eq!(selected.directories, vec!["/data/disk/Photos"]);
    assert!(selected.contains_entry(Path::new("/data/disk/Photos/file.jpg"), false));
    assert!(!selected.contains_entry(Path::new("/data/disk/Documents/file.txt"), false));
    assert!(!selected.contains_entry(Path::new("/documents/file.txt"), false));
    assert_eq!(scope.volumes[0].index_mode, IndexMode::Blocklist);
    assert_eq!(scope.volumes[1].index_mode, IndexMode::Allowlist);
    assert!(scope.for_volume("missing").is_err());
}

#[test]
fn selected_unavailable_or_unconfigured_volumes_have_no_scan_roots() {
    let mut scope = Scope {
        directories: vec!["/data/disk".into()],
        exclusions: Vec::new(),
        defaults: Defaults {
            excluded_directories: Vec::new(),
            excluded_names: Vec::new(),
        },
        volumes: vec![volume("disk", "/data/disk", IndexMode::Blocklist)],
    };
    for mode in [
        IndexMode::Unconfigured,
        IndexMode::Disabled,
        IndexMode::Allowlist,
    ] {
        scope.volumes[0].index_mode = mode;
        assert!(scope.for_volume("disk").unwrap().directories.is_empty());
    }
    scope.volumes[0].index_mode = IndexMode::Blocklist;
    scope.volumes[0].is_available = false;
    assert!(scope.for_volume("disk").unwrap().directories.is_empty());
}
