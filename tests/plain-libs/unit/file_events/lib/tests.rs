use std::{fs, time::Duration};

use tempfile::tempdir;

use super::*;

#[test]
fn integration() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let root = dir.path().canonicalize()?;

    let (tx, rx) = std::sync::mpsc::channel();

    let mut watcher = RecommendedWatcher::new(tx)?;

    watcher.watch(&root, RecursiveMode::Recursive)?;

    let file_path = root.join("file.txt");
    fs::write(&file_path, b"Lorem ipsum")?;

    let event = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("no events received")
        .expect("received an error");

    assert_eq!(event.paths, vec![file_path]);

    Ok(())
}

#[test]
fn native_watcher_can_move_into_the_service_task() {
    fn require_send<T: Send>() {}
    require_send::<RecommendedWatcher>();
}
