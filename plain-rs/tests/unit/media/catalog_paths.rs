use super::*;

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
fn infer_type_classifies_documents_off_shared_mime_table() {
    // text/* entries of the shared plain-rs table.
    for name in [
        "a.txt",
        "readme.log",
        "notes.md",
        "data.csv",
        "sheet.tsv",
        "page.html",
        "index.htm",
        "style.css",
        "script.js",
        "module.mjs",
    ] {
        assert_eq!(infer_type(name), "doc", "{name} must be a doc");
    }
    // plain-app extraDocumentMimeTypes beyond text/*.
    for name in ["report.pdf", "old.doc", "thesis.DOCX", "sheet.XLSX"] {
        assert_eq!(infer_type(name), "doc", "{name} must be a doc");
    }
    // Case-insensitive, same as the audio/video/image arms.
    assert_eq!(infer_type("RESUME.PDF"), "doc");

    // plain-app has no legacy .xls in extraDocumentMimeTypes — parity keeps
    // it out until the shared table decision changes on the phone side too.
    assert_eq!(infer_type("book.xls"), "other");
    assert_eq!(infer_type("app.json"), "other");
    assert_eq!(infer_type("conf.xml"), "other");

    // Non-docs stay as they were.
    assert_eq!(infer_type("song.mp3"), "audio");
    assert_eq!(infer_type("clip.mp4"), "video");
    assert_eq!(infer_type("photo.jpg"), "image");
    assert_eq!(infer_type("archive.zip"), "other");
    assert_eq!(infer_type("blob.bin"), "other");
    // Extensionless files have no extension-derived MIME and are not docs
    // (plain-app MediaStore assigns them application/octet-stream too).
    assert_eq!(infer_type("Makefile"), "other");
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
        vec![std::path::PathBuf::from("/")]
    );
}
