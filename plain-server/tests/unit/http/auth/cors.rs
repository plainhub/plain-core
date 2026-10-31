use super::*;
use axum::{Router, body::Body, routing::post};
use tower::ServiceExt;

async fn preflight(
    core: &crate::server::ContentServer,
    origin: &str,
    scheme: &'static str,
) -> Response {
    let router = Router::new()
        .route("/graphql", post(|| async { StatusCode::NO_CONTENT }))
        .layer(axum::middleware::from_fn_with_state(
            core.runtime_state(),
            apply,
        ));
    let mut request = axum::http::Request::builder()
        .method("OPTIONS")
        .uri("/graphql")
        .header("host", "device.local")
        .header("origin", origin)
        .header("access-control-request-method", "POST")
        .header(
            "access-control-request-headers",
            "C-ID, c-platform,c-version,Authorization, Content-Type,x-untrusted",
        )
        .header("x-forwarded-proto", "https")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(crate::http::ConnectionScheme(scheme));
    router.oneshot(request).await.unwrap()
}

#[tokio::test]
async fn cors_uses_actual_scheme_authority_and_safe_preflight_headers() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    core.set_build_debug(false);
    let response = preflight(&core, "https://device.local", "https").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "https://device.local"
    );
    assert_eq!(response.headers()[header::VARY], "Origin");
    assert_eq!(
        response.headers()[header::ACCESS_CONTROL_ALLOW_HEADERS],
        "c-id, c-platform, c-version, authorization, content-type"
    );
    assert_eq!(
        preflight(&core, "https://device.local", "http")
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        preflight(&core, "http://device.local:444", "http")
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        preflight(&core, "http://device.local:80", "http")
            .await
            .status(),
        StatusCode::OK
    );
    core.stop_core().await;
}

#[tokio::test]
async fn cross_origin_access_follows_current_shared_preferences_or_debug_configuration() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    for (any_host, debug, allowed) in [
        (false, false, false),
        (true, false, true),
        (false, true, true),
        (false, false, false),
    ] {
        core.preferences()
            .unwrap()
            .set("allow_any_host", any_host)
            .unwrap();
        core.set_build_debug(debug);
        let response = preflight(&core, "http://localhost:4000", "http").await;
        assert_eq!(
            response.status(),
            if allowed {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            }
        );
        assert_eq!(
            response
                .headers()
                .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            allowed
        );
    }
    core.stop_core().await;
}
