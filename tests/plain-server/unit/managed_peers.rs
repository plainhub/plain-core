use super::*;
use crate::server::ContentServer;
use plain_graphql::Request;
use plain_rs::features::services::peers::managed_resources::*;
use plain_rs::infrastructure::db::{DPeer, chat_store::peers};
use plain_rs::{
    features::{
        chat::enums::{DeviceType, PeerStatus},
        prefs::Prefs,
        services::sessions,
    },
    infrastructure::{db::chat_store::SaveMode, host::request_scope::NativeRequestScope},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn start(path: &std::path::Path, id: &str) -> ContentServer {
    std::fs::create_dir_all(path).unwrap();
    let prefs = Arc::new(Prefs::load(&path.join("prefs.json")).unwrap());
    prefs.set("client_id", id).unwrap();
    ContentServer::start_core(path, &plain_libs::crypto::gen_token(), prefs).unwrap()
}

fn peer(id: &str) -> DPeer {
    let mut peer = DPeer::new(id, "Fixture", "127.0.0.1", Some(9), DeviceType::Phone);
    peer.status = PeerStatus::Paired;
    peer.public_key = plain_libs::utils::base64::base64_encode(&[7; 32]);
    peer.token = plain_libs::utils::base64::base64_encode(&[8; 32]);
    peer
}

#[tokio::test]
async fn native_peer_requests_reject_invalid_target_scope_and_size_before_creating_routes() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = start(dir.path(), "local");
    let state = server.runtime_state();
    let executor = server.native_graphql().unwrap();
    for id in ["", "missing", "https://other.invalid"] {
        assert!(
            executor
                .execute_peer_scoped(
                    id,
                    Request::new("{ app { clientId } }"),
                    Default::default(),
                    std::time::Duration::from_secs(30)
                )
                .await
                .is_err()
        );
        assert!(
            executor
                .open_managed_peer_subscriptions(id, Default::default())
                .await
                .is_err()
        );
    }
    for mut peer in [
        peer("local"),
        peer("unpaired"),
        peer("no-identity"),
        peer("no-token"),
    ] {
        match peer.id.as_str() {
            "unpaired" => peer.status = PeerStatus::Unpaired,
            "no-identity" => peer.public_key.clear(),
            "no-token" => peer.token.clear(),
            _ => {}
        }
        peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
        assert!(
            executor
                .execute_peer_scoped(
                    &peer.id,
                    Request::new("{ app { clientId } }"),
                    Default::default(),
                    std::time::Duration::from_secs(30)
                )
                .await
                .is_err()
        );
    }
    let scope = NativeRequestScope::default();
    scope.close();
    assert_eq!(
        executor
            .execute_peer_scoped(
                "missing",
                Request::new("{ app { clientId } }"),
                scope.clone(),
                std::time::Duration::from_secs(30)
            )
            .await
            .unwrap_err(),
        "Native request scope closed"
    );
    assert_eq!(
        executor
            .open_managed_peer_subscriptions("missing", scope)
            .await
            .unwrap_err(),
        "Native request scope closed"
    );
    assert_eq!(
        executor
            .execute_peer_scoped(
                "missing",
                Request::new("x".repeat(GRAPHQL_LIMIT)),
                Default::default(),
                std::time::Duration::from_secs(30)
            )
            .await
            .unwrap_err(),
        "GraphQL request too large"
    );
    assert!(state.managed_resources.routes.lock().unwrap().is_empty());
    server.stop_core().await;
}

