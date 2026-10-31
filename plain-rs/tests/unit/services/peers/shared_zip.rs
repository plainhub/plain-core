use super::*;
use std::io::Read;
#[test]
fn zip_roundtrip_validates_names_sources_and_cancellation_and_cleans_temporary() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    std::fs::write(&source, "synthetic 中文").unwrap();
    let item = || Item {
        source_path: source.clone(),
        entry_name: "folder/中文 +?#%.txt".into(),
    };
    let (file, temporary) = pack(
        directory.path().join("zip"),
        vec![item()],
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let path = temporary.0.clone();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut entry = archive.by_name("folder/中文 +?#%.txt").unwrap();
    let mut text = String::new();
    entry.read_to_string(&mut text).unwrap();
    assert_eq!(text, "synthetic 中文");
    drop(entry);
    drop(archive);
    drop(temporary);
    assert!(!path.exists());
    for name in ["/escape", "../escape", "a/../b", "a\\b", "a//b", ""] {
        assert!(
            pack(
                directory.path().join("zip"),
                vec![Item {
                    entry_name: name.into(),
                    ..item()
                }],
                Arc::new(AtomicBool::new(false))
            )
            .is_err()
        );
    }
    assert!(
        pack(
            directory.path().join("zip"),
            vec![item(), item()],
            Arc::new(AtomicBool::new(false))
        )
        .is_err()
    );
    assert!(
        pack(
            directory.path().join("zip"),
            vec![item()],
            Arc::new(AtomicBool::new(true))
        )
        .is_err()
    );
    assert_eq!(
        std::fs::read_dir(directory.path().join("zip"))
            .unwrap()
            .count(),
        0
    );
}
#[test]
fn archive_preserves_large_files_and_nested_names_without_native_or_http_adapter() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    let expected = vec![7; 256 * 1024];
    std::fs::write(&source, &expected).unwrap();
    let (file, temporary) = pack(
        directory.path().join("zip"),
        vec![Item {
            source_path: source,
            entry_name: "root/子目录/a.txt".into(),
        }],
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let path = temporary.0.clone();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut content = Vec::new();
    archive
        .by_name("root/子目录/a.txt")
        .unwrap()
        .read_to_end(&mut content)
        .unwrap();
    assert_eq!(content, expected);
    drop(archive);
    drop(temporary);
    assert!(!path.exists());
}
#[test]
fn dropping_zip_owners_cancels_work_and_releases_its_spool() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    std::fs::write(&source, vec![8; 128 * 1024]).unwrap();
    let canceled = Arc::new(AtomicBool::new(false));
    let cancel = Cancel(canceled.clone());
    let (file, temporary) = pack(
        directory.path().join("zip"),
        vec![Item {
            source_path: source.clone(),
            entry_name: "a.txt".into(),
        }],
        canceled.clone(),
    )
    .unwrap();
    let path = temporary.0.clone();
    assert!(path.exists());
    drop(file);
    drop(temporary);
    drop(cancel);
    assert!(canceled.load(Ordering::SeqCst));
    assert!(!path.exists());
    assert!(
        pack(
            directory.path().join("zip"),
            vec![Item {
                source_path: source,
                entry_name: "a.txt".into()
            }],
            canceled
        )
        .is_err()
    );
}
