use super::*;

async fn open() -> (
    Arc<NativeResources>,
    NativeResourceReader,
    NativeResourceStream,
    watch::Sender<bool>,
    u64,
) {
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let resources = Arc::new(NativeResources::new(host.clone()));
    let (stop, stopped) = watch::channel(false);
    let opening = {
        let resources = resources.clone();
        tokio::spawn(async move {
            resources
                .read_native("content://fixture/document", stopped)
                .await
        })
    };
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "fileResourceStream");
    let id = request["params"]["id"].as_str().unwrap();
    assert!(resources.attach_native(id, generation + 1).is_err());
    let stream = resources.attach_native(id, generation).unwrap();
    assert!(resources.attach_native(id, generation).is_err());
    host.reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    let reader = opening.await.unwrap().unwrap();
    let metadata: Value =
        serde_json::from_str(&stream.next_request().await.unwrap().unwrap()).unwrap();
    assert_eq!(metadata, json!({"path":"content://fixture/document"}));
    (resources, reader, stream, stop, generation)
}

#[tokio::test]
async fn native_stream_preserves_bytes_and_has_one_terminal_receipt() {
    let (_resources, mut reader, stream, _stop, _generation) = open().await;
    assert!(stream.write(Vec::new()).await.is_err());
    assert!(stream.write(vec![0; 64 * 1024 + 1]).await.is_err());
    let bytes = (0..64 * 1024)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    stream.write(bytes.clone()).await.unwrap();
    assert_eq!(reader.read().await.unwrap(), Some(bytes));
    stream.finish().await.unwrap();
    assert!(stream.finish().await.is_err());
    assert!(stream.write(vec![1]).await.is_err());
    assert_eq!(reader.read().await.unwrap(), None);
    assert_eq!(reader.read().await.unwrap(), None);
}

#[tokio::test]
async fn native_stream_failure_reaches_the_reader() {
    let (_resources, mut reader, stream, _stop, _generation) = open().await;
    stream.fail().await.unwrap();
    assert!(reader.read().await.is_err());
    assert_eq!(reader.read().await.unwrap(), None);
    assert!(stream.finish().await.is_err());
}

#[tokio::test]
async fn cancellation_closes_both_directions() {
    let (_resources, mut reader, stream, stop, _generation) = open().await;
    stop.send_replace(true);
    assert!(reader.read().await.is_err());
    assert!(stream.write(vec![1]).await.is_err());
    assert!(stream.next_request().await.is_err());
}

#[tokio::test]
async fn replacing_the_host_rejects_old_resource_handles() {
    let (resources, mut reader, stream, _stop, generation) = open().await;
    let (replacement, _requests) = resources.host.connect();
    assert_ne!(generation, replacement);
    assert!(reader.read().await.is_err());
    assert!(stream.write(vec![1]).await.is_err());
    assert!(stream.next_request().await.is_err());
}

#[tokio::test]
async fn cancelling_before_attachment_releases_registration_and_capacity() {
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let resources = Arc::new(NativeResources::new(host.clone()));
    let (_stop, stopped) = watch::channel(false);
    let opening = {
        let resources = resources.clone();
        tokio::spawn(async move {
            resources
                .read_native("content://fixture/document", stopped)
                .await
        })
    };
    let request = requests.recv().await.unwrap();
    let id = request["params"]["id"].as_str().unwrap().to_owned();
    host.reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    opening.abort();
    let Err(error) = opening.await else {
        panic!("cancelled resource open completed")
    };
    assert!(error.is_cancelled());
    assert!(resources.attach_native(&id, generation).is_err());
    assert!(resources.pending.lock().unwrap().is_empty());
    assert_eq!(resources.capacity.available_permits(), 32);
}

