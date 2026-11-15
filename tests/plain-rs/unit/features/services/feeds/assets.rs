use super::*;

#[tokio::test]
async fn canceled_imports_are_drained_and_release_only_their_own_reference() {
    for reused in [false, true] {
        let directory = plain_libs::temporary::tempdir().unwrap();
        let db = Arc::new(Db::open(&directory.path().join("feeds.db")).unwrap());
        let assets = Arc::new(FeedAssets::new(db.clone(), directory.path().to_owned()));
        let existing = reused.then(|| {
            crate::features::chat::attachments::app_file_store::import_bytes(
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
                        let file =
                            crate::features::chat::attachments::app_file_store::import_bytes(
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
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Arc::new(Db::open(&directory.path().join("feeds.db")).unwrap());
    let assets = FeedAssets::new(db.clone(), directory.path().to_owned());
    let file = assets
        .import(|db, directory| {
            crate::features::chat::attachments::app_file_store::import_bytes(
                db,
                directory,
                b"accepted picture",
                "image/png",
            )
        })
        .await
        .unwrap();
    assets.wait_writes().await;
    assert!(file.real_path.exists());
    assert_eq!(db.get_app_file(&file.id).unwrap().ref_count, 1);
    assert!(
        assets
            .import(|_, _| Err(std::io::Error::other("import failed")))
            .await
            .is_err()
    );
    assets.wait_writes().await;
}

#[tokio::test]
async fn cached_assets_return_resolvable_fid_references_and_release_them() {
    use plain_libs::axum::{Router, routing::get};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        plain_libs::axum::serve(
            listener,
            Router::new()
                .route(
                    "/image",
                    get(|| async {
                        (
                            [("content-type", "image/png")],
                            b"generated image".as_slice(),
                        )
                    }),
                )
                .route(
                    "/icon",
                    get(|| async { ([("content-type", "image/svg+xml")], "<svg/>") }),
                )
                .route(
                    "/invalid",
                    get(|| async { ([("content-type", "text/plain")], "not an image") }),
                ),
        )
        .await
        .unwrap();
    });
    let directory = plain_libs::temporary::tempdir().unwrap();
    let db = Arc::new(Db::open(&directory.path().join("feeds.db")).unwrap());
    let assets = FeedAssets::new(db.clone(), directory.path().to_owned());
    let image = assets
        .cache(&format!("http://{address}/image"))
        .await
        .unwrap();
    assert!(image.starts_with("fid:"), "{image}");
    let path =
        crate::features::files::app_files::FileStore::new(db.clone(), directory.path().into())
            .resolve(image.strip_prefix("fid:").unwrap())
            .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"generated image");
    let duplicate = assets
        .cache(&format!("http://{address}/image"))
        .await
        .unwrap();
    assert_eq!(duplicate, image);
    let id = image
        .strip_prefix("fid:")
        .unwrap()
        .split('.')
        .next()
        .unwrap();
    assert_eq!(db.get_app_file(id).unwrap().ref_count, 2);
    assets.release(&duplicate);
    assert!(path.exists());
    assert_eq!(db.get_app_file(id).unwrap().ref_count, 1);
    assets.release(&image);
    assert!(!path.exists());
    assert!(db.get_app_file(id).is_none());
    assert!(
        assets
            .cache(&format!("http://{address}/icon"))
            .await
            .is_err()
    );
    assert!(
        assets
            .cache(&format!("http://{address}/invalid"))
            .await
            .is_err()
    );
    assert_eq!(db.count_app_files(), 0);
    let icon = assets
        .cache_bookmark_icon(&format!("http://{address}/icon"))
        .await
        .unwrap();
    assert!(icon.starts_with("fid:"), "{icon}");
    assets.release(&icon);
    assert_eq!(db.count_app_files(), 0);
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn feed_sync_persists_fid_images_and_logos_without_importing_them_again() {
    use plain_libs::axum::{Router, routing::get};
    for full_content in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let image_reads = Arc::new(AtomicUsize::new(0));
        let reads = image_reads.clone();
        let xml = format!(
            r#"<rss><channel><title>Generated feed</title><link>http://{address}/</link><image><url>http://{address}/logo</url></image><item><guid>generated-entry</guid><title>Generated entry</title><link>http://{address}/article</link><description>&lt;p&gt;Summary&lt;/p&gt;&lt;img src="http://{address}/image"&gt;</description></item></channel></rss>"#
        );
        let server = tokio::spawn(async move {
            plain_libs::axum::serve(listener, Router::new()
                .route("/rss", get(move || { let xml = xml.clone(); async move { xml } }))
                .route("/", get(|| async { "<html><body>Generated site</body></html>" }))
                .route("/article", get(|| async { "<article><p>Generated complete article text long enough to replace the summary.</p></article>" }))
                .route("/logo", get(|| async { ([("content-type", "image/png")], b"generated logo".as_slice()) }))
                .route("/image", get(move || { reads.fetch_add(1, Ordering::SeqCst); async { ([("content-type", "image/png")], b"generated picture".as_slice()) } })))
                .await.unwrap();
        });
        let directory = plain_libs::temporary::tempdir().unwrap();
        let db = Arc::new(Db::open(&directory.path().join("feeds.db")).unwrap());
        let assets = FeedAssets::new(db.clone(), directory.path().to_owned());
        db.feed_save(
            "feed",
            "Generated feed",
            &format!("http://{address}/rss"),
            full_content,
            &super::super::now(),
        )
        .unwrap();
        let (events, _) = tokio::sync::broadcast::channel(16);
        let feed = db.feed_get("feed").unwrap().unwrap();
        super::super::sync_feed(&db, &events, &feed, Some(&assets))
            .await
            .unwrap();
        let stored_feed = db.feed_get("feed").unwrap().unwrap();
        let entry = db.feed_entries_list("", 10, 0).unwrap().pop().unwrap();
        assert!(entry.image.starts_with("fid:"), "{}", entry.image);
        assert!(stored_feed.logo.starts_with("fid:"), "{}", stored_feed.logo);
        let store =
            crate::features::files::app_files::FileStore::new(db.clone(), directory.path().into());
        for reference in [&entry.image, &stored_feed.logo] {
            assert!(
                store
                    .resolve(reference.strip_prefix("fid:").unwrap())
                    .unwrap()
                    .exists()
            );
        }
        assert_eq!(image_reads.load(Ordering::SeqCst), 1);
        let refreshed = super::super::sync_entry_content_with_assets(&db, &entry.id, Some(&assets))
            .await
            .unwrap();
        assert_eq!(refreshed.image, entry.image);
        assert!(refreshed.content.contains("Generated complete article"));
        super::super::sync_feed(&db, &events, &stored_feed, Some(&assets))
            .await
            .unwrap();
        assert_eq!(image_reads.load(Ordering::SeqCst), 1);
        for reference in [&entry.image, &stored_feed.logo] {
            let id = reference
                .strip_prefix("fid:")
                .unwrap()
                .split('.')
                .next()
                .unwrap();
            assert_eq!(db.get_app_file(id).unwrap().ref_count, 1);
        }
        server.abort();
        assert!(server.await.unwrap_err().is_cancelled());
    }
}
