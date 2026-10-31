use super::*;
use axum::{Router, body::Body, routing::get};
use tower::ServiceExt;

#[tokio::test]
async fn native_bearer_requires_an_actual_loopback_peer_and_never_trusts_forwarded_headers() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let token = state.token.clone();
    let router = Router::new()
        .route("/graphql", get(|| async { StatusCode::NO_CONTENT }))
        .layer(axum::middleware::from_fn_with_state(state, authorize));
    for (peer, bearer, expected) in [
        (None, true, StatusCode::FORBIDDEN),
        (Some("192.0.2.42:1234"), true, StatusCode::FORBIDDEN),
        (Some("127.0.0.1:1234"), true, StatusCode::NO_CONTENT),
        (Some("[::1]:1234"), true, StatusCode::NO_CONTENT),
        (Some("192.0.2.42:1234"), false, StatusCode::NO_CONTENT),
        (None, false, StatusCode::NO_CONTENT),
    ] {
        let mut builder = axum::http::Request::builder()
            .uri("/graphql")
            .header("x-forwarded-for", "127.0.0.1")
            .header("forwarded", "for=127.0.0.1");
        if bearer {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        let mut request = builder.body(Body::empty()).unwrap();
        if let Some(peer) = peer {
            request
                .extensions_mut()
                .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        }
        assert_eq!(
            router.clone().oneshot(request).await.unwrap().status(),
            expected
        );
    }
    core.stop_core().await;
}
