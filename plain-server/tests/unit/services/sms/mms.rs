use super::*;
fn pending() -> Pending {
    Pending {
        id: "pending".into(),
        number: "+12025550123".into(),
        body: "Hello".into(),
        thread_id: "42".into(),
        attachments: vec![Attachment {
            path: "a.jpg".into(),
            content_type: "image/jpeg".into(),
            name: "a.jpg".into(),
        }],
        launch_time_sec: 100,
        minimum_id: 10,
        created_at: String::new(),
    }
}
fn candidate(id: i64, address: &str, body: &str, thread: &str, types: &[&str]) -> Candidate {
    Candidate {
        id,
        address_candidates: vec![crate::services::sms::addresses::MmsAddress {
            value: address.into(),
            is_sender: false,
        }],
        parts: std::iter::once(super::super::parts::Part {
            path: "text".into(),
            content_type: "text/plain".into(),
            name: String::new(),
            file_name: String::new(),
            has_data: false,
            text: body.into(),
        })
        .chain(types.iter().map(|mime| super::super::parts::Part {
            path: "attachment".into(),
            content_type: (*mime).into(),
            name: String::new(),
            file_name: String::new(),
            has_data: true,
            text: String::new(),
        }))
        .collect(),
        thread_id: thread.into(),
    }
}
#[test]
fn provider_matching_requires_recipient_body_thread_and_attachment_families() {
    let candidates = vec![
        candidate(
            11,
            "+12025550123",
            "Hello",
            "42",
            &["image/png; charset=utf-8", "application/smil"],
        ),
        candidate(12, "+12025550124", "Hello", "42", &["image/jpeg"]),
        candidate(13, "+12025550123", "Other", "42", &["image/jpeg"]),
        candidate(14, "+12025550123", "Hello", "43", &["image/jpeg"]),
        candidate(15, "+12025550123", "Hello", "42", &["video/mp4"]),
        candidate(
            16,
            "+12025550123",
            "Hello",
            "42",
            &["image/jpeg", "image/jpeg"],
        ),
    ];
    assert_eq!(matching(&pending(), candidates), vec![11]);
}
#[test]
fn attachment_normalization_preserves_count_and_ignores_smil() {
    assert_eq!(
        normalized_types(
            ["IMAGE/JPEG; charset=utf-8", "application/smil", "image/png"].into_iter()
        ),
        vec!["image/*", "image/*"]
    );
    assert_ne!(
        normalized_types(["image/jpeg"].into_iter()),
        normalized_types(["image/jpeg", "image/png"].into_iter())
    );
}

fn runtime(directory: &std::path::Path, host: Arc<Host>) -> Arc<Runtime> {
    Runtime::new(
        host,
        Arc::new(Db::open(&directory.join("plain.db")).unwrap()),
        directory.to_path_buf(),
        Arc::new(plain_rs::kv::LazyDb::new(directory.join("plain.db"))),
        broadcast::channel(16).0,
    )
}

#[tokio::test]
async fn provider_attachments_use_one_mime_primitive_and_preserve_stream_bytes() {
    for mime in [json!(" image/png "), json!(""), Value::Null] {
        let directory = tempfile::tempdir().unwrap();
        let host = Arc::new(Host::default());
        let (generation, mut requests) = host.connect();
        let resources = Arc::new(crate::host::resources::NativeResources::new(host.clone()));
        let (_stop, stopped) = watch::channel(false);
        let runtime = runtime(directory.path(), host.clone());
        runtime.set_resources(resources.clone(), stopped);
        let destination = directory.path().join("attachment");
        let copying = {
            let destination = destination.clone();
            tokio::spawn(async move {
                runtime
                    .copy_attachment("content://fixture/123", &destination)
                    .await
            })
        };
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "systemFileResource");
        assert_eq!(request["params"]["operation"], "mime");
        assert_eq!(request["params"]["path"], "content://fixture/123");
        let expected = if mime == json!(" image/png ") {
            "image/png"
        } else {
            "application/octet-stream"
        };
        host.reply(generation, json!({"id":request["id"],"result":mime}))
            .unwrap();
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "fileResourceStream");
        let stream = resources
            .attach_native(request["params"]["id"].as_str().unwrap(), generation)
            .unwrap();
        host.reply(generation, json!({"id":request["id"],"result":null}))
            .unwrap();
        let metadata: Value =
            serde_json::from_str(&stream.next_request().await.unwrap().unwrap()).unwrap();
        assert_eq!(metadata, json!({"path":"content://fixture/123"}));
        stream.write(vec![0, 1, 2, 255]).await.unwrap();
        stream.write(vec![3, 0, 254]).await.unwrap();
        stream.finish().await.unwrap();
        assert_eq!(copying.await.unwrap().unwrap(), expected);
        assert_eq!(
            std::fs::read(destination).unwrap(),
            vec![0, 1, 2, 255, 3, 0, 254]
        );
        assert!(requests.try_recv().is_err());
    }
}

#[tokio::test]
async fn failed_or_invalid_mime_facts_do_not_open_or_copy_provider_attachments() {
    for reply in [
        json!({"error":"Provider permission denied"}),
        json!({"result":42}),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let host = Arc::new(Host::default());
        let (generation, mut requests) = host.connect();
        let resources = Arc::new(crate::host::resources::NativeResources::new(host.clone()));
        let (_stop, stopped) = watch::channel(false);
        let runtime = runtime(directory.path(), host.clone());
        runtime.set_resources(resources, stopped);
        let destination = directory.path().join("attachment");
        let copying = {
            let destination = destination.clone();
            tokio::spawn(async move {
                runtime
                    .copy_attachment("content://fixture/123", &destination)
                    .await
            })
        };
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "systemFileResource");
        let mut reply = reply;
        reply["id"] = request["id"].clone();
        host.reply(generation, reply).unwrap();
        assert!(copying.await.unwrap().is_err());
        assert!(!destination.exists());
        assert!(requests.try_recv().is_err());
    }
}

#[tokio::test]
async fn local_attachments_copy_directly_without_any_host_request() {
    let directory = tempfile::tempdir().unwrap();
    let host = Arc::new(Host::default());
    let (_generation, mut requests) = host.connect();
    let runtime = runtime(directory.path(), host);
    let source = directory.path().join("original.PNG");
    let destination = directory.path().join("attachment");
    std::fs::write(&source, [0, 255, 1, 2]).unwrap();
    assert_eq!(
        runtime
            .copy_attachment(source.to_str().unwrap(), &destination)
            .await
            .unwrap(),
        "image/png"
    );
    assert_eq!(
        std::fs::read(&source).unwrap(),
        std::fs::read(destination).unwrap()
    );
    assert!(requests.try_recv().is_err());
}
