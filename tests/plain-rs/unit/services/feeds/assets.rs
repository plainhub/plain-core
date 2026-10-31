use super::*;

#[tokio::test]
async fn cancelled_imports_are_drained_and_release_only_their_own_reference() {
    for reused in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let db = Arc::new(Db::open(&directory.path().join("feeds.db")).unwrap());
        let assets = Arc::new(FeedAssets::new(db.clone(), directory.path().to_owned()));
        let existing = reused.then(|| {
            plain_rs::chat::attachments::app_file_store::import_bytes(
                &db,
                directory.path(),
                b"feed picture",
                "image/png",
            )
            .unwrap()
        });
        let (started, observed) = tokio::sync::oneshot::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        let (imported, result) = tokio::sync::oneshot::channel();
        let request = tokio::spawn({
            let assets = assets.clone();
            async move {
                assets
                    .import(move |db, directory| {
                        let _ = started.send(());
                        blocked.recv().unwrap();
                        let file = plain_rs::chat::attachments::app_file_store::import_bytes(
                            db,
                            directory,
                            b"feed picture",
                            "image/png",
                        )?;
                        let _ = imported.send(file.clone());
                        Ok(file)
                    })
                    .await
            }
        });
        observed.await.unwrap();
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
        let completion = assets.wait_writes();
        tokio::pin!(completion);
        assert!(futures_util::poll!(completion.as_mut()).is_pending());
        let other_feed = assets.scoped();
        let unrelated = other_feed.wait_writes();
        tokio::pin!(unrelated);
        assert!(futures_util::poll!(unrelated.as_mut()).is_ready());
        release.send(()).unwrap();
        completion.await;
        let file = result.await.unwrap();
        assert_eq!(file.real_path.exists(), reused);
        if let Some(existing) = existing {
            assert_eq!(existing.id, file.id);
            assert_eq!(db.get_app_file(&file.id).unwrap().ref_count, 1);
        } else {
            assert!(db.get_app_file(&file.id).is_none());
        }
    }
}

#[tokio::test]
async fn accepted_imports_retain_their_file_and_failed_imports_release_the_write_scope() {
    let directory = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(&directory.path().join("feeds.db")).unwrap());
    let assets = FeedAssets::new(db.clone(), directory.path().to_owned());
    let path = assets
        .import(|db, directory| {
            plain_rs::chat::attachments::app_file_store::import_bytes(
                db,
                directory,
                b"accepted picture",
                "image/png",
            )
        })
        .await
        .unwrap();
    assets.wait_writes().await;
    assert!(Path::new(&path).exists());
    let id = Path::new(&path).file_stem().unwrap().to_str().unwrap();
    assert_eq!(db.get_app_file(id).unwrap().ref_count, 1);
    assert!(
        assets
            .import(|_, _| Err(std::io::Error::other("import failed")))
            .await
            .is_err()
    );
    assets.wait_writes().await;
}
