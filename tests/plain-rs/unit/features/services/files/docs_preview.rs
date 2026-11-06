use super::*;

#[test]
fn converts_office_bytes_to_markdown_without_an_external_tool() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/preview.docx");
    let original = std::fs::read(&path).unwrap();
    let markdown = convert(&path).unwrap();
    assert!(markdown.contains("# Document preview"), "{markdown}");
    assert!(markdown.contains("中文 & Markdown"), "{markdown}");
    assert!(markdown.contains("| Item | Value |"), "{markdown}");
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn rejects_corrupt_and_missing_input() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("broken.doc");
    assert!(convert(&path).is_err());
    std::fs::write(&path, b"not an office document").unwrap();
    assert!(convert(&path).is_err());
}

#[tokio::test]
async fn resource_uris_use_the_native_stream_and_propagate_failures() {
    use serde_json::json;
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let (generation, mut requests) = state.host.connect();
    for (path, fail) in [
        ("content://documents/preview", false),
        ("ph://documents/preview", false),
        ("content://documents/broken", true),
    ] {
        let result = {
            let state = state.clone();
            tokio::spawn(async move { read(&state, path).await })
        };
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "fileResourceStream");
        let stream = state
            .resources
            .attach_native(request["params"]["id"].as_str().unwrap(), generation)
            .unwrap();
        state
            .host
            .reply(generation, json!({"id":request["id"],"result":null}))
            .unwrap();
        let metadata: serde_json::Value =
            serde_json::from_str(&stream.next_request().await.unwrap().unwrap()).unwrap();
        assert_eq!(metadata["path"], path);
        if fail {
            stream.fail().await.unwrap();
            assert!(result.await.unwrap().is_err());
        } else {
            stream
                .write(
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/testdata/preview.docx"
                    ))
                    .to_vec(),
                )
                .await
                .unwrap();
            stream.finish().await.unwrap();
            assert!(
                result
                    .await
                    .unwrap()
                    .unwrap()
                    .contains("# Document preview")
            );
        }
    }
    core.stop_core().await;
}
