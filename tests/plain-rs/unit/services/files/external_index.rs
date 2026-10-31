use super::*;

fn known(path: &str, key: Option<&str>) -> Known {
    Known {
        identity: Identity {
            media_type: 3,
            file_id: "provider:42".into(),
            path: path.into(),
        },
        file_key: key.map(str::to_owned),
    }
}

#[test]
fn recycled_provider_ids_cannot_override_a_different_physical_identity() {
    let row = known("/media/source.png", Some("disk:inode:birth1"));
    let current = [row.identity.clone()];
    assert!(!matches_identity(&row, Some("disk:inode:birth2"), &current));
    assert!(matches_identity(&row, Some("disk:inode:birth1"), &[]));
    assert!(matches_identity(&row, None, &current));
    assert!(!matches_identity(&row, None, &[]));
    let unrelated = [Identity {
        media_type: 2,
        ..row.identity.clone()
    }];
    assert!(!matches_identity(&row, None, &unrelated));
}

#[test]
fn directory_move_identity_batch_survives_reopen_without_stale_sources_or_neighbor_changes() {
    let directory = tempfile::tempdir().unwrap();
    let old = PathBuf::from("/media/source/image.png");
    let destination = PathBuf::from("/media/destination/image.png");
    let neighbor = PathBuf::from("/media/source-other/image.png");
    {
        let store = plain_rs::kv::Db::open(&directory.path().join("plain.db")).unwrap();
        save_identities(
            &store,
            &[
                (old.clone(), known(old.to_str().unwrap(), Some("one"))),
                (
                    neighbor.clone(),
                    known(neighbor.to_str().unwrap(), Some("two")),
                ),
            ],
            &[],
        )
        .unwrap();
        save_identities(
            &store,
            &[(
                destination.clone(),
                known(destination.to_str().unwrap(), Some("one")),
            )],
            &[old.clone()],
        )
        .unwrap();
        assert!(store.get(identity_key(&old)).unwrap().is_none());
    }
    let store = plain_rs::kv::Db::open(&directory.path().join("plain.db")).unwrap();
    assert!(store.get(identity_key(&old)).unwrap().is_none());
    for path in [&destination, &neighbor] {
        let row: Known =
            serde_json::from_str(&store.get(identity_key(path)).unwrap().unwrap()).unwrap();
        assert_eq!(row.identity.path, path.to_str().unwrap());
    }
    assert_eq!(store.scan_prefix(IDENTITY_PREFIX).count(), 2);
}

#[tokio::test]
async fn disappearance_requires_actual_missing_path_not_a_directory_or_symlink() {
    let directory = tempfile::tempdir().unwrap();
    assert!(!missing_path(directory.path()).await.unwrap());
    let path = directory.path().join("removed.png");
    assert!(missing_path(&path).await.unwrap());
    std::fs::write(&path, b"new content").unwrap();
    assert!(!missing_path(&path).await.unwrap());
    #[cfg(unix)]
    {
        let link = directory.path().join("dangling.png");
        std::os::unix::fs::symlink(directory.path().join("missing"), &link).unwrap();
        assert!(!missing_path(&link).await.unwrap());
    }
}

#[tokio::test]
async fn queued_changes_and_watcher_errors_precede_a_large_reconciliation_backlog() {
    let mut work = Work::default();
    work.files
        .extend((0..10000).map(|index| PathBuf::from(format!("/media/{index}.png"))));
    let (send, receive) = tokio::sync::oneshot::channel();
    send.send("closed file").unwrap();
    assert_eq!(
        change_or_work(receive, work.pending())
            .await
            .unwrap()
            .unwrap(),
        "closed file"
    );
    assert_eq!(work.files.len(), 10000);
    assert!(
        change_or_work(
            std::future::ready(Err::<(), _>("watcher error")),
            work.pending()
        )
        .await
        .unwrap()
        .is_err()
    );
    assert!(
        change_or_work(std::future::pending::<()>(), work.pending())
            .await
            .is_none()
    );
    assert_eq!(work.files().len(), BACKGROUND_BATCH_SIZE);
    assert_eq!(work.files.len(), 10000 - BACKGROUND_BATCH_SIZE);
}

#[test]
fn new_volume_reconciliation_replaces_prior_work_and_keeps_roots_breadth_first() {
    let roots = vec![
        PathBuf::from("/media/internal"),
        PathBuf::from("/media/removable"),
    ];
    let mut work = Work::new(&roots);
    assert!(work.pending());
    assert_eq!(work.directories.pop_front(), Some(roots[0].clone()));
    work.directories
        .push_back(PathBuf::from("/media/internal/child"));
    assert_eq!(work.directories.pop_front(), Some(roots[1].clone()));
    work.files
        .push_back(PathBuf::from("/media/removable/old.png"));
    work = Work::new(&roots[..1]);
    assert!(work.files.is_empty());
    assert!(work.walking.is_none());
    assert!(work.missing.is_none());
    assert!(work.reconciling);
    assert_eq!(work.directories.into_iter().collect::<Vec<_>>(), roots[..1]);
}
