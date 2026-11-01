use super::*;

fn bytes(mask: u32, cookie: u32, name: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&7i32.to_ne_bytes());
    bytes.extend_from_slice(&mask.to_ne_bytes());
    bytes.extend_from_slice(&cookie.to_ne_bytes());
    bytes.extend_from_slice(&(name.len() as u32).to_ne_bytes());
    bytes.extend_from_slice(name);
    bytes
}

#[tokio::test]
async fn creation_does_not_index_unfinished_files_but_close_and_moves_do() {
    let mut watcher = Watcher::open().unwrap();
    watcher.directories.insert(7, PathBuf::from("/media"));
    let mut changes = |mask| {
        let event = decode(&bytes(mask, 91, b"item.png\0")).unwrap().remove(0);
        watcher.change(event).map(|event| event.kind)
    };
    assert_eq!(changes(libc::IN_CREATE), None);
    assert_eq!(changes(libc::IN_CLOSE_WRITE), Some(Kind::Completed));
    assert_eq!(changes(libc::IN_MOVED_FROM), Some(Kind::From(91)));
    assert_eq!(changes(libc::IN_MOVED_TO), Some(Kind::To(91)));
    assert_eq!(changes(libc::IN_DELETE), Some(Kind::Removed));
    assert_eq!(
        changes(libc::IN_CREATE | libc::IN_ISDIR),
        Some(Kind::Directory)
    );
}

#[test]
fn malformed_events_cannot_escape_the_observed_directory() {
    assert!(decode(&[0; 3]).is_err());
    for name in [b"../other\0".as_slice(), b"/absolute\0", b".\0", b"..\0"] {
        assert!(decode(&bytes(libc::IN_CLOSE_WRITE, 0, name)).is_err());
    }
    let mut truncated = bytes(libc::IN_CLOSE_WRITE, 0, b"item\0");
    truncated.pop();
    assert!(decode(&truncated).is_err());
}

#[tokio::test]
async fn queued_descendant_events_follow_directory_moves_and_overflow_requests_reconciliation() {
    let mut watcher = Watcher::open().unwrap();
    watcher
        .directories
        .insert(7, PathBuf::from("/media/source/child"));
    let event = decode(&bytes(libc::IN_CLOSE_WRITE, 0, b"item.png\0"))
        .unwrap()
        .remove(0);
    watcher.moved(Path::new("/media/source"), Path::new("/media/destination"));
    assert_eq!(
        watcher.change(event).unwrap().path,
        Path::new("/media/destination/child/item.png")
    );
    let event = decode(&bytes(libc::IN_Q_OVERFLOW, 0, b""))
        .unwrap()
        .remove(0);
    assert_eq!(watcher.change(event).unwrap().kind, Kind::Refresh);
}

#[tokio::test]
async fn real_filesystem_events_wait_for_close_and_keep_complete_bytes() {
    use std::io::Write;
    let directory = tempfile::tempdir().unwrap();
    let mut watcher = Watcher::open().unwrap();
    watcher.add(directory.path()).unwrap();
    let path = directory.path().join("image.png");
    let mut writer = std::fs::File::create(&path).unwrap();
    writer.write_all(b"first").unwrap();
    let events = watcher.read().await.unwrap();
    assert!(!events.is_empty());
    assert!(
        events
            .into_iter()
            .filter_map(|event| watcher.change(event))
            .all(|event| event.kind != Kind::Completed)
    );
    writer.write_all(b"second").unwrap();
    drop(writer);
    let events = watcher.read().await.unwrap();
    let completed = events
        .into_iter()
        .filter_map(|event| watcher.change(event))
        .filter(|event| event.kind == Kind::Completed)
        .collect::<Vec<_>>();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].path, path);
    assert_eq!(std::fs::read(&path).unwrap(), b"firstsecond");
}
