use super::*;
use plain_rs::host::bridge::Host;
use serde_json::json;
use std::sync::Arc;

fn id3() -> Vec<u8> {
    let body = b"\x03eng\0lyrics";
    let mut frame = b"USLT".to_vec();
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(&[0, 0]);
    frame.extend_from_slice(body);
    let mut bytes = b"ID3\x03\0\0".to_vec();
    bytes.extend_from_slice(&[0, 0, 0, frame.len() as u8]);
    bytes.extend(frame);
    bytes
}

#[tokio::test]
async fn playback_lyrics_choose_sidecars_in_rust_without_platform_requests() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("song+name.mp3");
    let sidecar = directory.path().join("song+name.lrc");
    std::fs::write(&path, id3()).unwrap();
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let resources = NativeResources::new(host.clone());
    let (stop, stopped) = watch::channel(false);
    for value in [None, Some(""), Some("[00:01]sidecar"), Some("   ")] {
        if let Some(value) = value {
            std::fs::write(&sidecar, value).unwrap();
        }
        let expected = value.filter(|value| !value.is_empty()).unwrap_or("lyrics");
        let file_uri = format!("file://{}", path.display()).replace('+', "%2B");
        assert_eq!(
            read_for_playback(file_uri, Some((&resources, stopped.clone())))
                .await
                .unwrap(),
            expected,
        );
        assert_eq!(
            read(path.to_string_lossy().into_owned(), None)
                .await
                .unwrap(),
            "lyrics"
        );
    }
    assert!(requests.try_recv().is_err());
    stop.send(true).unwrap();
    assert_eq!(
        read_for_playback(
            path.to_string_lossy().into_owned(),
            Some((&resources, stopped))
        )
        .await
        .unwrap_err(),
        "Server stopped",
    );
    host.disconnect(generation);
}

#[tokio::test]
async fn ordinary_files_and_file_uris_use_the_shared_parser_without_resources() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("song+name.mp3");
    std::fs::write(&path, id3()).unwrap();
    assert_eq!(
        read(path.to_string_lossy().into_owned(), None)
            .await
            .unwrap(),
        "lyrics"
    );
    assert_eq!(
        read(format!("file://{}", path.display()), None)
            .await
            .unwrap(),
        "lyrics"
    );
    assert_eq!(
        read(
            format!("file://{}", path.display()).replace('+', "%2B"),
            None
        )
        .await
        .unwrap(),
        "lyrics"
    );
}

#[tokio::test]
async fn sdk_bytes_cross_one_resource_stream_and_use_the_same_parser() {
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let resources = Arc::new(NativeResources::new(host.clone()));
    let (_stop, stopped) = watch::channel(false);
    let task = {
        let resources = resources.clone();
        tokio::spawn(async move {
            read_for_playback(
                "content://fixture/audio".into(),
                Some((resources.as_ref(), stopped)),
            )
            .await
        })
    };
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "fileResourceStream");
    let stream = resources
        .attach_native(request["params"]["id"].as_str().unwrap(), generation)
        .unwrap();
    host.reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    let metadata = stream.next_request().await.unwrap().unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&metadata).unwrap(),
        json!({"path":"content://fixture/audio"})
    );
    let bytes = id3();
    for chunk in bytes.chunks(3) {
        stream.write(chunk.to_vec()).await.unwrap();
    }
    let _ = stream.finish().await;
    assert_eq!(task.await.unwrap().unwrap(), "lyrics");
    assert!(requests.try_recv().is_err());
    host.disconnect(generation);
}

#[tokio::test]
async fn stream_failures_are_errors_and_request_cancellation_stops_the_sdk_stream() {
    for cancel in [false, true] {
        let host = Arc::new(Host::default());
        let (generation, mut requests) = host.connect();
        let resources = Arc::new(NativeResources::new(host.clone()));
        let (_stop, stopped) = watch::channel(false);
        let task = {
            let resources = resources.clone();
            tokio::spawn(async move {
                read(
                    "content://fixture/audio".into(),
                    Some((resources.as_ref(), stopped)),
                )
                .await
            })
        };
        let request = requests.recv().await.unwrap();
        let stream = resources
            .attach_native(request["params"]["id"].as_str().unwrap(), generation)
            .unwrap();
        host.reply(generation, json!({"id":request["id"],"result":null}))
            .unwrap();
        stream.next_request().await.unwrap().unwrap();
        if cancel {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            assert!(stream.write(vec![1]).await.is_err());
        } else {
            stream.fail().await.unwrap();
            assert!(task.await.unwrap().is_err());
        }
        host.disconnect(generation);
    }
}