#[tokio::test]
async fn cancelling_an_owner_interrupts_pending_route_resolution() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = start(dir.path(), "local");
    let state = server.runtime_state();
    peers::save(&state.db, &[peer("remote")], SaveMode::Insert).unwrap();
    let slot = Arc::new(tokio::sync::Mutex::new(None));
    state
        .managed_resources
        .routes
        .lock()
        .unwrap()
        .insert("remote".into(), slot.clone());
    let locked = slot.lock().await;
    let executor = server.native_graphql().unwrap();
    let request_scope = NativeRequestScope::default();
    let subscription_scope = NativeRequestScope::default();
    let request = executor.execute_peer_scoped(
        "remote",
        Request::new("{ app { clientId } }"),
        request_scope.clone(),
        std::time::Duration::from_secs(30),
    );
    let subscription =
        executor.open_managed_peer_subscriptions("remote", subscription_scope.clone());
    tokio::pin!(request, subscription);
    assert!(futures_util::poll!(request.as_mut()).is_pending());
    assert!(futures_util::poll!(subscription.as_mut()).is_pending());
    request_scope.close();
    subscription_scope.close();
    assert_eq!(request.await.unwrap_err(), "Native request scope closed");
    assert_eq!(
        subscription.await.unwrap_err(),
        "Native request scope closed"
    );
    assert!(locked.is_none());
    drop(locked);
    server.stop_core().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn native_queries_and_subscriptions_share_a_pinned_route_without_a_local_listener() {
    const LOCAL_ID: &str = "706c6169-1414-8000-8000-000000000002";
    const REMOTE_ID: &str = "706c6169-1414-8000-8000-000000000003";
    let dir = plain_libs::temporary::tempdir().unwrap();
    let local = start(&dir.path().join("local"), LOCAL_ID);
    let remote = start(&dir.path().join("remote"), REMOTE_ID);
    assert_eq!(local.port, 0);
    let (cert, key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (_, port) = remote.start_public(0, 0, cert, key).await.unwrap();
    let state = local.runtime_state();
    let remote_state = remote.runtime_state();
    let mut session = sessions::create(&remote_state, LOCAL_ID.into()).unwrap();
    session.client_id = LOCAL_ID.into();
    session.r#type = plain_rs::features::sessions::SessionType::Login;
    remote_state.db.session_save(&session).unwrap();
    let mut peer = peer(REMOTE_ID);
    peer.port = Some(port);
    peer.token = session.token;
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let executor = local.native_graphql().unwrap();
    let request = || Request::new("{ app { clientId } }");
    let response = executor
        .execute_peer_scoped(
            REMOTE_ID,
            request(),
            Default::default(),
            std::time::Duration::from_secs(30),
        )
        .await
        .unwrap();
    assert!(response.errors.is_empty(), "{response:?}");
    assert_eq!(
        plain_libs::serde_json::to_value(response).unwrap()["data"]["app"]["clientId"],
        REMOTE_ID
    );
    let mut wrong_target = peer.clone();
    wrong_target.id = "different-peer".into();
    peers::save(&state.db, &[wrong_target], SaveMode::Insert).unwrap();
    assert_eq!(
        executor
            .execute_peer_scoped(
                "different-peer",
                request(),
                Default::default(),
                std::time::Duration::from_secs(30)
            )
            .await
            .unwrap_err(),
        "Managed device identity changed"
    );
    let route = state
        .managed_resources
        .route(&state, REMOTE_ID)
        .await
        .unwrap();
    assert_eq!(
        route.pin,
        decode_pin(
            &remote_state
                .public_server
                .lock()
                .await
                .as_ref()
                .unwrap()
                .tls_certificate_sha256
        )
        .unwrap()
    );
    let again = state
        .managed_resources
        .route(&state, REMOTE_ID)
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&route, &again));
    let scope = NativeRequestScope::default();
    let handle = executor
        .open_managed_peer_subscriptions(REMOTE_ID, scope.clone())
        .await
        .unwrap();
    let subscription = executor.subscription_session(handle).unwrap();
    assert_eq!(subscription.scope_id(), scope.id());
    subscription.send(r#"{"type":"subscribe","id":"content","payload":{"query":"subscription { contentChanged }"}}"#).unwrap();
    let ready = tokio::time::timeout(Duration::from_secs(5), subscription.read())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        plain_libs::serde_json::from_str::<plain_libs::serde_json::Value>(&ready).unwrap()["payload"]
            ["extensions"]["subscriptionReady"],
        true
    );
    assert!(Arc::ptr_eq(
        &route,
        &state
            .managed_resources
            .route(&state, REMOTE_ID)
            .await
            .unwrap()
    ));
    remote_state
        .events
        .send(plain_rs::infrastructure::events::WsEvent::broadcast(
            "CONTENT_CHANGED",
            "{}".into(),
        ))
        .unwrap();
    let event = tokio::time::timeout(Duration::from_secs(5), subscription.read())
        .await
        .unwrap()
        .unwrap();
    let event: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(&event).unwrap();
    assert_eq!(event["id"], "content");
    assert_eq!(event["payload"]["data"]["contentChanged"], true);
    let sockets = local.native_sockets().unwrap();
    let socket_handle = sockets
        .open(
            crate::native::sockets::SocketKind::ImageEditor,
            Some(REMOTE_ID.into()),
            Default::default(),
        )
        .unwrap();
    let socket_session = sockets.session(&socket_handle).unwrap();
    for expected in [vec![0], vec![3]] {
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), socket_session.read())
                .await
                .unwrap()
                .unwrap(),
            expected
        );
    }
    remote_state
        .events
        .send(plain_rs::infrastructure::events::WsEvent::broadcast_binary(
            "IMAGE_EDITOR_UPDATE",
            vec![0, 255, 1],
        ))
        .unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), socket_session.read())
            .await
            .unwrap()
            .unwrap(),
        vec![4, 0, 255, 1]
    );
    let wrong_pin = [0; 32];
    assert_ne!(route.pin, wrong_pin);
    let wrong_route = Arc::new(PeerRoute {
        peer: route.peer.clone(),
        base: route.base.clone(),
        client_id: route.client_id.clone(),
        key: route.key.clone(),
        url_key: route.url_key.clone(),
        pin: wrong_pin,
        client: http_client::builder()
            .no_proxy()
            .https_only(true)
            .pin_certificate(wrong_pin)
            .redirect(http_client::redirect::Policy::none())
            .build()
            .unwrap(),
        expires: route.expires,
    });
    let slot = state
        .managed_resources
        .routes
        .lock()
        .unwrap()
        .get(REMOTE_ID)
        .unwrap()
        .clone();
    *slot.lock().await = Some(wrong_route);
    assert!(
        executor
            .execute_peer_scoped(
                REMOTE_ID,
                request(),
                Default::default(),
                std::time::Duration::from_secs(30)
            )
            .await
            .is_err()
    );
    let rejected_handle = executor
        .open_managed_peer_subscriptions(REMOTE_ID, Default::default())
        .await
        .unwrap();
    let rejected = executor.subscription_session(rejected_handle).unwrap();
    rejected.send(r#"{"type":"subscribe","id":"content","payload":{"query":"subscription { contentChanged }"}}"#).unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(5), rejected.read())
            .await
            .unwrap()
            .is_err()
    );
    let rejected_socket = sockets
        .open(
            crate::native::sockets::SocketKind::ImageEditor,
            Some(REMOTE_ID.into()),
            Default::default(),
        )
        .unwrap();
    let rejected_socket = sockets.session(&rejected_socket).unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(5), rejected_socket.read())
            .await
            .unwrap()
            .is_err()
    );
    executor.close_subscriptions(rejected_handle);
    *slot.lock().await = Some(route.clone());
    peers::unpair(&state.db, REMOTE_ID).unwrap();
    assert!(route.check_current(&state).is_err());
    assert!(
        executor
            .execute_peer_scoped(
                REMOTE_ID,
                request(),
                Default::default(),
                std::time::Duration::from_secs(30)
            )
            .await
            .is_err()
    );
    assert!(subscription.send(r#"{"type":"ping"}"#).is_err());
    assert!(
        tokio::time::timeout(Duration::from_secs(5), subscription.read())
            .await
            .unwrap()
            .is_err()
    );
    assert!(
        tokio::time::timeout(Duration::from_secs(5), socket_session.read())
            .await
            .unwrap()
            .is_err()
    );
    executor.close_subscriptions(handle);
    local.stop_core().await;
    remote.stop_core().await;
}

#[tokio::test]
async fn cached_route_rejects_identity_address_and_management_token_changes() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = start(dir.path(), "local");
    let state = server.runtime_state();
    let original = peer("remote");
    peers::save(&state.db, &[original.clone()], SaveMode::Insert).unwrap();
    let route = PeerRoute {
        peer: original.clone(),
        base: http_client::Url::parse("https://127.0.0.1:9").unwrap(),
        client_id: "local".into(),
        key: vec![8; 32],
        url_key: vec![9; 32],
        pin: [1; 32],
        client: http_client::builder().no_proxy().build().unwrap(),
        expires: Instant::now() + ROUTE_LIFETIME,
    };
    assert!(route.check_current(&state).is_ok());
    for field in ["public-key", "token", "address", "status"] {
        let mut changed = original.clone();
        match field {
            "public-key" => changed.public_key = "other".into(),
            "token" => changed.token = plain_libs::utils::base64::base64_encode(&[1; 32]),
            "address" => changed.port = Some(10),
            "status" => changed.status = PeerStatus::Unpaired,
            _ => unreachable!(),
        }
        state.db.upsert_peer(&changed);
        let stored = peers::get(&state.db, &changed.id).unwrap().unwrap();
        assert_eq!(stored.token, changed.token);
        assert_eq!(stored.public_key, changed.public_key);
        assert_eq!(stored.port, changed.port);
        assert_eq!(stored.status, changed.status);
        assert!(route.check_current(&state).is_err(), "{field}");
        assert!(
            route
                .exchange(&state, r#"{"query":"{ app { clientId } }"}"#, None)
                .await
                .is_err(),
            "{field}"
        );
    }
    server.stop_core().await;
}

#[test]
fn management_credentials_require_exact_key_and_certificate_lengths() {
    assert!(decode_key(&plain_libs::utils::base64::base64_encode(&[0; 32])).is_ok());
    for size in [0, 31, 33] {
        assert!(decode_key(&plain_libs::utils::base64::base64_encode(&vec![0; size])).is_err());
    }
    assert_eq!(decode_pin(&"ab".repeat(32)).unwrap(), [0xab; 32]);
    assert!(decode_pin(&"a".repeat(63)).is_err());
    assert!(decode_pin(&"g".repeat(64)).is_err());
}

#[tokio::test]
async fn native_request_budgets_outlive_shared_timeouts_and_cancel_without_retry() {
    use plain_libs::axum::{body::Bytes, routing::post};
    use plain_rs::infrastructure::transport::http_response;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::{mpsc, oneshot};
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = start(dir.path(), "local");
    let state = server.runtime_state();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut peer = peer("remote");
    peer.port = Some(address.port());
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let key = decode_key(&peer.token).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let (arrived, mut requests) = mpsc::unbounded_channel::<oneshot::Sender<()>>();
    let router = plain_libs::axum::Router::new().route(
        "/graphql",
        post({
            let key = key.clone();
            let calls = calls.clone();
            move |body: Bytes| {
                let key = key.clone();
                let calls = calls.clone();
                let arrived = arrived.clone();
                async move {
                    let plaintext = plain_rs::infrastructure::transport::http_request::decrypt_raw(
                        &key, "POST", "/graphql", &body,
                    )
                    .unwrap();
                    let text = std::str::from_utf8(&plaintext).unwrap();
                    let document: plain_libs::serde_json::Value =
                        plain_libs::serde_json::from_str(text).unwrap();
                    assert_eq!(
                        document["query"],
                        "mutation { deleteChunks(fileId: \"fixture\") }"
                    );
                    calls.fetch_add(1, Ordering::SeqCst);
                    let (release, wait) = oneshot::channel();
                    arrived.send(release).unwrap();
                    let _ = wait.await;
                    http_response::Context::new(&body)
                        .encrypt(&key, br#"{"data":{"deleteChunks":true}}"#)
                        .unwrap()
                }
            }
        }),
    );
    let upstream =
        tokio::spawn(async move { plain_libs::axum::serve(listener, router).await.unwrap() });
    let route = Arc::new(PeerRoute {
        peer,
        base: http_client::Url::parse(&format!("http://{address}")).unwrap(),
        client_id: "local".into(),
        key,
        url_key: vec![9; 32],
        pin: [1; 32],
        client: http_client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .read_timeout(Duration::from_secs(30))
            .build()
            .unwrap(),
        expires: Instant::now() + ROUTE_LIFETIME,
    });
    state.managed_resources.routes.lock().unwrap().insert(
        "remote".into(),
        Arc::new(tokio::sync::Mutex::new(Some(route))),
    );
    let execute = |budget: u64, scope: NativeRequestScope| {
        let executor = server.native_graphql().unwrap();
        tokio::spawn(async move {
            executor
                .execute_peer_scoped(
                    "remote",
                    Request::new("mutation { deleteChunks(fileId: \"fixture\") }"),
                    scope,
                    Duration::from_secs(budget),
                )
                .await
        })
    };
    for (budget, elapsed) in [(30, 11), (120, 31)] {
        let pending = execute(budget, Default::default());
        let release = requests.recv().await.unwrap();
        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(elapsed)).await;
        tokio::task::yield_now().await;
        assert!(
            !pending.is_finished(),
            "caller budget {budget} was shortened"
        );
        release.send(()).unwrap();
        tokio::time::resume();
        let response = pending.await.unwrap().unwrap();
        assert_eq!(
            plain_libs::serde_json::to_value(response).unwrap()["data"]["deleteChunks"],
            true
        );
    }
    let pending = execute(30, Default::default());
    let release = requests.recv().await.unwrap();
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(31)).await;
    assert_eq!(pending.await.unwrap().unwrap_err(), "connection_timeout");
    tokio::time::resume();
    let _ = release.send(());
    let scope = NativeRequestScope::default();
    let pending = execute(120, scope.clone());
    let release = requests.recv().await.unwrap();
    scope.close();
    assert_eq!(
        pending.await.unwrap().unwrap_err(),
        "Native request scope closed"
    );
    let _ = release.send(());
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert!(requests.try_recv().is_err());
    server.stop_core().await;
    upstream.abort();
    let _ = upstream.await;
}

