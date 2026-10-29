use super::*;
use plain_rs::{db::Db, enums::DataType};

fn row(title: &str) -> MediaFileRow {
    MediaFileRow {
        album: String::new(),
        album_artist: String::new(),
        album_id: "native-album".into(),

        id: "audio-id".into(),
        data_type: 1,
        path: "/missing/native-song.mp3".into(),
        title: title.into(),
        artist: "Artist".into(),
        bucket_id: "bucket".into(),
        size_bytes: 42,
        duration_ms: 42_000,
        cover_image_id: None,
        width: 0,
        height: 0,
        rotation: 0,
        taken_at: None,
        created_at: "first".into(),
        updated_at: "first".into(),
        favorited: false,
        trashed: false,
        original_path: None,
    }
}

#[test]
fn indexed_audio_preserves_identity_tags_and_duration_and_falls_back_only_for_empty_titles() {
    let metadata = from_row(row("Indexed Title"));
    assert_eq!(metadata.file_id.as_deref(), Some("audio-id"));
    assert_eq!(metadata.track.title, "Indexed Title");
    assert_eq!(metadata.track.artist, "Artist");
    assert_eq!(metadata.track.album_id, "native-album");
    assert_eq!(metadata.track.duration_ms, 42_000);
    let metadata = from_row(row(""));
    assert_eq!(metadata.track.title, "native-song");
    assert_eq!(metadata.track.duration_ms, 42_000);
}

#[test]
fn native_unindexed_files_are_probed_for_tags_without_creating_persistent_media_rows() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/audio-tagged.mp3"
    );
    let metadata = read_file(path).unwrap();
    assert!(metadata.file_id.is_none());
    assert_eq!(metadata.title, "Hydrate Title");
    assert_eq!(metadata.artist, "Hydrate Artist");
    assert!(metadata.duration_ms > 0);
    assert!(
        db.media_file_by_path(DataType::Audio, path)
            .unwrap()
            .is_none()
    );
    assert!(read_file("/missing/nonexistent.mp3").is_err());
}

#[tokio::test]
async fn uri_audio_tags_use_one_bounded_descriptor_without_sdk_metadata_or_staging() {
    use serde_json::json;
    use std::sync::Arc;
    let directory = tempfile::tempdir().unwrap();
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/audio-tagged.mp3"
    ))
    .unwrap();
    let mut wrapped = b"private-prefix".to_vec();
    let offset = wrapped.len() as i64;
    wrapped.extend_from_slice(&bytes);
    wrapped.extend_from_slice(b"private-suffix");
    let source = directory.path().join("descriptor");
    std::fs::write(&source, wrapped).unwrap();
    let host = Arc::new(crate::host::bridge::Host::default());
    let (generation, mut requests) = host.connect();
    let resources = Arc::new(crate::host::resources::NativeResources::new(host.clone()));
    let cache = directory.path().join("cache");
    let reading = tokio::spawn({
        let resources = resources.clone();
        let cache = cache.clone();
        async move { read_uri(&resources, &cache, "content://fixture/audio").await }
    });
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "fileResourceStream");
    let stream = resources
        .attach_native(request["params"]["id"].as_str().unwrap(), generation)
        .unwrap();
    host.reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    let packet: serde_json::Value =
        serde_json::from_str(&stream.next_request().await.unwrap().unwrap()).unwrap();
    assert_eq!(
        packet,
        json!({"path":"content://fixture/audio","file":true})
    );
    stream
        .file(
            crate::native::resource_stream::NativeResourceFile::new(
                std::fs::File::open(source).unwrap(),
                offset,
                bytes.len() as i64,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let track = reading.await.unwrap().unwrap();
    assert_eq!(track.path, "content://fixture/audio");
    assert_eq!(track.title, "Hydrate Title");
    assert_eq!(track.artist, "Hydrate Artist");
    assert!(track.duration_ms > 0);
    assert!(track.file_id.is_none());
    assert!(!cache.exists());
    assert!(requests.try_recv().is_err());
}

#[test]
fn file_uri_preserves_the_reference_but_uses_the_decoded_filename_for_missing_tags() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("untagged song.mp3");
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/audio-untagged.mp3"
        ),
        &path,
    )
    .unwrap();
    let uri = format!("file://{}", path.to_str().unwrap().replace(' ', "%20"));
    let track = read_file(&uri).unwrap();
    assert_eq!(track.path, uri);
    assert_eq!(track.title, "untagged song");
    assert_eq!(track.artist, "");
    assert!(track.duration_ms > 0);
    assert!(read_file(directory.path().to_str().unwrap()).is_err());
}
