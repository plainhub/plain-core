use super::*;

#[cfg(target_os = "macos")]
#[test]
fn automatic_scan_requires_explicit_access_for_protected_macos_locations() {
    for path in [
        "/Users/example/Music",
        "/Users/example/music/Track.mp3",
        "/Users/example/Documents/report.pdf",
        "/Users/example/Desktop",
        "/Users/example/Downloads",
        "/Users/example/Library/Mobile Documents/com~apple~CloudDocs",
        "/Volumes/Media/Archive.MUSICLIBRARY/database",
        "/Users/example/Pictures/Photos.photoslibrary/originals",
        "/Users/example/Pictures/Photo Booth Library/Pictures",
        "/Users/example/Movies/iMovie Library.imovielibrary",
    ] {
        assert!(requires_foreground_scan(Path::new(path)), "{path}");
    }
    for path in [
        "/",
        "/Users/example",
        "/Users/example/Music projects",
        "/Users/example/Pictures/photo.png",
        "/Users/example/Pictures/Photo Booth Library notes/photo.png",
        "/Users/example/work/Documents/report.pdf",
        "/Volumes/Media/Music/Track.mp3",
    ] {
        assert!(!requires_foreground_scan(Path::new(path)), "{path}");
    }
}

#[test]
fn media_exclusions() {
    let data = Path::new("/opt/plainapp/data");
    let cache = Path::new("/opt/plainapp/cache");
    let extra = vec!["/home/u/old".to_string()];
    let excl = |p: &str| is_media_excluded_name_or_root(p, data, cache, &extra);

    // System and virtual roots.
    assert!(excl("/usr/share/icons/a.png"));
    assert!(excl("/var/log/x.log"));
    assert!(excl("/proc/1/status"));
    // Component boundary: /usr2 is NOT /usr.
    assert!(!excl("/usr2/photo.jpg"));
    // User data areas stay in.
    assert!(!excl("/DATA/Gallery/photo.jpg"));
    assert!(!excl("/home/u/Photos/a.jpg"));
    assert!(!excl("/mnt/usb1/dcam/b.jpg"));
    // Hidden entries anywhere (also covers the app's `.plain-trash`).
    assert!(excl("/DATA/.thumbs/a.jpg"));
    assert!(excl("/home/u/proj/.git/HEAD"));
    assert!(excl("/disk/.plain-trash/old.png"));
    assert!(excl("/home/u/web/node_modules/lib.js"));
    assert!(!excl("/home/u/app/target/debug/x"));
    assert!(!excl("/home/u/site/dist/icon.svg"));
    assert!(!excl("/home/u/site/BUILD/style.css"));
    assert!(excl(
        "/Users/alice/Movies/CapCut/User Data/Cache/effect/7408409772950637830/image/blusher.png"
    ));
    assert!(excl(
        "/Users/alice/Library/Browser/User Data/GPUCache/image.png"
    ));
    assert!(excl(
        "/Users/alice/Library/Browser/User Data/Code Cache/image.png"
    ));
    assert!(!excl("/Users/alice/Pictures/Cache/edited.png"));
    assert!(!excl("/Users/alice/Pictures/Cachet/photo.jpg"));
    // Config-provided extra roots.
    assert!(excl("/home/u/old/legacy.png"));
    assert!(
        !excl("/home/u/oldphoto/x.png"),
        "prefix must respect boundaries"
    );
    // The app's own dirs (thumb cache pollution source).
    assert!(excl("/opt/plainapp/cache/thumbs/ab/x.webp"));
    assert!(excl("/opt/plainapp/data/plain.db"));
}

