use super::*;
use plain_rs::host::resource_stream::NativeResourceFile;
use plain_rs::inference::embedding::{ImageEncoder, ImageSource};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Encoder {
    decode: bool,
    fail: bool,
    released: Arc<AtomicUsize>,
}
impl ImageEncoder for Encoder {
    fn release(&self) {
        self.released.fetch_add(1, Ordering::SeqCst);
    }
    fn image_reader(&self, source: &mut dyn ImageSource) -> Result<Option<Vec<f32>>, String> {
        let mut bytes = Vec::new();
        source.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"raw-image");
        Ok((!self.decode).then_some(vec![1.0]))
    }
    fn image(&self, path: &std::path::Path) -> Result<Option<Vec<f32>>, String> {
        assert!(self.decode);
        assert_eq!(std::fs::read(path).unwrap(), b"decoded-image");
        if self.fail {
            Err("encoder failure".into())
        } else {
            Ok(Some(vec![2.0]))
        }
    }
}

async fn resource_image(decode: bool, fail: bool, available: bool) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("photo");
    std::fs::write(&source, b"outsideraw-imageoutside").unwrap();
    let cache = directory.path().join("cache");
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let resources = Arc::new(plain_rs::host::resources::NativeResources::new(
        host.clone(),
    ));
    let released = Arc::new(AtomicUsize::new(0));
    let encoder = Arc::new(Encoder {
        decode,
        fail,
        released: released.clone(),
    });
    let mut provider = ImageProvider::new(
        host.clone(),
        resources.clone(),
        cache.clone(),
        Some(encoder.clone()),
        Arc::new(plain_rs::db::Db::open(std::path::Path::new(":memory:")).unwrap()),
        true,
    );
    let decoded = Arc::new(AtomicUsize::new(0));
    let decode_count = decoded.clone();
    let worker_cache = cache.clone();
    let worker = tokio::task::spawn_blocking(move || {
        let item = Image {
            id: "photo".into(),
            path: "ph://photo".into(),
        };
        let result = if decode {
            provider.encode_with(encoder, &item, |source, output| {
                decode_count.fetch_add(1, Ordering::SeqCst);
                assert_eq!(std::fs::read(source).unwrap(), b"raw-image");
                assert!(source.starts_with(&worker_cache));
                assert!(output.starts_with(&worker_cache));
                if available {
                    std::fs::write(output, b"decoded-image").unwrap();
                }
                Ok(Value::Bool(available))
            })
        } else {
            provider.encode(encoder, &item)
        };
        provider.finish();
        result
    });
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "fileResourceStream");
    let stream = resources
        .attach_native(request["params"]["id"].as_str().unwrap(), generation)
        .unwrap();
    host.reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    let metadata: Value =
        serde_json::from_str(&stream.next_request().await.unwrap().unwrap()).unwrap();
    assert_eq!(metadata, json!({"path":"ph://photo","file":true}));
    stream
        .file(NativeResourceFile::new(std::fs::File::open(source).unwrap(), 7, 9).unwrap())
        .await
        .unwrap();
    let result = worker.await.unwrap();
    if fail && available {
        assert!(result.unwrap_err().to_string().contains("encoder failure"));
    } else {
        assert_eq!(
            result.unwrap(),
            available.then_some(if decode { vec![2.0] } else { vec![1.0] })
        );
    }
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert_eq!(decoded.load(Ordering::SeqCst), usize::from(decode));
    assert!(requests.try_recv().is_err());
    if decode {
        assert_eq!(std::fs::read_dir(cache).unwrap().count(), 0);
    } else {
        assert!(!cache.exists());
    }
}

#[tokio::test]
async fn raw_photo_handle_uses_rust_encoder_without_sdk_decode_or_temporary_output() {
    resource_image(false, false, true).await;
}

#[tokio::test]
async fn codec_fallback_uses_generic_primitive_and_rust_releases_all_staged_files() {
    for (fail, available) in [(false, true), (true, true), (false, false)] {
        resource_image(true, fail, available).await;
    }
}
