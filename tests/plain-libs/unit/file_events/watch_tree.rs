use super::*;
use crate::temporary::tempdir;
use std::os::unix::fs::symlink;

#[test]
fn watches_through_aliases_but_reports_ancestor_cycles_and_dangling_links() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("tree");
    let child = root.join("child");
    fs::create_dir_all(&child).unwrap();
    fs::write(child.join("file"), b"synthetic").unwrap();
    symlink(&child, root.join("alias")).unwrap();
    symlink(&root, child.join("loop")).unwrap();
    symlink(root.join("missing"), root.join("dangling")).unwrap();
    let root_link = temp.path().join("root-link");
    symlink(root, &root_link).unwrap();

    let mut files = Vec::new();
    let mut loops = 0;
    let mut dangling = 0;
    for entry in WatchTree::new(root_link.clone()) {
        match entry {
            Ok(entry) if entry.metadata().unwrap().is_file() => files.push(entry.path),
            Ok(_) => {}
            Err(error) => match error.kind {
                crate::file_events::ErrorKind::Generic(message) => {
                    assert!(message.contains("points to an ancestor"));
                    loops += 1;
                }
                crate::file_events::ErrorKind::Io(error) => {
                    assert_eq!(error.kind(), io::ErrorKind::NotFound);
                    dangling += 1;
                }
                kind => panic!("unexpected traversal error: {kind:?}"),
            },
        }
    }
    files.sort();
    assert_eq!(
        files,
        [root_link.join("alias/file"), root_link.join("child/file")]
    );
    assert_eq!((loops, dangling), (2, 1));
}

#[test]
fn limits_open_directories_and_keeps_every_deep_entry() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_path_buf();
    let mut directory = root.clone();
    for _ in 0..40 {
        fs::write(directory.join("file"), b"").unwrap();
        directory.push("sub");
        fs::create_dir(&directory).unwrap();
    }

    let mut tree = WatchTree::new(root);
    let mut files = 0;
    while let Some(entry) = tree.next() {
        if entry.unwrap().metadata().unwrap().is_file() {
            files += 1;
        }
        let open = tree
            .stack
            .iter()
            .filter(|dir| matches!(dir.entries, Entries::Open(_)))
            .count();
        assert!(open <= MAX_OPEN_DIRECTORIES);
    }
    assert_eq!(files, 40);
}
