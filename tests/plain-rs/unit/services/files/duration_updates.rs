use super::*;
use std::{
    fs::{File, FileTimes},
    time::{Duration as TimeDuration, UNIX_EPOCH},
};

fn fixture(provider_catalog: bool, mp4: bool) -> (tempfile::TempDir, Arc<Runtime>, Value) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sample.mp4");
    let data = if mp4 {
        let mut header = vec![0u8; 12];
        header.extend(1000u32.to_be_bytes());
        header.extend(3211u32.to_be_bytes());
        let mut mvhd = 28u32.to_be_bytes().to_vec();
        mvhd.extend(b"mvhd");
        mvhd.extend(header);
        let mut moov = 36u32.to_be_bytes().to_vec();
        moov.extend(b"moov");
        moov.extend(mvhd);
        moov
    } else {
        b"unsupported".to_vec()
    };
    std::fs::write(&path, &data).unwrap();
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(
            FileTimes::new().set_modified(UNIX_EPOCH + TimeDuration::from_secs(1_700_000_000)),
        )
        .unwrap();
    let cache = Arc::new(DurationCache::new(&dir.path().join("cache")));
    let host = Arc::new(Host::default());
    let resources = Arc::new(plain_rs::host::resources::NativeResources::new(
        host.clone(),
    ));
    let mut runtime = Runtime::new(cache, host, resources, dir.path().join("cache"));
    Arc::get_mut(&mut runtime).unwrap().provider_catalog = provider_catalog;
    let row = json!({"id":"7","path":path,"size":data.len(),"updatedAt":"2023-11-14T22:13:20Z","durationMs":0});
    (dir, runtime, row)
}
async fn wait_idle(runtime: &Runtime) {
    loop {
        let completed = runtime.idle.notified();
        tokio::pin!(completed);
        completed.as_mut().enable();
        let pending = !runtime.pending.lock().unwrap().is_empty();
        if !pending {
            break;
        }
        completed.await;
    }
}

#[tokio::test]
async fn positive_duration_and_non_duration_kinds_do_not_open_cache_or_request_host() {
    let (directory, runtime, row) = fixture(true, true);
    let (_generation, mut requests) = runtime.host.connect();
    let mut positive = row.clone();
    positive["durationMs"] = json!(5_000_000_001_i64);
    runtime
        .enrich(DataType::Audio, std::slice::from_mut(&mut positive))
        .await
        .unwrap();
    let mut image = row.clone();
    runtime
        .enrich(DataType::Image, std::slice::from_mut(&mut image))
        .await
        .unwrap();
    assert_eq!(positive["durationMs"], 5_000_000_001_i64);
    assert!(!directory.path().join("cache").exists());
    assert!(runtime.pending.lock().unwrap().is_empty());
    assert!(requests.try_recv().is_err());
    runtime.shutdown().await;
}

#[tokio::test]
async fn native_probe_is_deferred_deduplicated_and_the_next_query_uses_exact_cached_milliseconds() {
    let (_directory, runtime, row) = fixture(false, true);
    let (_generation, mut requests) = runtime.host.connect();
    let permit = runtime.permits.acquire_many(10).await.unwrap();
    let mut first = vec![row.clone()];
    runtime.enrich(DataType::Video, &mut first).await.unwrap();
    let mut repeated = vec![row.clone()];
    runtime
        .enrich(DataType::Video, &mut repeated)
        .await
        .unwrap();
    assert_eq!(runtime.pending.lock().unwrap().len(), 1);
    assert_eq!(first[0]["durationMs"], 0);
    drop(permit);
    wait_idle(&runtime).await;
    runtime.enrich(DataType::Video, &mut first).await.unwrap();
    assert_eq!(first[0]["durationMs"], 3211);
    assert!(runtime.pending.lock().unwrap().is_empty());
    assert!(requests.try_recv().is_err());
    runtime.shutdown().await;
}

#[tokio::test]
async fn mp4_probe_only_revalidates_the_provider_without_requesting_a_duration_sample() {
    let (_directory, runtime, row) = fixture(true, true);
    let (generation, mut requests) = runtime.host.connect();
    let input = identity(DataType::Video, &row).unwrap();
    let job = tokio::spawn({
        let runtime = runtime.clone();
        async move {
            runtime
                .complete(DataType::Video, input, runtime.cache.revision())
                .await
        }
    });
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "systemMediaRows");
    assert_eq!(request["params"]["plan"]["ids"], json!(["7"]));
    assert_eq!(request["params"]["limit"], 1);
    runtime
        .host
        .reply(generation, json!({"id":request["id"],"result":[row]}))
        .unwrap();
    job.await.unwrap().unwrap();
    assert_eq!(runtime.cache.all().unwrap()[0].duration_ms, 3211);
    assert!(requests.try_recv().is_err());
    runtime.shutdown().await;
}

