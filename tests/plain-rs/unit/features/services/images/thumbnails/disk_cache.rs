use super::*;
use futures_util::FutureExt;

#[test]
fn only_reserved_bytes_can_be_written_and_failure_releases_them() {
    let cache = Arc::new(DiskCache::new(100));
    let directory = plain_libs::temporary::tempdir().unwrap();
    let first_path = directory.path().join("first.png");
    let second_path = directory.path().join("second.png");
    assert!(
        cache.reserve(first_path.clone(), 60).is_none(),
        "don't write while counting"
    );
    cache.maintain(directory.path()).unwrap();
    let first = cache.reserve(first_path.clone(), 60).unwrap();
    assert!(cache.reserve(second_path.clone(), 41).is_none());
    let second = cache.reserve(second_path.clone(), 40).unwrap();
    assert!(
        cache
            .reserve(directory.path().join("third.jpg"), 1)
            .is_none()
    );
    drop(second);
    first.persist(&[0; 60]).unwrap();
    assert_eq!(cache.total(), 60);
    let invalid_parent = first_path.join("impossible.jpg");
    assert!(
        cache
            .reserve(invalid_parent, 40)
            .unwrap()
            .persist(&[0; 40])
            .is_err()
    );
    assert_eq!(cache.total(), 60);
    assert!(cache.reserve(second_path, 40).is_some());
}

#[test]
fn refusal_raises_cleanup_and_reads_do_not_change_removal_order() {
    let cache = Arc::new(DiskCache::new(100));
    let directory = plain_libs::temporary::tempdir().unwrap();
    let old = directory.path().join("a.jpg");
    let newer = directory.path().join("b.png");
    for (index, (path, size)) in [(&old, 20), (&newer, 65)].into_iter().enumerate() {
        fs::write(path, vec![0; size]).unwrap();
        fs::File::open(path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(
                SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(1_700_000_000 + index as u64),
            ))
            .unwrap();
    }
    cache.maintain(directory.path()).unwrap();
    assert_eq!(cache.total(), 85);
    assert!(
        cache
            .reserve(directory.path().join("new.jpg"), 16)
            .is_none()
    );
    assert!(cache.wake.notified().now_or_never().is_some());
    assert_eq!(fs::read(&old).unwrap().len(), 20);
    cache.maintain(directory.path()).unwrap();
    assert!(!old.exists());
    assert!(newer.exists());
    assert_eq!(cache.total(), 65);
}

#[cfg(unix)]
#[test]
fn scanning_does_not_follow_links_and_counts_old_nested_thumbnails() {
    use std::os::unix::fs::symlink;
    let cache = DiskCache::new(100);
    let directory = plain_libs::temporary::tempdir().unwrap();
    let root = directory.path().join("thumbs");
    let original = directory.path().join("original.png");
    fs::create_dir_all(root.join("old/location")).unwrap();
    fs::write(root.join("old/location/thumbnail.jpg"), [0; 18]).unwrap();
    fs::write(&original, [0; 80]).unwrap();
    symlink(&original, root.join("link.png")).unwrap();
    symlink(directory.path(), root.join("linked-directory")).unwrap();
    fs::write(root.join("pending.tmp"), [0; 100]).unwrap();
    cache.maintain(&root).unwrap();
    assert_eq!(cache.total(), 18);
    assert_eq!(fs::read(original).unwrap().len(), 80);
}

#[test]
fn eviction_does_not_delete_a_file_that_has_not_finished_being_saved() {
    let cache = Arc::new(DiskCache::new(100));
    let directory = plain_libs::temporary::tempdir().unwrap();
    let existing = directory.path().join("existing.jpg");
    fs::write(&existing, [0; 85]).unwrap();
    cache.maintain(directory.path()).unwrap();
    let publishing = directory.path().join("publishing.jpg");
    let reservation = cache.reserve(publishing.clone(), 10).unwrap();
    fs::write(&publishing, [0; 10]).unwrap();
    fs::File::open(&publishing)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(SystemTime::UNIX_EPOCH))
        .unwrap();
    cache.maintain(directory.path()).unwrap();
    assert!(!existing.exists());
    assert!(publishing.exists());
    assert_eq!(cache.total(), 10);
    fs::remove_file(&publishing).unwrap();
    reservation.persist(&[0; 10]).unwrap();
    assert_eq!(cache.total(), 10);
}

#[test]
fn clearing_the_cache_recounts_space_before_allowing_new_writes() {
    let cache = Arc::new(DiskCache::new(100));
    let directory = plain_libs::temporary::tempdir().unwrap();
    let old = directory.path().join("old.jpg");
    let new = directory.path().join("new.jpg");
    fs::write(&old, [0; 100]).unwrap();
    cache.maintain(directory.path()).unwrap();
    assert!(cache.reserve(new.clone(), 1).is_none());
    fs::remove_file(old).unwrap();
    cache.reset();
    assert!(cache.reserve(new.clone(), 1).is_none());
    cache.maintain(directory.path()).unwrap();
    cache
        .reserve(new.clone(), 100)
        .unwrap()
        .persist(&[0; 100])
        .unwrap();
    assert_eq!(fs::metadata(new).unwrap().len(), 100);
    assert_eq!(cache.total(), 100);
}
