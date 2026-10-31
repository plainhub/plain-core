use super::*;

fn row(id: &str, path: &Path, trash: bool, root: Option<&Path>) -> Row {
    Row {
        id: id.into(),
        path: path.to_string_lossy().into_owned(),
        trashed: trash,
        mime_type: "text/plain".into(),
        volume_root: root.map(|root| root.to_string_lossy().into_owned()),
    }
}

#[test]
fn trash_selection_preserves_idempotence_missing_rows_and_document_filtering() {
    let ids = vec!["1".into(), "2".into(), "3".into(), "4".into()];
    let a = row("1", Path::new("/one"), false, None);
    let b = row("2", Path::new("/two"), true, None);
    let mut c = row("3", Path::new("/three"), false, None);
    c.mime_type = "image/jpeg".into();
    let (planned, failed) = plan(
        DataType::Doc,
        Action::Trash,
        &ids,
        false,
        None,
        vec![a, b, c],
    )
    .unwrap();
    assert_eq!(planned.len(), 1);
    assert_eq!(planned[0].write.set_trash, Some(true));
    assert!(!planned[0].write.delete);
    assert_eq!(failed, vec!["2", "3", "4"]);
}

#[test]
fn delete_selection_matches_the_requested_trash_state() {
    let ids = vec!["1".into(), "2".into()];
    let (planned, failed) = plan(
        DataType::Image,
        Action::Delete,
        &ids,
        true,
        None,
        vec![
            row("1", Path::new("/one"), false, None),
            row("2", Path::new("/two"), true, None),
        ],
    )
    .unwrap();
    assert_eq!(planned[0].before.id, "2");
    assert!(planned[0].write.delete);
    assert_eq!(failed, vec!["1"]);
}

#[test]
fn move_names_consider_existing_files_and_every_other_item_in_the_batch() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let destination = root.join("target");
    for name in ["a", "b", "target"] {
        std::fs::create_dir(root.join(name)).unwrap();
    }
    let a = root.join("a/note.txt");
    let b = root.join("b/note.txt");
    for path in [&a, &b, &destination.join("note.txt")] {
        std::fs::write(path, b"data").unwrap();
    }
    let ids = vec!["1".into(), "2".into()];
    let (planned, failed) = plan(
        DataType::Doc,
        Action::Move,
        &ids,
        false,
        Some(&destination),
        vec![
            row("1", &a, false, Some(&root)),
            row("2", &b, false, Some(&root)),
        ],
    )
    .unwrap();
    assert!(failed.is_empty());
    assert_eq!(
        planned[0].write.display_name.as_deref(),
        Some("note (1).txt")
    );
    assert_eq!(
        planned[1].write.display_name.as_deref(),
        Some("note (2).txt")
    );
    assert_eq!(planned[0].write.relative_path.as_deref(), Some("target/"));
    assert!(!destination.join("note (1).txt").exists());
    assert!(!destination.join("note (2).txt").exists());
}

#[test]
fn cross_volume_moves_are_rejected_before_any_sdk_write() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let source_root = root.join("volume");
    let destination = root.join("other");
    std::fs::create_dir(&source_root).unwrap();
    std::fs::create_dir(&destination).unwrap();
    let source = source_root.join("note.txt");
    std::fs::write(&source, b"source").unwrap();
    let (planned, failed) = plan(
        DataType::Doc,
        Action::Move,
        &["1".into()],
        false,
        Some(&destination),
        vec![row("1", &source, false, Some(&source_root))],
    )
    .unwrap();
    assert!(planned.is_empty());
    assert_eq!(failed, vec!["1"]);
    assert_eq!(std::fs::read(source).unwrap(), b"source");
}

#[test]
fn delete_receipt_requires_a_changed_count_and_an_absent_provider_row() {
    let ids: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
    let rows = ids
        .iter()
        .map(|id| row(id, Path::new("/file"), false, None))
        .collect();
    let (planned, mut failed) =
        plan(DataType::Doc, Action::Delete, &ids, false, None, rows).unwrap();
    let successful = evaluate(
        Action::Delete,
        planned,
        vec![
            Receipt {
                id: "1".into(),
                count: 1,
                row: None,
            },
            Receipt {
                id: "2".into(),
                count: 0,
                row: None,
            },
            Receipt {
                id: "3".into(),
                count: 1,
                row: Some(row("3", Path::new("/file"), false, None)),
            },
        ],
        &mut failed,
    )
    .unwrap();
    assert_eq!(successful.len(), 1);
    assert_eq!(successful[0].id, "1");
    assert_eq!(failed, vec!["2", "3"]);
}

#[tokio::test]
async fn provider_delete_uses_two_batch_exchanges_and_no_business_action_on_host() {
    use std::sync::Arc;
    let directory = tempfile::tempdir().unwrap();
    let host = Arc::new(Host::default());
    let db = Arc::new(plain_rs::db::Db::open(&directory.path().join("plain.db")).unwrap());
    let platform = super::super::platform::FilePlatform::new(
        host.clone(),
        db,
        directory.path().to_path_buf(),
        directory.path().join("cache"),
        tokio::sync::broadcast::channel(8).0,
        #[cfg(feature = "runtime-media")]
        Arc::new(super::super::media_scans::MediaScans::new(
            tokio::sync::broadcast::channel(8).0,
            std::env::temp_dir().join("plain-media-scan-cache"),
        )),
    );
    let (generation, mut requests) = host.connect();
    let worker = {
        let host = host.clone();
        tokio::spawn(async move {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "mediaProviderRows");
            assert_eq!(
                request["params"],
                json!({"type":24,"ids":["1","2"],"includeVolumes":false})
            );
            host.reply(generation, json!({"id":request["id"],"result":[{"id":"1","path":"/one","trashed":false,"mimeType":"text/plain","volumeRoot":null},{"id":"2","path":"/two","trashed":false,"mimeType":"text/plain","volumeRoot":null}]})).unwrap();
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "mediaProviderWrite");
            let writes = request["params"]["writes"].as_array().unwrap();
            assert_eq!(writes.len(), 2);
            for write in writes {
                assert_eq!(write["delete"], true);
                assert!(write.get("action").is_none());
            }
            host.reply(generation, json!({"id":request["id"],"result":[{"id":"1","count":1,"row":null},{"id":"2","count":0,"row":null}]})).unwrap();
            assert!(requests.try_recv().is_err());
        })
    };
    let outcome = run(
        &host,
        &platform,
        DataType::Doc,
        Action::Delete,
        &["1".into(), "2".into()],
        false,
        "",
    )
    .await
    .unwrap();
    assert_eq!(outcome.successful.len(), 1);
    assert_eq!(outcome.failed_ids, vec!["2"]);
    worker.await.unwrap();
}
