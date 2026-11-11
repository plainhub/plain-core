use super::*;
use crate::native::resources::{MAX_RESOURCE_READ_BYTES, NativeResourceRegistry};
use plain_rs::infrastructure::{
    db::chat_store::{SaveMode, peers},
    host::request_scope::NativeRequestScope,
};

#[path = "../native/uploads/fixture.rs"]
mod fixture;
use fixture::Fixture;

#[tokio::test]
async fn native_peer_resources_use_pinned_head_and_exact_bounded_ranges_without_a_local_listener() {
    let fixture = Fixture::new().await;
    assert_eq!(fixture.local.port, 0);
    let path = fixture.root.path().join("resource.bin");
    let bytes: Vec<u8> = (0..MAX_RESOURCE_READ_BYTES * 4 + 17)
        .map(|index| (index % 251) as u8)
        .collect();
    std::fs::write(&path, &bytes).unwrap();
    let registry = NativeResourceRegistry::default();
    let scope = NativeRequestScope::default();
    let resources = fixture.local.native_resources().unwrap();
    let info = resources
        .open_peer(&registry, &scope, "remote", path.to_str().unwrap(), None)
        .await
        .unwrap();
    assert_eq!(info.size, bytes.len() as u64);
    assert_eq!(info.mime, "application/octet-stream");
    let resource = registry.get(&scope, &info.id).unwrap();
    for (offset, length) in [
        (0, 0),
        (7, MAX_RESOURCE_READ_BYTES * 2 + 3),
        (bytes.len() - 1, 1),
    ] {
        assert_eq!(
            resource
                .read_range(offset as u64, length as u64)
                .await
                .unwrap(),
            &bytes[offset..offset + length]
        );
    }
    assert!(
        resource
            .read_range(0, MAX_RESOURCE_RESPONSE_BYTES + 1)
            .await
            .is_err()
    );
    assert!(resource.read_range(bytes.len() as u64, 1).await.is_err());
    assert!(
        registry
            .get(&NativeRequestScope::default(), &info.id)
            .is_err()
    );
    registry.close_owner(scope.id());
    assert!(resource.read_range(0, 1).await.is_err());
    fixture.stop().await;
}

#[tokio::test]
async fn native_peer_resources_reject_permission_loss_and_paired_identity_revocation() {
    let fixture = Fixture::new().await;
    let path = fixture.root.path().join("resource.bin");
    std::fs::write(&path, b"bytes").unwrap();
    let registry = NativeResourceRegistry::default();
    let scope = NativeRequestScope::default();
    let resources = fixture.local.native_resources().unwrap();
    let info = resources
        .open_peer(&registry, &scope, "remote", path.to_str().unwrap(), None)
        .await
        .unwrap();
    let resource = registry.get(&scope, &info.id).unwrap();
    fixture
        .remote
        .runtime_state()
        .prefs
        .set("remote_access", false)
        .unwrap();
    assert!(resource.read_range(0, 5).await.is_err());
    fixture
        .remote
        .runtime_state()
        .prefs
        .set("remote_access", true)
        .unwrap();
    assert_eq!(resource.read_range(0, 5).await.unwrap(), b"bytes");
    let mut peer = fixture.peer.clone();
    peer.public_key = plain_libs::utils::base64::base64_encode(&[99; 32]);
    peers::save(&fixture.local.runtime_state().db, &[peer], SaveMode::Upsert).unwrap();
    assert!(resource.read_range(0, 5).await.is_err());
    assert!(resource.read_range(0, 0).await.is_err());
    fixture.stop().await;
}

#[tokio::test]
async fn native_peer_resources_reject_changed_file_length_without_exposing_partial_bytes() {
    let fixture = Fixture::new().await;
    let path = fixture.root.path().join("resource.bin");
    std::fs::write(&path, b"initial").unwrap();
    let registry = NativeResourceRegistry::default();
    let scope = NativeRequestScope::default();
    let info = fixture
        .local
        .native_resources()
        .unwrap()
        .open_peer(&registry, &scope, "remote", path.to_str().unwrap(), None)
        .await
        .unwrap();
    std::fs::write(&path, b"replaced-content").unwrap();
    assert!(
        registry
            .get(&scope, &info.id)
            .unwrap()
            .read_range(0, info.size)
            .await
            .is_err()
    );
    fixture.stop().await;
}