#[tokio::test]
async fn request_timeout_errors_use_the_native_timeout_contract() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/graphql", listener.local_addr().unwrap());
    let pending = tokio::spawn(async move {
        http_client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(url)
            .timeout(Duration::from_secs(1))
            .send()
            .await
    });
    let (socket, _) = listener.accept().await.unwrap();
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(2)).await;
    let error = pending.await.unwrap().err().unwrap();
    tokio::time::resume();
    assert!(error.is_timeout());
    assert_eq!(request_error(error.into()), "connection_timeout");
    drop(socket);
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn stale_bootstrap_unauthorized_cannot_revoke_a_replacement_management_session() {
    use plain_libs::axum::{body::Bytes, http::StatusCode, routing::post};
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
    use tokio::sync::{mpsc, oneshot, watch};
    let dir = plain_libs::temporary::tempdir().unwrap();
    let local = start(dir.path(), "local");
    let state = local.runtime_state();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut peer = peer("remote");
    peer.port = Some(listener.local_addr().unwrap().port());
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let key = decode_key(&peer.token).unwrap();
    let (arrived, mut requests) = mpsc::unbounded_channel::<oneshot::Sender<()>>();
    let router = plain_libs::axum::Router::new().route(
        "/graphql",
        post(move |body: Bytes| {
            let key = key.clone();
            let arrived = arrived.clone();
            async move {
                let plaintext = plain_rs::infrastructure::transport::http_request::decrypt_raw(
                    &key, "POST", "/graphql", &body,
                )
                .unwrap();
                let text = std::str::from_utf8(&plaintext).unwrap();
                let document: plain_libs::serde_json::Value =
                    plain_libs::serde_json::from_str(text).unwrap();
                assert!(
                    document["query"]
                        .as_str()
                        .unwrap()
                        .contains("tlsCertificateSha256")
                );
                let (release, wait) = oneshot::channel();
                arrived.send(release).unwrap();
                wait.await.unwrap();
                StatusCode::UNAUTHORIZED
            }
        }),
    );
    let (cert, key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from_pem_slice(&cert).unwrap()],
            PrivateKeyDer::from_pem_slice(&key).unwrap(),
        )
        .unwrap();
    let (stop, receiver) = watch::channel(false);
    let upstream = tokio::spawn(plain_libs::protocols::https::serve(
        listener, router, tls, receiver,
    ));
    for changed in [false, true] {
        let executor = local.native_graphql().unwrap();
        let pending = tokio::spawn(async move {
            executor
                .execute_peer_scoped(
                    "remote",
                    Request::new("{ app { clientId } }"),
                    Default::default(),
                    Duration::from_secs(30),
                )
                .await
        });
        let release = requests.recv().await.unwrap();
        if changed {
            peer.token = plain_libs::utils::base64::base64_encode(&[9; 32]);
            state.db.upsert_peer(&peer);
        }
        release.send(()).unwrap();
        assert_eq!(
            pending.await.unwrap().unwrap_err(),
            if changed {
                "Management session revoked"
            } else {
                "unauthorized"
            }
        );
        assert_eq!(
            peers::get(&state.db, "remote").unwrap().unwrap().token,
            peer.token
        );
        assert!(
            state.managed_resources.routes.lock().unwrap()["remote"]
                .try_lock()
                .unwrap()
                .is_none()
        );
    }
    assert!(requests.try_recv().is_err());
    stop.send(true).unwrap();
    upstream.await.unwrap().unwrap();
    local.stop_core().await;
}
