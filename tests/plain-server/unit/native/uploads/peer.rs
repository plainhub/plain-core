use super::*;
use crate::native::uploads::UploadWriters;
use plain_rs::infrastructure::host::request_scope::NativeRequestScope;

#[cfg(feature = "http_transport")]
use crate::native::uploads::fixture::Fixture;

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn scoped_native_peer_uploads_preserve_empty_small_and_multiple_window_bytes_over_tls() {
    let fixture = Fixture::new().await;
    assert_eq!(fixture.local.port, 0);
    let writers = UploadWriters::default();
    let scope = NativeRequestScope::default();
    let remote = fixture.remote.runtime_state();
    for size in [0, 17, MAX_UPLOAD_WRITE_BYTES * 5 + 31] {
        let id = format!("fixture-{size}");
        let bytes: Vec<u8> = (0..size)
            .map(|i| ((i * 31 + i / 256) % 256) as u8)
            .collect();
        let handle = writers
            .open_peer(
                &fixture.local.native_uploads().unwrap(),
                &scope,
                "remote",
                &id,
                0,
                size as u64,
            )
            .await
            .unwrap();
        for window in bytes.chunks(MAX_UPLOAD_WRITE_BYTES) {
            writers.write(&scope, &handle, window).await.unwrap();
        }
        assert_eq!(writers.finish(&scope, &handle).await.unwrap(), size as u64);
        assert_eq!(
            remote.upload_chunks.list(&id).await.unwrap(),
            vec![format!("0:{size}")]
        );
        let chunk = remote.upload_chunks.directory().join(&id).join("chunk_0");
        assert_eq!(std::fs::read(chunk).unwrap(), bytes);
        let destination = fixture.root.path().join(format!("result-{size}.bin"));
        let query = plain_graphql::Request::new("mutation Merge($id:String!,$path:String!,$size:Long!){mergeChunks(fileId:$id,totalChunks:1,path:$path,replace:true,totalSize:$size){status}}")
            .variables(plain_graphql::Variables::from_json(plain_libs::serde_json::json!({"id":id,"path":destination,"size":size})));
        let response = fixture
            .local
            .native_graphql()
            .unwrap()
            .execute_peer_scoped(
                "remote",
                query,
                scope.clone(),
                std::time::Duration::from_secs(30),
            )
            .await
            .unwrap();
        assert!(response.errors.is_empty(), "{response:?}");
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if std::fs::read(&destination).is_ok_and(|actual| actual == bytes) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    fixture.stop().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn peer_writer_bounds_requests_and_rechecks_pairing_and_session_before_writes_and_finish() {
    let fixture = Fixture::new().await;
    let writers = UploadWriters::default();
    let scope = NativeRequestScope::default();
    for (id, window) in [
        ("empty-write", vec![]),
        ("oversize", vec![0; MAX_UPLOAD_WRITE_BYTES + 1]),
        ("overflow", vec![0; 2]),
    ] {
        let handle = writers
            .open_peer(
                &fixture.local.native_uploads().unwrap(),
                &scope,
                "remote",
                id,
                0,
                1,
            )
            .await
            .unwrap();
        assert!(writers.write(&scope, &handle, &window).await.is_err());
        assert!(writers.finish(&scope, &handle).await.is_err());
    }
    let first = writers
        .open_peer(
            &fixture.local.native_uploads().unwrap(),
            &scope,
            "remote",
            "first",
            0,
            1,
        )
        .await
        .unwrap();
    let second = writers
        .open_peer(
            &fixture.local.native_uploads().unwrap(),
            &scope,
            "remote",
            "second",
            0,
            1,
        )
        .await
        .unwrap();
    writers.write(&scope, &second, b"b").await.unwrap();
    let mut peer = fixture.peer.clone();
    peer.token = plain_libs::crypto::gen_token();
    fixture.local.runtime_state().db.upsert_peer(&peer);
    assert!(
        writers
            .write(&scope, &first, b"a")
            .await
            .unwrap_err()
            .contains("revoked")
    );
    assert!(
        writers
            .finish(&scope, &second)
            .await
            .unwrap_err()
            .contains("revoked")
    );
    fixture.stop().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn peer_upload_finish_rejects_wrong_index_length_and_authorization_receipts() {
    let fixture = Fixture::new().await;
    for (status, body, expected) in [
        (StatusCode::CREATED, "1:0", "Upload receipt mismatch"),
        (StatusCode::CREATED, "0:1", "Upload receipt mismatch"),
        (StatusCode::CREATED, "invalid", "Invalid upload receipt"),
        (StatusCode::OK, "unexpected", "unexpected"),
        (StatusCode::UNAUTHORIZED, "private", "unauthorized"),
        (StatusCode::FORBIDDEN, "private", "remote_access_disabled"),
    ] {
        let mut chunk =
            PeerUploadChunk::open(fixture.local.runtime_state(), "remote", "fixture", 0, 0)
                .await
                .unwrap();
        chunk.request = Box::pin(async move { Ok((status, body.as_bytes().to_vec())) });
        assert_eq!(chunk.finish().await.unwrap_err(), expected);
    }
    let mut chunk = PeerUploadChunk::open(fixture.local.runtime_state(), "remote", "fixture", 0, 1)
        .await
        .unwrap();
    chunk.request = Box::pin(async { Ok((StatusCode::UNAUTHORIZED, vec![])) });
    assert_eq!(chunk.write(b"a").await.unwrap_err(), "unauthorized");
    fixture.stop().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn peer_upload_window_queue_is_bounded_and_does_not_allocate_a_whole_chunk() {
    let fixture = Fixture::new().await;
    let mut chunk = PeerUploadChunk::open(
        fixture.local.runtime_state(),
        "remote",
        "fixture",
        0,
        (MAX_UPLOAD_WRITE_BYTES * 3) as u64,
    )
    .await
    .unwrap();
    let (tx, mut rx) = mpsc::channel(2);
    chunk.sender = Some(tx);
    chunk.request = Box::pin(std::future::pending());
    let bytes = vec![7; MAX_UPLOAD_WRITE_BYTES];
    chunk.write(&bytes).await.unwrap();
    chunk.write(&bytes).await.unwrap();
    let mut blocked = Box::pin(chunk.write(&bytes));
    assert!(futures_util::poll!(blocked.as_mut()).is_pending());
    let first = rx.recv().await.unwrap().unwrap();
    assert_eq!(first.len(), MAX_UPLOAD_WRITE_BYTES);
    blocked.await.unwrap();
    assert_eq!(
        rx.recv().await.unwrap().unwrap().len(),
        MAX_UPLOAD_WRITE_BYTES
    );
    assert_eq!(
        rx.recv().await.unwrap().unwrap().len(),
        MAX_UPLOAD_WRITE_BYTES
    );
    assert!(rx.try_recv().is_err());
    fixture.stop().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn native_peer_upload_rejects_remote_permission_and_session_revocation_over_tls() {
    use plain_rs::features::{access::AccessFeature, prefs::pref_keys};
    let fixture = Fixture::new().await;
    let writers = UploadWriters::default();
    let scope = NativeRequestScope::default();
    let remote = fixture.remote.runtime_state();
    let denied = writers
        .open_peer(
            &fixture.local.native_uploads().unwrap(),
            &scope,
            "remote",
            "denied",
            0,
            0,
        )
        .await
        .unwrap();
    remote
        .prefs
        .set(pref_keys::API_PERMISSIONS, Vec::<AccessFeature>::new())
        .unwrap();
    assert_eq!(
        writers.finish(&scope, &denied).await.unwrap_err(),
        "remote_access_disabled"
    );
    assert!(
        remote
            .upload_chunks
            .list("denied")
            .await
            .unwrap()
            .is_empty()
    );
    remote
        .prefs
        .set(pref_keys::API_PERMISSIONS, [AccessFeature::Files])
        .unwrap();
    let revoked = writers
        .open_peer(
            &fixture.local.native_uploads().unwrap(),
            &scope,
            "remote",
            "revoked",
            0,
            0,
        )
        .await
        .unwrap();
    remote.db.session_delete("local").unwrap();
    assert_eq!(
        writers.finish(&scope, &revoked).await.unwrap_err(),
        "unauthorized"
    );
    assert!(
        remote
            .upload_chunks
            .list("revoked")
            .await
            .unwrap()
            .is_empty()
    );
    fixture.stop().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn peer_upload_refuses_a_wrong_tls_pin_before_creating_remote_chunks() {
    let fixture = Fixture::new().await;
    let mut chunk =
        PeerUploadChunk::open(fixture.local.runtime_state(), "remote", "wrong-pin", 0, 0)
            .await
            .unwrap();
    let route = &chunk.route;
    let wrong = Arc::new(PeerRoute {
        peer: route.peer.clone(),
        base: route.base.clone(),
        client_id: route.client_id.clone(),
        key: route.key.clone(),
        url_key: route.url_key.clone(),
        pin: [0; 32],
        client: plain_libs::protocols::http_client::builder()
            .no_proxy()
            .https_only(true)
            .pin_certificate([0; 32])
            .redirect(plain_libs::protocols::http_client::redirect::Policy::none())
            .build()
            .unwrap(),
        supports_managed_resources: true,
        expires: route.expires,
    });
    let (tx, rx) = mpsc::channel(2);
    chunk.sender = Some(tx);
    chunk.request = crate::managed_peers::upload::request(
        fixture.local.runtime_state(),
        wrong,
        "upload_chunk",
        r#"{"fileId":"wrong-pin","index":0,"size":0}"#,
        "chunk",
        "application/octet-stream",
        rx,
    )
    .unwrap();
    assert!(chunk.finish().await.is_err());
    assert!(
        fixture
            .remote
            .runtime_state()
            .upload_chunks
            .list("wrong-pin")
            .await
            .unwrap()
            .is_empty()
    );
    fixture.stop().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn peer_upload_uses_progress_timeout_instead_of_inherited_header_deadline() {
    let fixture = Fixture::new().await;
    let mut chunk = PeerUploadChunk::open(fixture.local.runtime_state(), "remote", "slow", 0, 0)
        .await
        .unwrap();
    let route = &chunk.route;
    let immediate_header_timeout = Arc::new(PeerRoute {
        peer: route.peer.clone(),
        base: route.base.clone(),
        client_id: route.client_id.clone(),
        key: route.key.clone(),
        url_key: route.url_key.clone(),
        pin: route.pin,
        client: plain_libs::protocols::http_client::builder()
            .no_proxy()
            .https_only(true)
            .pin_certificate(route.pin)
            .read_timeout(std::time::Duration::ZERO)
            .redirect(plain_libs::protocols::http_client::redirect::Policy::none())
            .build()
            .unwrap(),
        supports_managed_resources: true,
        expires: route.expires,
    });
    let (tx, rx) = mpsc::channel(2);
    chunk.sender = Some(tx);
    chunk.request = crate::managed_peers::upload::request(
        fixture.local.runtime_state(),
        immediate_header_timeout,
        "upload_chunk",
        r#"{"fileId":"slow","index":0,"size":0}"#,
        "chunk",
        "application/octet-stream",
        rx,
    )
    .unwrap();
    assert_eq!(chunk.finish().await.unwrap(), 0);
    assert_eq!(
        fixture
            .remote
            .runtime_state()
            .upload_chunks
            .list("slow")
            .await
            .unwrap(),
        vec!["0:0"]
    );
    fixture.stop().await;
}