#[tokio::test]
async fn peer_exports_stream_large_files_to_local_collision_safe_destinations() {
    use crate::native::resources::ResourceDestination;
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    let fixture = Fixture::new().await;
    let source = fixture.root.path().join("large-source.bin");
    let chunk: Vec<u8> = (0..65536).map(|index| ((index * 17) % 251) as u8).collect();
    let mut file = std::fs::File::create(&source).unwrap();
    for _ in 0..513 {
        file.write_all(&chunk).unwrap();
    }
    drop(file);
    let destination = fixture.root.path().join("downloads");
    std::fs::create_dir(&destination).unwrap();
    let occupied = destination.join("large.bin");
    std::fs::write(&occupied, b"existing file").unwrap();
    let scope = NativeRequestScope::default();
    let path = fixture
        .local
        .native_resources()
        .unwrap()
        .export(
            &scope,
            Some("remote"),
            source.to_str().unwrap(),
            ResourceDestination::Directory {
                directory: destination.to_string_lossy().into_owned(),
                name: "large.bin".into(),
            },
        )
        .await
        .unwrap();
    assert_ne!(std::path::Path::new(&path), occupied);
    assert_eq!(std::fs::read(&occupied).unwrap(), b"existing file");
    assert!(std::fs::metadata(&path).unwrap().len() > MAX_RESOURCE_RESPONSE_BYTES);
    let digest = |path: &std::path::Path| {
        let mut file = std::fs::File::open(path).unwrap();
        let mut digest = Sha256::new();
        let mut buffer = [0; 65536];
        loop {
            let count = file.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
        }
        digest.finalize()
    };
    assert_eq!(digest(&source), digest(std::path::Path::new(&path)));
    assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 2);
    scope.close();
    assert!(
        fixture
            .local
            .native_resources()
            .unwrap()
            .export(
                &scope,
                Some("remote"),
                source.to_str().unwrap(),
                ResourceDestination::File(destination.join("cancelled.bin"))
            )
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 2);
    fixture.stop().await;
}

#[tokio::test]
async fn peer_resources_reject_a_wrong_tls_pin_without_switching_transport() {
    let fixture = Fixture::new().await;
    let state = fixture.local.runtime_state();
    let route = state
        .managed_proxy
        .peers
        .route(&state, "remote")
        .await
        .unwrap();
    let bad = Arc::new(PeerRoute {
        peer: route.peer.clone(),
        base: route.base.clone(),
        client_id: route.client_id.clone(),
        key: route.key.clone(),
        url_key: route.url_key.clone(),
        pin: [0; 32],
        client: http_client::builder()
            .no_proxy()
            .https_only(true)
            .pin_certificate([0; 32])
            .redirect(http_client::redirect::Policy::none())
            .build()
            .unwrap(),
        supports_managed_resources: true,
        expires: route.expires,
    });
    let slot = state
        .managed_proxy
        .peers
        .routes
        .lock()
        .unwrap()
        .get("remote")
        .unwrap()
        .clone();
    *slot.lock().await = Some(bad);
    let path = fixture.root.path().join("resource.bin");
    std::fs::write(&path, b"bytes").unwrap();
    assert!(
        PeerResource::open(state, "remote", path.to_str().unwrap(), None)
            .await
            .is_err()
    );
    fixture.stop().await;
}

#[tokio::test]
async fn peer_generated_thumbnails_preserve_seekable_bytes_when_http_uses_a_full_body() {
    let fixture = Fixture::new().await;
    let path = fixture.root.path().join("image.png");
    plain_libs::image::RgbImage::from_fn(800, 600, |x, y| {
        plain_libs::image::Rgb([(x % 251) as u8, (y % 251) as u8, 20])
    })
    .save(&path)
    .unwrap();
    let registry = NativeResourceRegistry::default();
    let scope = NativeRequestScope::default();
    let info = fixture
        .local
        .native_resources()
        .unwrap()
        .open_peer(
            &registry,
            &scope,
            "remote",
            path.to_str().unwrap(),
            Some(Size::Small),
        )
        .await
        .unwrap();
    let resource = registry.get(&scope, &info.id).unwrap();
    let bytes = resource.read_range(0, info.size).await.unwrap();
    assert_eq!(resource.read_range(3, 11).await.unwrap(), bytes[3..14]);
    let decoded = plain_libs::image::load_from_memory(&bytes).unwrap();
    assert!(decoded.width() <= 256 && decoded.height() <= 256);
    let prepared = fixture
        .local
        .runtime_state()
        .cache
        .root()
        .join("native_resources");
    assert_eq!(std::fs::read_dir(&prepared).unwrap().count(), 1);
    registry.close_owner(scope.id());
    assert_eq!(std::fs::read_dir(&prepared).unwrap().count(), 0);
    assert!(resource.read_range(0, 1).await.is_err());
    fixture.stop().await;
}