#[tokio::test]
async fn changed_provider_physical_file_or_invalidation_rejects_stale_work() {
    for change in ["provider", "physical", "invalidation"] {
        let (_directory, runtime, mut row) = fixture(true, true);
        let (generation, mut requests) = runtime.host.connect();
        let input = identity(DataType::Video, &row).unwrap();
        let revision = runtime.cache.revision();
        let job = tokio::spawn({
            let runtime = runtime.clone();
            async move { runtime.complete(DataType::Video, input, revision).await }
        });
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "systemMediaRows");
        match change {
            "provider" => row["size"] = json!(999),
            "physical" => std::fs::write(row["path"].as_str().unwrap(), b"replaced").unwrap(),
            "invalidation" => {
                runtime.cache.remove("video", &["7".into()]).unwrap();
            }
            _ => unreachable!(),
        }
        runtime
            .host
            .reply(generation, json!({"id":request["id"],"result":[row]}))
            .unwrap();
        job.await.unwrap().unwrap();
        assert!(runtime.cache.all().unwrap().is_empty(), "change={change}");
        runtime.shutdown().await;
    }
}

#[tokio::test]
async fn concurrent_shutdowns_cancel_provider_revalidation_and_ignore_late_replies() {
    let (_directory, runtime, row) = fixture(true, true);
    let (generation, mut requests) = runtime.host.connect();
    let mut rows = vec![row.clone()];
    runtime.enrich(DataType::Audio, &mut rows).await.unwrap();
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "systemMediaRows");
    tokio::join!(runtime.shutdown(), runtime.shutdown());
    assert!(runtime.pending.lock().unwrap().is_empty());
    runtime
        .host
        .reply(generation, json!({"id":request["id"],"result":[row]}))
        .unwrap();
    assert!(runtime.cache.all().unwrap().is_empty());
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn supported_audio_duration_is_rust_owned_with_a_platform_catalog() {
    let (directory, runtime, _) = fixture(true, false);
    let path = directory.path().join("audio-without-extension");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/audio-untagged.mp3"),
        &path,
    )
    .unwrap();
    let (_generation, mut requests) = runtime.host.connect();
    let reading = runtime.probe_milliseconds(path.to_str().unwrap());
    tokio::pin!(reading);
    let duration = tokio::select! {
        result = &mut reading => result.unwrap().unwrap(),
        request = requests.recv() => panic!("Unexpected metadata request: {request:?}"),
    };
    assert_eq!(duration / 1000, 4);
    assert!(requests.try_recv().is_err());
    assert!(!directory.path().join("cache").exists());
    runtime.shutdown().await;
}

#[tokio::test]
async fn opaque_audio_duration_reads_only_the_descriptor_range_without_sdk_metadata_or_staging() {
    let (directory, runtime, _) = fixture(true, false);
    let audio = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/audio-untagged.mp3"),
    )
    .unwrap();
    let mut bytes = b"prefix".to_vec();
    let base = bytes.len() as i64;
    bytes.extend_from_slice(&audio);
    bytes.extend_from_slice(b"suffix");
    let path = directory.path().join("descriptor");
    std::fs::write(&path, bytes).unwrap();
    let (generation, mut requests) = runtime.host.connect();
    let reading = tokio::spawn({
        let runtime = runtime.clone();
        async move { runtime.probe_milliseconds("content://fixture/audio").await }
    });
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "fileResourceStream");
    let stream = runtime
        .resources
        .attach_native(request["params"]["id"].as_str().unwrap(), generation)
        .unwrap();
    runtime
        .host
        .reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    let metadata: Value =
        serde_json::from_str(&stream.next_request().await.unwrap().unwrap()).unwrap();
    assert_eq!(
        metadata,
        json!({"path":"content://fixture/audio","file":true})
    );
    stream
        .file(
            plain_rs::host::resource_stream::NativeResourceFile::new(
                File::open(&path).unwrap(),
                base,
                audio.len() as i64,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reading.await.unwrap().unwrap().unwrap() / 1000, 4);
    assert!(requests.try_recv().is_err());
    assert!(!directory.path().join("cache").exists());
    runtime.shutdown().await;
}

#[tokio::test]
async fn duration_probe_keeps_unknown_distinct_from_zero_without_requesting_platform_facts() {
    let (_directory, runtime, row) = fixture(true, false);
    let (_generation, mut requests) = runtime.host.connect();
    assert_eq!(
        runtime
            .probe_milliseconds(row["path"].as_str().unwrap())
            .await
            .unwrap(),
        None
    );
    assert!(requests.try_recv().is_err());
    runtime.shutdown().await;
}

#[tokio::test]
async fn unknown_duration_never_requests_platform_metadata_or_caches_a_fabricated_value() {
    let (_directory, runtime, row) = fixture(true, false);
    let (_generation, mut requests) = runtime.host.connect();
    assert_eq!(
        runtime
            .probe_milliseconds(row["path"].as_str().unwrap())
            .await
            .unwrap(),
        None
    );
    let mut rows = vec![row];
    runtime.enrich(DataType::Audio, &mut rows).await.unwrap();
    wait_idle(&runtime).await;
    assert!(runtime.cache.all().unwrap().is_empty());
    assert!(requests.try_recv().is_err());
    runtime.shutdown().await;
}

#[tokio::test]
async fn inaccessible_sources_fail_uniformly_without_initializing_duration_cache() {
    for provider_catalog in [false, true] {
        let (directory, runtime, _) = fixture(provider_catalog, false);
        let (_generation, mut requests) = runtime.host.connect();
        let path = directory.path().join("missing-file");
        assert!(
            runtime
                .probe_milliseconds(path.to_str().unwrap())
                .await
                .is_err()
        );
        assert!(runtime.cache.all().unwrap().is_empty());
        assert!(requests.try_recv().is_err());
        runtime.shutdown().await;
    }
}
