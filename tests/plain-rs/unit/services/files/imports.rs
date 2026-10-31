use super::*;
use plain_rs::host::bridge::Host;
use serde_json::json;

fn fixture() -> (tempfile::TempDir, Arc<FileStore>) {
    let directory = tempfile::tempdir().unwrap();
    let db = Arc::new(plain_rs::db::Db::open(&directory.path().join("plain.db")).unwrap());
    let store = FileStore::new(db, directory.path().to_path_buf());
    (directory, store)
}
fn request(source: &str) -> Import {
    Import {
        source: source.into(),
        name: "picked.txt".into(),
        mime: "text/plain".into(),
        delete_source: false,
    }
}

#[tokio::test]
async fn file_uri_import_uses_rust_without_host_or_staging() {
    let (directory, store) = fixture();
    let source = directory.path().join("source+name.txt");
    std::fs::write(&source, b"local input").unwrap();
    let host = Arc::new(Host::default());
    let (_generation, mut requests) = host.connect();
    let resources = NativeResources::new(host);
    let (_stop, stopped) = watch::channel(false);
    let file = import(
        &store,
        &resources,
        &store.directory.join("cache"),
        request(&format!("FILE://{}", source.display())),
        stopped,
        Arc::new(tokio::sync::RwLock::new(())),
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read(store.stored_path(&file.real_path).unwrap()).unwrap(),
        b"local input"
    );
    assert!(source.exists());
    assert!(!directory.path().join("cache").exists());
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn platform_stream_imports_exact_bytes_and_cleans_staging_on_success_or_failure() {
    for outcome in ["finish", "failure", "cancel"] {
        let (directory, store) = fixture();
        let host = Arc::new(Host::default());
        let (generation, mut requests) = host.connect();
        let resources = Arc::new(NativeResources::new(host.clone()));
        let (stop, stopped) = watch::channel(false);
        let cache_changes = Arc::new(tokio::sync::RwLock::new(()));
        let task = {
            let resources = resources.clone();
            let store = store.clone();
            let cache_changes = cache_changes.clone();
            tokio::spawn(async move {
                import(
                    &store,
                    &resources,
                    &store.directory.join("cache"),
                    request("content://fixture/picked"),
                    stopped,
                    cache_changes,
                )
                .await
            })
        };
        let call = requests.recv().await.unwrap();
        assert_eq!(call["method"], "fileResourceStream");
        let stream = resources
            .attach_native(call["params"]["id"].as_str().unwrap(), generation)
            .unwrap();
        host.reply(generation, json!({"id":call["id"],"result":true}))
            .unwrap();
        stream.write(b"first".to_vec()).await.unwrap();
        assert!(cache_changes.try_write().is_err());
        stream.write(b" second".to_vec()).await.unwrap();
        match outcome {
            "finish" => stream.finish().await.unwrap(),
            "failure" => stream.fail().await.unwrap(),
            _ => {
                stop.send_replace(true);
            }
        }
        let result = task.await.unwrap();
        assert!(cache_changes.try_write().is_ok());
        if outcome == "finish" {
            let file = result.unwrap();
            assert_eq!(
                std::fs::read(store.stored_path(&file.real_path).unwrap()).unwrap(),
                b"first second"
            );
        } else {
            assert!(result.is_err());
            assert!(!directory.path().join("files").exists());
        }
        assert_eq!(
            std::fs::read_dir(directory.path().join("cache"))
                .unwrap()
                .count(),
            0
        );
        assert!(requests.try_recv().is_err());
    }
}

#[tokio::test]
async fn destructive_platform_requests_and_stopped_imports_fail_before_opening_a_resource() {
    let (directory, store) = fixture();
    let host = Arc::new(Host::default());
    let (_generation, mut requests) = host.connect();
    let resources = NativeResources::new(host);
    let (stop, stopped) = watch::channel(false);
    let mut destructive = request("content://fixture/picked");
    destructive.delete_source = true;
    assert!(
        import(
            &store,
            &resources,
            &store.directory.join("cache"),
            destructive,
            stopped.clone(),
            Arc::new(tokio::sync::RwLock::new(())),
        )
        .await
        .is_err()
    );
    stop.send_replace(true);
    assert!(
        import(
            &store,
            &resources,
            &store.directory.join("cache"),
            request("ph://fixture/picked"),
            stopped,
            Arc::new(tokio::sync::RwLock::new(())),
        )
        .await
        .is_err()
    );
    assert!(!directory.path().join("cache").exists());
    assert!(requests.try_recv().is_err());
}
