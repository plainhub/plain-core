use super::*;

#[test]
fn cache_layout_is_independent_of_the_persistent_root() {
    let directory = tempfile::tempdir().unwrap();
    let data = directory.path().join("data");
    std::fs::create_dir(&data).unwrap();
    let cache = CachePaths::new(&directory.path().join("system-cache"));
    cache.initialize(&data).unwrap();
    assert!(std::ptr::eq(cache.root(), cache.clone().root()));
    assert_eq!(cache.image_cache(), cache.root().join("image_cache"));
    assert_eq!(cache.video_cache(), cache.root().join("video_cache"));
    assert_eq!(cache.shared_previews(), cache.root().join("share_open"));
    assert!(cache.shared_previews().is_dir());
    assert_eq!(cache.app_updates(), cache.root().join("updates"));
    assert_eq!(cache.pdf_previews(), cache.root().join("preview_pdf"));
    assert_eq!(
        cache.crash_report_export(),
        cache.root().join("crash_report.txt")
    );
    assert!(!data.join("cache").exists());
}

#[test]
fn shared_preview_initialization_preserves_an_existing_file_and_rejects_it() {
    let directory = tempfile::tempdir().unwrap();
    let data = directory.path().join("data");
    std::fs::create_dir(&data).unwrap();
    let cache = CachePaths::new(&directory.path().join("cache"));
    std::fs::create_dir(cache.root()).unwrap();
    std::fs::write(cache.shared_previews(), b"existing bytes").unwrap();
    assert!(cache.initialize(&data).is_err());
    assert_eq!(
        std::fs::read(cache.shared_previews()).unwrap(),
        b"existing bytes"
    );
}

#[test]
fn disposable_cache_cannot_contain_persistent_application_files() {
    let directory = tempfile::tempdir().unwrap();
    let data = directory.path().join("data");
    std::fs::create_dir(&data).unwrap();
    for root in [directory.path(), data.as_path()] {
        assert!(CachePaths::new(root).initialize(&data).is_err());
    }
    assert!(
        CachePaths::new(Path::new("relative-cache"))
            .initialize(&data)
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn cache_overlap_and_sdk_links_are_rejected_after_path_resolution() {
    let directory = tempfile::tempdir().unwrap();
    let data = directory.path().join("data");
    std::fs::create_dir(&data).unwrap();
    let alias = directory.path().join("alias");
    std::os::unix::fs::symlink(&data, &alias).unwrap();
    assert!(
        CachePaths::new(directory.path())
            .initialize(&alias)
            .is_err()
    );
    for child in ["image_cache", "video_cache", "share_open", "updates"] {
        let cache_root = directory.path().join(child);
        std::fs::create_dir(&cache_root).unwrap();
        std::os::unix::fs::symlink(&data, cache_root.join(child)).unwrap();
        assert!(CachePaths::new(&cache_root).initialize(&data).is_err());
    }
}
