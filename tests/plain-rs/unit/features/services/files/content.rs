use super::*;

#[tokio::test]
async fn photo_resource_uris_use_the_sdk_resource_stream_instead_of_filesystem_reads() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let path = "ph://asset/L0/001";
    links::authorize_path(&state, path).unwrap();
    let result = read(
        &state,
        links::File {
            path: path.into(),
            media_id: "photos:asset/L0/001".into(),
            name: "photo.png".into(),
        },
        Options {
            access_order: 0,
            action: Action::Download,
            size: None,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            starts_view: false,
        },
    )
    .await
    .unwrap();
    assert!(
        matches!(result, Content::Resource { path: resource, download: true, name: Some(name) } if resource == path && name == "photo.png")
    );
    core.stop_core().await;
}

#[tokio::test]
async fn managed_audio_thumbnail_returns_only_embedded_cover_bytes() {
    use crate::infrastructure::host::resource_stream::NativeResourceFile;

    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let (generation, mut requests) = state.host.connect();
    let path = "content://media/external/audio/media/42";
    for fixture in ["audio-cover.mp3", "audio-untagged.mp3"] {
        let host_state = state.clone();
        let result = tokio::spawn(async move {
            read(
                &host_state,
                links::File {
                    path: path.into(),
                    media_id: String::new(),
                    name: "song.mp3".into(),
                },
                Options {
                    access_order: 0,
                    action: Action::Preview,
                    size: Some(Size::Large),
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    starts_view: false,
                },
            )
            .await
            .unwrap()
        });
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "systemFileResource");
        assert_eq!(request["params"]["operation"], "mime");
        state
            .host
            .reply(
                generation,
                json!({"id":request["id"],"result":"audio/mpeg"}),
            )
            .unwrap();
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
        assert_eq!(metadata, json!({"path":path,"file":true}));
        let file = std::fs::File::open(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("testdata")
                .join(fixture),
        )
        .unwrap();
        stream
            .file(NativeResourceFile::new(file, 0, -1).unwrap())
            .await
            .unwrap();
        match result.await.unwrap() {
            Content::Thumbnail {
                bytes: Ok(Some(bytes)),
                ..
            } if fixture == "audio-cover.mp3" => assert_eq!(
                bytes.as_ref(),
                include_bytes!("../../../../../../plain-rs/testdata/v-prog-gray.jpg")
            ),
            Content::NotFound if fixture == "audio-untagged.mp3" => {}
            _ => panic!("unexpected audio preview for {fixture}"),
        }
    }
    core.stop_core().await;
}