#[test]
fn platform_system_root_matching_respects_path_boundaries_and_windows_case() {
    assert!(under_root_case_insensitive(
        "D:/WINDOWS/System32",
        "D:/Windows"
    ));
    assert!(!under_root_case_insensitive(
        "D:/WindowsOld/file",
        "D:/Windows"
    ));
    #[cfg(target_os = "windows")]
    {
        assert!(is_media_excluded_at(
            "c:/windows/System32/a.dll",
            Path::new("Z:/data"),
            Path::new("Z:/cache"),
            &[]
        ));
        assert!(is_media_excluded_at(
            "D:/Program Files/App/a.exe",
            Path::new("Z:/data"),
            Path::new("Z:/cache"),
            &[]
        ));
    }
    #[cfg(target_os = "macos")]
    {
        assert!(is_media_excluded_at(
            "/System/Library/CoreServices/a",
            Path::new("/tmp/data"),
            Path::new("/tmp/cache"),
            &[]
        ));
        assert!(is_media_excluded_at(
            "/Applications/SomeApp/Resources/icon.png",
            Path::new("/tmp/data"),
            Path::new("/tmp/cache"),
            &[]
        ));
        assert!(!is_media_excluded_at(
            "/Users/alice/Pictures/a.jpg",
            Path::new("/tmp/data"),
            Path::new("/tmp/cache"),
            &[]
        ));
    }
    #[cfg(target_os = "linux")]
    {
        assert!(is_media_excluded_at(
            "/srv/site/dist/icon.svg",
            Path::new("/tmp/data"),
            Path::new("/tmp/cache"),
            &[]
        ));
        assert!(!is_media_excluded_at(
            "/srv2/site/dist/icon.svg",
            Path::new("/tmp/data"),
            Path::new("/tmp/cache"),
            &[]
        ));
        assert!(is_media_excluded_at(
            "/etc/ssl/certs/a.pem",
            Path::new("/tmp/data"),
            Path::new("/tmp/cache"),
            &[]
        ));
        assert!(!is_media_excluded_at(
            "/home/alice/Pictures/a.jpg",
            Path::new("/tmp/data"),
            Path::new("/tmp/cache"),
            &[]
        ));
    }
}

#[test]
fn photos_library_package_is_excluded_from_media_scan_on_macos() {
    #[cfg(target_os = "macos")]
    {
        let data = Path::new("/tmp/data");
        let cache = Path::new("/tmp/cache");
        let excluded = |path: &str| is_media_excluded_at(path, data, cache, &[]);
        assert!(excluded(
            "/Users/alice/Pictures/Photos Library.photoslibrary"
        ));
        assert!(excluded(
            "/Users/alice/Pictures/Photos Library.photoslibrary/resources/derivatives/cvt/photo.jpeg"
        ));
        assert!(excluded(
            "/Volumes/Backup/ALBUM.PHOTOSLIBRARY/originals/photo.jpeg"
        ));
        assert!(!excluded(
            "/Users/alice/Pictures/Photos Library.photoslibrary-backup/photo.jpeg"
        ));
        assert!(!excluded("/Users/alice/Pictures/photo.jpeg"));
    }
}

#[test]
fn infer_type_known_extensions() {
    assert_eq!(infer_type("a.mp3"), "audio");
    assert_eq!(infer_type("b.MP3"), "audio");
    assert_eq!(infer_type("c.MOV"), "video");
    assert_eq!(infer_type("d.png"), "image");
    assert_eq!(infer_type("e.txt"), "doc");
}

#[test]
fn infer_type_separates_documents_from_source_code() {
    for name in [
        "notes.txt",
        "README.md",
        "data.csv",
        "report.pdf",
        "letter.doc",
        "sheet.xlsx",
        "slides.pptx",
        "book.epub",
        "book.xls",
    ] {
        assert_eq!(infer_type(name), "doc", "{name}");
    }
    for name in [
        "main.rs",
        "script.js",
        "module.mjs",
        "style.css",
        "page.html",
        "app.json",
        "conf.xml",
        "Makefile",
        "archive.zip",
    ] {
        assert_eq!(infer_type(name), "other", "{name}");
    }
}

#[test]
fn the_default_scan_root_expands_to_all_configured_sources() {
    let sources = vec!["/fixtures/Pictures".into(), "/fixtures/Music".into()];
    assert_eq!(
        selected_roots("/".into(), &sources),
        vec![
            std::path::PathBuf::from("/fixtures/Pictures"),
            std::path::PathBuf::from("/fixtures/Music")
        ]
    );
    assert_eq!(
        selected_roots("/fixtures/selected".into(), &sources),
        vec![std::path::PathBuf::from("/fixtures/selected")]
    );
    assert_eq!(
        selected_roots("/".into(), &[]),
        Vec::<std::path::PathBuf>::new()
    );
}

