use crate::test_runtime::Core as ContentServer;
use plain_libs::serde_json::json;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn stat_authorizes_the_resolved_path_once_and_rejects_invalid_references_before_host_access()
{
    let temp = plain_libs::temporary::tempdir().unwrap();
    let directory = temp.path().join("core");
    let server = ContentServer::open(&directory).unwrap();
    let state = server.runtime_state();
    state.file_platform.use_sdk_for_test();
    let path = directory.join("example.txt");
    std::fs::write(&path, b"example").unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let observed = calls.clone();
    let host = state.host.clone();
    let (generation, mut incoming) = host.connect();
    let worker = tokio::spawn(async move {
        while let Some(request) = incoming.recv().await {
            assert_eq!(request["method"], "systemAccessFacts");
            observed
                .lock()
                .unwrap()
                .push(request["params"]["features"].clone());
            host.reply(
                generation,
                json!({"id":request["id"],"result":{"granted":{"FILES":false}}}),
            )
            .unwrap();
        }
    });
    for reference in [
        "app://example.txt".to_owned(),
        format!("file://{}", path.display()),
    ] {
        assert!(
            crate::features::services::files::browse::stat_record(&state.file_platform, &reference)
                .await
                .unwrap()
                .is_some()
        );
    }
    assert!(calls.lock().unwrap().is_empty());
    assert!(crate::features::services::files::browse::stat_record(
        &state.file_platform, "app://missing.txt"
    ).await.unwrap().is_none());
    let external = temp.path().join("external.txt");
    std::fs::write(&external, b"external").unwrap();
    assert!(
        crate::features::services::files::browse::stat_record(
            &state.file_platform,
            external.to_str().unwrap()
        )
        .await
        .is_err()
    );
    assert_eq!(*calls.lock().unwrap(), vec![json!(["FILES"])]);
    for reference in [
        "",
        " ",
        ".",
        "app://../example.txt",
        "fid:invalid",
        "file://remote/example.txt",
    ] {
        assert!(
            crate::features::services::files::browse::stat_record(&state.file_platform, reference)
                .await
                .is_err()
        );
    }
    assert_eq!(calls.lock().unwrap().len(), 1);
    server.shutdown().await;
    worker.abort();
}

#[tokio::test]
async fn app_data_listing_filters_current_private_files_before_paging_and_search() {
    use plain_libs::filesystem::{SortBy, browse::Request};
    let fixture = plain_libs::temporary::tempdir().unwrap();
    let parent = fixture.path().canonicalize().unwrap();
    let directory = parent.join("data");
    let core = ContentServer::open(&directory).unwrap();
    let state = core.runtime_state();
    let platform = &state.file_platform;
    let legacy = directory.join("legacy.db");
    let upload = directory.join("uploads/abandoned");
    std::fs::write(&legacy, b"legacy fixture").unwrap();
    for (id, bytes) in [("abandoned", b"abandoned".as_slice()), ("resumable", b"keep".as_slice())] {
        let mut chunk = state.upload_chunks.open_chunk(id, 0, bytes.len() as u64, state.stop.clone()).await.unwrap();
        chunk.write(bytes).await.unwrap();
        chunk.finish().await.unwrap();
    }
    let nested_prefs = directory.join("uploads/prefs.json");
    std::fs::write(&nested_prefs, b"user file").unwrap();
    for name in ["plain.db-journal", "prefs.json.tmp", "tls-identity.json", "tls-identity.tmp", "tls.key", "tls.tmp"] {
        let path = directory.join(name);
        if !path.exists() {
            std::fs::write(path, b"private fixture").unwrap();
        }
    }
    let staged = directory.join(".restore-stage-fixture/payload/prefs.json");
    std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
    std::fs::write(&staged, b"staged fixture").unwrap();
    let mounts = platform.mounts().await.unwrap();
    assert!(mounts.as_array().unwrap().iter().any(|m| m["driveType"] == "APP" && m["mountPoint"] == directory.to_str().unwrap()));

    let request = |root: &std::path::Path, query: &str, limit| Request {
        root: root.to_str().unwrap().into(), query: query.into(), sort_by: SortBy::NameAsc,
        offset: 0, limit, count_only: false, count_children: false,
    };
    let page = super::execute(platform, request(&directory, "show_hidden:true", None)).await.unwrap();
    let names = page["items"].as_array().unwrap().iter().map(|item| item["name"].as_str().unwrap()).collect::<Vec<_>>();
    assert!(names.contains(&"legacy.db") && names.contains(&"uploads"));
    assert!(!names.contains(&".restore-stage-fixture"));
    assert!(platform.authorize(staged.to_str().unwrap(), false).await.is_err());
    assert!(platform.authorize(staged.to_str().unwrap(), true).await.is_err());
    assert!(state.files.delete(directory.to_string_lossy().into_owned()).await.is_err());
    for name in ["plain.db", "plain.db-wal", "plain.db-shm", "plain.db-journal", "prefs.json", "prefs.json.tmp", "tls-identity.json", "tls-identity.tmp", "tls.key", "tls.tmp"] {
        assert!(!names.contains(&name), "{name} appeared in App Data");
        let path = directory.join(name);
        assert!(platform.authorize(path.to_str().unwrap(), true).await.is_err());
        if path.exists() {
            assert!(platform.authorize(path.to_str().unwrap(), false).await.is_err());
        }
    }
    let mut search = super::search(platform, request(&parent, "text:prefs.json show_hidden:true", Some(1))).await.unwrap();
    let (page, done) = search.recv().await.unwrap().unwrap();
    assert!(done);
    assert_eq!(page.count, 1);
    assert_eq!(page.items[0].path, nested_prefs.to_str().unwrap());
    assert!(state.files.delete(upload.to_string_lossy().into_owned()).await.unwrap().removed);
    assert!(state.upload_chunks.list("abandoned").await.unwrap().is_empty());
    assert_eq!(state.upload_chunks.list("resumable").await.unwrap()[0].size_bytes, 4);
    assert_eq!(std::fs::read(legacy).unwrap(), b"legacy fixture");
    core.stop_core().await;
}
