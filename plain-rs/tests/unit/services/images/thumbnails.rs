use crate::test_runtime::Core as ContentServer;
use plain_rs::services::images::thumbnails::Request;
use sha2::Digest;
use std::sync::Arc;

#[tokio::test]
async fn native_thumbnail_output_is_one_shot_and_validates_before_consuming() {
    let thumbnails = super::Thumbnails::default();
    let (sender, mut receiver) = tokio::sync::oneshot::channel();
    thumbnails
        .outputs
        .lock()
        .unwrap()
        .insert("fixture".into(), sender);
    assert!(thumbnails.submit("fixture", Vec::new()).is_err());
    assert!(
        thumbnails
            .submit("fixture", vec![0; 32 * 1024 * 1024 + 1])
            .is_err()
    );
    thumbnails.submit("fixture", vec![0, 255, 1]).unwrap();
    assert_eq!(receiver.try_recv().unwrap(), vec![0, 255, 1]);
    assert!(thumbnails.submit("fixture", vec![1]).is_err());
    assert!(thumbnails.submit("expired", vec![1]).is_err());
}

#[test]
fn cancelled_thumbnail_output_discards_its_token() {
    let thumbnails = super::Thumbnails::default();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    thumbnails
        .outputs
        .lock()
        .unwrap()
        .insert("cancelled".into(), sender);
    let output = super::Output {
        service: &thumbnails,
        token: "cancelled".into(),
    };
    drop(output);
    assert!(thumbnails.submit("cancelled", vec![1]).is_err());
    assert!(receiver.blocking_recv().is_err());
}

fn request_for(path: &std::path::Path) -> Request {
    Request {
        path: path.to_str().unwrap().to_owned(),
        width: 80,
        height: 60,
        center_crop: true,
        file_id: String::new(),
        file_name: String::new(),
    }
}

fn server_with_cache() -> (tempfile::TempDir, ContentServer, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", "thumb-fixture").unwrap();
    let cache = dir.path().join("cache");
    let core = ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[4; 32]),
        prefs,
    )
    .unwrap();
    (dir, core, cache)
}

#[tokio::test]
async fn sdk_decode_validates_receipts_and_cleans_every_output_token() {
    for (receipt, output, expected) in [
        (
            serde_json::json!(true),
            Some(vec![0, 255, 1]),
            Ok(Some(vec![0, 255, 1])),
        ),
        (serde_json::json!(null), None, Ok(None)),
        (
            serde_json::json!(false),
            None,
            Err("invalid thumbnail receipt"),
        ),
        (
            serde_json::json!(true),
            None,
            Err("missing thumbnail output"),
        ),
    ] {
        let (_dir, core, _cache) = server_with_cache();
        let state = core.runtime_state();
        let (generation, mut requests) = state.host.connect();
        let owned = state.clone();
        let worker = tokio::spawn(async move {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "thumbnailDecode");
            assert_eq!(request["params"]["mediaId"], "sdk-id");
            assert_eq!(request["params"]["width"], 80);
            let token = request["params"]["outputToken"].as_str().unwrap();
            if let Some(bytes) = output {
                owned.thumbnails.submit(token, bytes).unwrap();
            }
            owned
                .host
                .reply(
                    generation,
                    serde_json::json!({"id":request["id"],"result":receipt}),
                )
                .unwrap();
        });
        let mut request = request_for(std::path::Path::new("/image.jpg"));
        request.file_id = "sdk-id".into();
        let actual = super::decode_sdk(&state, &request, true).await;
        assert_eq!(
            actual
                .as_ref()
                .map(|value| value.clone())
                .map_err(String::as_str),
            expected
        );
        worker.await.unwrap();
        assert!(state.thumbnails.outputs.lock().unwrap().is_empty());
        core.stop_core().await;
    }
}

#[tokio::test]
async fn cancelling_a_pending_sdk_decode_removes_its_token() {
    let (_dir, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let owned = state.clone();
    let call = tokio::spawn(async move {
        super::decode_sdk(
            &owned,
            &request_for(std::path::Path::new("/image.jpg")),
            false,
        )
        .await
    });
    let request = requests.recv().await.unwrap();
    let token = request["params"]["outputToken"].as_str().unwrap();
    call.abort();
    assert!(call.await.unwrap_err().is_cancelled());
    assert!(state.thumbnails.outputs.lock().unwrap().is_empty());
    assert!(state.thumbnails.submit(token, vec![1]).is_err());
    core.stop_core().await;
}
