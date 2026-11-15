use super::*;

#[test]
fn converts_office_bytes_to_markdown_without_an_external_tool() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/preview.docx");
    let original = std::fs::read(&path).unwrap();
    let markdown = convert(&path, Format::Office).unwrap();
    assert!(markdown.contains("# Document preview"), "{markdown}");
    assert!(markdown.contains("中文 & Markdown"), "{markdown}");
    assert!(markdown.contains("| Item | Value |"), "{markdown}");
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn rejects_corrupt_and_missing_input() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("broken.doc");
    assert!(convert(&path, Format::Office).is_err());
    std::fs::write(&path, b"not an office document").unwrap();
    assert!(convert(&path, Format::Office).is_err());
}

#[tokio::test]
async fn resource_uris_use_the_native_stream_and_propagate_failures() {
    use plain_libs::serde_json::json;
    let directory = plain_libs::temporary::tempdir().unwrap();
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
            tokio::spawn(async move { read(&state, path, Format::Office).await })
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
        let metadata: plain_libs::serde_json::Value =
            plain_libs::serde_json::from_str(&stream.next_request().await.unwrap().unwrap())
                .unwrap();
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

#[test]
fn previews_csv_records_as_a_markdown_table_without_changing_file_bytes() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("report.CSV");
    let original =
        "\u{feff}Name,Note\r\n\"Kite, blue\",\"Two \"\"quotes\"\"\r\nnext | <script>\"\r\nA,";
    std::fs::write(&path, original).unwrap();
    let format = Format::from_name("report.CSV").unwrap();
    let markdown = convert(&path, format).unwrap();
    assert!(markdown.contains("| Name | Note |"), "{markdown}");
    assert!(
        markdown.contains("| Kite, blue | Two \"quotes\" next \\| \\<script> |"),
        "{markdown}"
    );
    assert!(markdown.contains("| A |  |"), "{markdown}");
    assert_eq!(std::fs::read(&path).unwrap(), original.as_bytes());
}

#[tokio::test]
async fn csv_preview_does_not_replace_file_content_for_read_or_download() {
    use super::super::{content, links};
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let path = directory.path().join("report.csv");
    let source = b"Name,Amount\nKite,12\n";
    std::fs::write(&path, source).unwrap();
    for action in [
        content::Action::Preview,
        content::Action::Describe,
        content::Action::Download,
    ] {
        let result = content::read(
            &state,
            links::File {
                path: path.to_string_lossy().into_owned(),
                media_id: String::new(),
                name: "report.csv".into(),
            },
            content::Options {
                access_order: 0,
                action,
                size: None,
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                starts_view: false,
            },
        )
        .await
        .unwrap();
        match result {
            content::Content::Markdown { text, name } if action == content::Action::Preview => {
                assert_eq!(name, "report.csv");
                assert!(text.contains("| Kite | 12 |"), "{text}");
            }
            content::Content::File { path, download, .. } if action != content::Action::Preview => {
                assert_eq!(download, action == content::Action::Download);
                assert_eq!(std::fs::read(path).unwrap(), source);
            }
            _ => panic!("Unexpected response for {action:?}"),
        }
    }
    core.stop_core().await;
}

#[test]
fn rejects_bad_or_oversized_csv_previews() {
    for invalid in [
        &b"Name,Note\nX,\"unclosed"[..],
        &b"\"quoted\"junk,B"[..],
        &b"Name,\xff"[..],
    ] {
        assert!(undoc::parse_csv(invalid).is_err());
    }
    assert!(undoc::parse_csv(&vec![b'a'; undoc::limits::MAX_INPUT_BYTES + 1]).is_err());
    let wide = ",".repeat(10_000) + "\n" + &"a\n".repeat(100);
    assert!(undoc::parse_csv(wide.as_bytes()).is_err());
    assert!(matches!(
        Format::from_mime("text/csv; charset=UTF-8"),
        Some(Format::Csv)
    ));
    assert!(Format::from_name("report.csv.txt").is_none());
}
