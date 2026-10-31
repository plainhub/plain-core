use super::*;
use std::sync::Arc;

#[tokio::test]
async fn animation_detection_never_exchanges_with_the_host() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("animation.gif");
    std::fs::write(&path, b"GIF89a\0\0").unwrap();
    let host = Host::default();
    let (_generation, mut requests) = host.connect();
    assert_eq!(
        read(&host, "animated", path.to_str().unwrap(), json!({}))
            .await
            .unwrap(),
        Value::Bool(true)
    );
    assert!(requests.try_recv().is_err());
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn native_app_paths_and_format_helpers_are_direct_and_do_not_restore_retired_commands() {
    let directory = tempfile::tempdir().unwrap();
    let host = Arc::new(Host::default());
    let (_generation, mut requests) = host.connect();
    let db = Arc::new(plain_rs::db::Db::open(&directory.path().join("plain.db")).unwrap());
    let path = plain_rs::services::files::references::resolve(
        db.clone(),
        directory.path().to_path_buf(),
        "app://nested/file.txt",
    )
    .unwrap();
    assert_eq!(Path::new(&path), directory.path().join("nested/file.txt"));
    assert!(
        plain_rs::services::files::references::resolve(
            db,
            directory.path().to_path_buf(),
            "app://../escape"
        )
        .is_err()
    );
    assert_eq!(
        read(&host, "mime", "photo.jpg", json!({})).await.unwrap(),
        Value::String("image/jpeg".into())
    );
    assert_eq!(
        read(&host, "decodePng", "/missing/photo.jpg", json!({}))
            .await
            .unwrap(),
        Value::Bool(false)
    );
    assert!(native("appDir", "", json!({})).await.is_err());
    assert!(native("zipExtract", "", json!({})).await.is_err());
    assert!(native("unknown", "", json!({})).await.is_err());
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn ordinary_mime_queries_are_portable_and_do_not_request_sdk_metadata() {
    let host = Host::default();
    let (_generation, mut requests) = host.connect();
    assert_eq!(
        read(&host, "mime", "track.MP3", json!({})).await.unwrap(),
        Value::String("audio/mpeg".into())
    );
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn opaque_mime_queries_use_the_existing_resource_primitive_on_every_platform() {
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let lookup = {
        let host = host.clone();
        tokio::spawn(async move { read(&host, "mime", "content://fixture/123", json!({})).await })
    };
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "systemFileResource");
    assert_eq!(
        request["params"],
        json!({"operation":"mime","path":"content://fixture/123","params":{}})
    );
    host.reply(
        generation,
        json!({"id":request["id"],"result":"video/3gpp"}),
    )
    .unwrap();
    assert_eq!(lookup.await.unwrap().unwrap(), json!("video/3gpp"));
    assert!(requests.try_recv().is_err());
}
