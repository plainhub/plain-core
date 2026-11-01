use super::*;
use std::fs::{File, FileTimes};
use std::time::{Duration, UNIX_EPOCH};

fn write(root: &Path, name: &str, seconds: u64) {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, name.as_bytes()).unwrap();
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(seconds)))
        .unwrap();
}

#[test]
fn selects_newest_files_across_directories_with_a_fixed_limit() {
    let dir = tempfile::tempdir().unwrap();
    for index in 0..150 {
        write(dir.path(), &format!("nested/{index:03}.txt"), index);
    }
    write(dir.path(), ".hidden", 200);
    let rows = list(dir.path(), 100);
    assert_eq!(rows.len(), 100);
    assert!(rows[0].path.ends_with("/.hidden"));
    assert!(rows[99].path.ends_with("/nested/051.txt"));
    assert!(
        rows.windows(2)
            .all(|pair| pair[0].updated_at >= pair[1].updated_at)
    );
    assert!(rows.iter().all(|row| !row.is_dir && row.child_count == 0));
    assert_eq!(rows[0].size, 7);
}

#[test]
fn equal_timestamps_have_a_repeatable_path_order_and_zero_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a", 10);
    write(dir.path(), "b", 10);
    write(dir.path(), "c", 10);
    let rows = list(dir.path(), 2);
    assert!(rows[0].path.ends_with("/c"));
    assert!(rows[1].path.ends_with("/b"));
    assert!(list(dir.path(), 0).is_empty());
    assert!(list(&dir.path().join("missing"), 100).is_empty());
}

#[cfg(unix)]
#[test]
fn file_links_use_target_metadata_and_directory_cycles_are_not_followed() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "target", 10);
    std::os::unix::fs::symlink(dir.path().join("target"), dir.path().join("alias")).unwrap();
    std::os::unix::fs::symlink(dir.path(), dir.path().join("cycle")).unwrap();
    std::os::unix::fs::symlink(dir.path().join("missing"), dir.path().join("broken")).unwrap();
    let rows = list(dir.path(), 100);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|row| row.path.ends_with("/alias")));
    assert!(rows.iter().all(|row| row.size == 6));
}
