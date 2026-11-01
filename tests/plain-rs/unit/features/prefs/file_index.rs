use super::*;

fn scope() -> Scope {
    Scope {
        directories: vec!["/users/example".into(), "/media/selected".into()],
        exclusions: vec!["/users/example/private".into()],
        defaults: Defaults {
            scan_directories: vec!["/users/example".into()],
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
    let fixture = tempfile::tempdir().unwrap();
    let root = normalized(fixture.path());
    for path in [
        "project/.git/objects",
        "project/node_modules/library",
        "project/target/debug",
        "project/src",
    ] {
        std::fs::create_dir_all(fixture.path().join(path)).unwrap();
    }
    std::fs::write(fixture.path().join("project/src/main.rs"), b"fn main() {}").unwrap();
    let scope = Scope {
        directories: vec![root],
        exclusions: vec![],
        defaults: scope().defaults,
    };
    let mut visited = Vec::new();
    crate::libs::file_walk::walk_entries(
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
    assert!(!visited.iter().any(|path| path.components().any(|part| {
        [".git", "node_modules", "target"]
            .iter()
            .any(|name| part.as_os_str() == *name)
    })));
}

#[test]
fn excluded_names_apply_to_directories_and_preserve_same_named_regular_files() {
    let scope = Scope {
        directories: vec!["/users/example".into()],
        exclusions: Vec::new(),
        defaults: Defaults {
            scan_directories: Vec::new(),
            excluded_directories: Vec::new(),
            excluded_names: vec!["build".into()],
        },
    };
    assert!(scope.contains_entry(Path::new("/users/example/build"), false));
    assert!(!scope.contains_entry(Path::new("/users/example/build"), true));
    assert!(!scope.contains_entry(Path::new("/users/example/build/source.rs"), false));
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

#[cfg(unix)]
#[test]
fn defaults_resolve_home_when_service_has_no_home_environment() {
    const CHILD: &str = "PLAIN_TEST_HOME_WITHOUT_ENV";
    if std::env::var_os(CHILD).is_some() {
        let expected = std::env::home_dir().unwrap();
        assert!(
            resolve_defaults()
                .scan_directories
                .iter()
                .any(|root| under(&normalized(&expected), root))
        );
        return;
    }
    let test = format!(
        "{}::defaults_resolve_home_when_service_has_no_home_environment",
        module_path!().split_once("::").unwrap().1
    );
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &test, "--nocapture"])
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn defaults_cover_all_users_and_mounted_data_disks() {
    let scope = Scope {
        directories: resolve_defaults().scan_directories,
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

#[cfg(target_os = "macos")]
#[test]
fn all_user_libraries_exclude_application_data_and_keep_cloud_documents() {
    let scope = Scope {
        directories: resolve_defaults().scan_directories,
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