#[tokio::test]
async fn file_descriptor_delivery_preserves_extent_and_owns_the_source() {
    use std::io::{Read, Seek, SeekFrom};
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let resources = Arc::new(NativeResources::new(host.clone()));
    let (_stop, stopped) = watch::channel(false);
    let opening = {
        let resources = resources.clone();
        tokio::spawn(async move {
            resources
                .read_file("content://fixture/asset", stopped)
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
    let metadata: Value =
        serde_json::from_str(&stream.next_request().await.unwrap().unwrap()).unwrap();
    assert_eq!(
        metadata,
        json!({"path":"content://fixture/asset","file":true})
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("asset");
    std::fs::write(&path, b"prefixPAYLOADsuffix").unwrap();
    let original = std::fs::File::open(&path).unwrap();
    stream
        .file(NativeResourceFile {
            file: original.try_clone().unwrap(),
            offset: 6,
            length: 7,
        })
        .await
        .unwrap();
    drop(original);
    let mut delivered = opening.await.unwrap().unwrap();
    assert_eq!(delivered.offset, 6);
    assert_eq!(delivered.length, 7);
    delivered
        .file
        .seek(SeekFrom::Start(delivered.offset as u64))
        .unwrap();
    let mut bytes = vec![0; delivered.length as usize];
    delivered.file.read_exact(&mut bytes).unwrap();
    assert_eq!(bytes, b"PAYLOAD");
    assert!(stream.write(vec![1]).await.is_err());
    assert!(stream.finish().await.is_err());
    assert!(requests.try_recv().is_err());
    assert!(resources.pending.lock().unwrap().is_empty());
    assert_eq!(resources.capacity.available_permits(), 32);
}

#[tokio::test]
async fn descriptor_range_rejection_keeps_the_stream_available_for_failure() {
    let (_resources, mut reader, stream, _stop, _generation) = open().await;
    for (offset, length) in [(-1, 1), (0, -2), (i64::MAX, 1)] {
        assert!(
            stream
                .file(NativeResourceFile {
                    file: tempfile::tempfile().unwrap(),
                    offset,
                    length
                })
                .await
                .is_err()
        );
    }
    stream.fail().await.unwrap();
    assert!(reader.read().await.is_err());
}

#[tokio::test]
async fn cancelling_descriptor_delivery_releases_capacity_without_a_second_request() {
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let resources = Arc::new(NativeResources::new(host.clone()));
    let (_stop, stopped) = watch::channel(false);
    let opening = {
        let resources = resources.clone();
        tokio::spawn(async move {
            resources
                .read_file("content://fixture/asset", stopped)
                .await
        })
    };
    let request = requests.recv().await.unwrap();
    let stream = resources
        .attach_native(request["params"]["id"].as_str().unwrap(), generation)
        .unwrap();
    host.reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    assert!(stream.next_request().await.unwrap().is_some());
    opening.abort();
    assert!(opening.await.err().unwrap().is_cancelled());
    assert!(
        stream
            .file(NativeResourceFile {
                file: tempfile::tempfile().unwrap(),
                offset: 0,
                length: 0
            })
            .await
            .is_err()
    );
    assert_eq!(resources.capacity.available_permits(), 32);
    assert!(resources.pending.lock().unwrap().is_empty());
    assert!(requests.try_recv().is_err());
}

#[test]
fn descriptor_unknown_length_is_resolved_without_reading_the_file() {
    use std::io::{Seek, SeekFrom};
    let mut file = tempfile::tempfile().unwrap();
    file.set_len(123).unwrap();
    file.seek(SeekFrom::Start(11)).unwrap();
    let mut resource = NativeResourceFile::new(file, 23, -1).unwrap();
    assert_eq!(resource.offset, 23);
    assert_eq!(resource.length, 100);
    assert_eq!(resource.file.stream_position().unwrap(), 11);
    for (offset, length) in [(-1, 1), (124, -1), (0, -2), (i64::MAX, 1)] {
        assert!(
            NativeResourceFile::new(resource.file.try_clone().unwrap(), offset, length).is_err()
        );
    }
}