#[cfg(target_os = "macos")]
#[test]
fn every_user_has_the_same_application_and_cloud_document_policy() {
    for user in ["first", "second"] {
        let cloud = format!("/Users/{user}/Library/Mobile Documents/com~apple~CloudDocs/photo.jpg");
        let application = format!("/Users/{user}/Library/Application Support/App/photo.jpg");
        assert!(!is_media_excluded_name_or_root(
            &cloud,
            Path::new("/app-data"),
            Path::new("/app-cache"),
            &[]
        ));
        assert!(is_cloud_drive_ancestor(
            &cloud,
            Path::new(&format!("/Users/{user}/Library"))
        ));
        assert!(is_media_excluded_name_or_root(
            &application,
            Path::new("/app-data"),
            Path::new("/app-cache"),
            &[]
        ));
        assert!(!is_cloud_drive_ancestor(
            &format!("{cloud}-backup"),
            Path::new("/Users/other/Library")
        ));
    }
}

#[test]
fn git_project_roots_cover_worktrees_submodules_and_bare_repositories() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    for name in [
        "regular/.git",
        "linked",
        "bare.git/objects",
        "bare.git/refs",
        "plain/data",
        "plain/reports",
    ] {
        std::fs::create_dir_all(root.join(name)).unwrap();
    }
    std::fs::write(
        root.join("linked/.git"),
        b"gitdir: ../regular/.git/worktrees/linked\n",
    )
    .unwrap();
    std::fs::write(root.join("bare.git/HEAD"), b"ref: refs/heads/main\n").unwrap();
    for name in ["regular", "linked", "bare.git"] {
        let repo = root.join(name);
        let nested = repo.join("nested");
        std::fs::create_dir(&nested).unwrap();
        let file = nested.join("file.txt");
        std::fs::write(&file, b"data").unwrap();
        assert_eq!(git_project_root(&repo), Some(repo.as_path()));
        assert_eq!(git_project_root(&nested), Some(repo.as_path()));
        assert_eq!(git_project_root(&file), Some(repo.as_path()));
    }
    for name in ["plain/data", "plain/reports"] {
        assert!(git_project_root(&root.join(name)).is_none());
    }
}

#[test]
fn hidden_files_and_directory_descendants_are_excluded_from_scanning() {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
    let directory = tempfile::Builder::new()
        .prefix("scan-hidden-")
        .tempdir_in(home)
        .unwrap();
    let root = directory.path();
    std::fs::create_dir(root.join(".hidden")).unwrap();
    for name in [".hidden/file.txt", ".secret.txt", "visible.txt"] {
        std::fs::write(root.join(name), b"data").unwrap();
    }
    assert_eq!(
        scan_excluded_root(&root.join(".secret.txt")),
        Some(root.join(".secret.txt").as_path())
    );
    assert_eq!(
        scan_excluded_root(&root.join(".hidden/file.txt")),
        Some(root.join(".hidden").as_path())
    );
    assert!(scan_excluded_root(&root.join("visible.txt")).is_none());
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let file = root.join("system-hidden.txt");
        let folder = root.join("system-hidden");
        std::fs::write(&file, b"data").unwrap();
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("file.txt"), b"data").unwrap();
        #[cfg(target_os = "macos")]
        {
            assert!(
                std::process::Command::new("chflags")
                    .arg("hidden")
                    .arg(&file)
                    .arg(&folder)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::MetadataExt;
            for path in [&file, &folder] {
                assert!(
                    std::process::Command::new("attrib.exe")
                        .arg("+H")
                        .arg(path)
                        .status()
                        .unwrap()
                        .success()
                );
                assert_ne!(
                    std::fs::symlink_metadata(path).unwrap().file_attributes() & 0x2,
                    0
                );
            }
        }
        assert_eq!(scan_excluded_root(&file), Some(file.as_path()));
        assert_eq!(
            scan_excluded_root(&folder.join("file.txt")),
            Some(folder.as_path())
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn native_hidden_library_ancestors_do_not_hide_cloud_documents() {
    let library = Path::new("/Users/profile/Library");
    let cloud = "/Users/profile/Library/Mobile Documents/com~apple~CloudDocs/photo.jpg";
    assert!(!hidden_ancestor_excluded(cloud, library, true));
    assert!(!hidden_ancestor_excluded(
        "/Users/profile/Library",
        library,
        true
    ));
    assert!(!hidden_ancestor_excluded(
        "/Users/profile/Library/Mobile Documents",
        library,
        true
    ));
    assert!(hidden_ancestor_excluded(
        "/Users/profile/Library/Caches/photo.jpg",
        library,
        true
    ));
    assert!(hidden_ancestor_excluded(
        "/Users/profile/Library/Mobile Documents/com~apple~CloudDocs/.hidden/photo.jpg",
        Path::new("/Users/profile/Library/Mobile Documents/com~apple~CloudDocs/.hidden"),
        true
    ));
}
