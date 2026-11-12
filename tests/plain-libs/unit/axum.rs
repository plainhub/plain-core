use super::*;
use crate::http::{Request, StatusCode};
use crate::http_body_util::BodyExt;
use crate::tower::ServiceExt;
use serde::Deserialize;
#[derive(Deserialize)]
struct Query {
    number: u8,
}
#[tokio::test]
async fn routing_json_query_extraction_limits_and_rejection_keep_contracts() {
    async fn handler(
        extract::Path(key): extract::Path<String>,
        extract::Query(query): extract::Query<Query>,
        Json(value): Json<crate::serde_json::Value>,
    ) -> Json<crate::serde_json::Value> {
        Json(crate::serde_json::json!({"key":key,"number":query.number,"value":value}))
    }
    let router = Router::new()
        .route("/items/:key", routing::post(handler))
        .layer(extract::DefaultBodyLimit::max(64));
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/items/a%20b?number=7")
                .header("content-type", "application/json")
                .body(body::Body::from("{\"n\":1}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: crate::serde_json::Value =
        crate::serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    assert_eq!(
        value,
        crate::serde_json::json!({"key":"a b","number":7,"value":{"n":1}})
    );
    for (uri, payload, status) in [
        ("/items/a?number=999", "{}", StatusCode::BAD_REQUEST),
        ("/items/a?number=7", "[", StatusCode::BAD_REQUEST),
        ("/missing?number=7", "{}", StatusCode::NOT_FOUND),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(body::Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/items/a?number=7")
                .header("content-type", "application/json")
                .body(body::Body::from(" ".repeat(65)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn extractor_futures_are_send_without_boxing_and_keep_optional_rejections() {
    use crate::axum_core::extract::FromRequestParts;
    use std::collections::HashMap;
    fn require_send<T: Send>(_: &T) {}
    let (mut parts, _) = crate::http::Request::builder()
        .uri("/?count=7")
        .body(())
        .unwrap()
        .into_parts();
    let future = extract::Query::<HashMap<String, u8>>::from_request_parts(&mut parts, &());
    require_send(&future);
    assert!(!std::any::type_name_of_val(&future).contains("alloc::boxed::Box<dyn"));
    assert_eq!(future.await.unwrap().0.get("count"), Some(&7));
    let (mut parts, _) = crate::http::Request::builder()
        .uri("/?count=bad")
        .body(())
        .unwrap()
        .into_parts();
    type Query = extract::Query<HashMap<String, u8>>;
    assert!(
        <Option<Query>>::from_request_parts(&mut parts, &())
            .await
            .unwrap()
            .is_none()
    );
    let rejected = <Result<Query, _>>::from_request_parts(&mut parts, &())
        .await
        .unwrap();
    assert!(rejected.is_err());
}

#[test]
#[ignore = "Requires PLAIN_LIBS_CODEGEN_RLIB and PLAIN_LIBS_CODEGEN_DEPS from a production library build"]
fn constructing_an_extractor_future_has_no_allocator_or_wrapper_calls() {
    let ir = crate::codegen_test::compile_probe(
        r#"
extern crate plain_libs;
use plain_libs::axum::extract::FromRequestParts;
use plain_libs::axum::extract::{Query, rejection::QueryRejection};
use std::{collections::BTreeMap, future::Future};
#[unsafe(no_mangle)]
pub fn construct_future(parts: &mut plain_libs::http::request::Parts) -> impl Future<Output = Result<Query<BTreeMap<String, u16>>, QueryRejection>> + Send + '_ {
    Query::from_request_parts(parts, &())
}
"#,
    );
    assert!(
        crate::codegen_test::external_calls(&ir, "construct_future").is_empty(),
        "constructing an extractor future must remain a stack operation"
    );
}

#[tokio::test]
async fn cancelling_native_parts_extraction_keeps_headers_extensions_and_body() {
    use crate::axum_core::extract::FromRequestParts;
    use std::{
        convert::Infallible,
        future::Future,
        sync::atomic::{AtomicUsize, Ordering},
        task::{Context, Waker},
    };
    struct State(AtomicUsize);
    struct PendingParts;
    impl FromRequestParts<State> for PendingParts {
        type Rejection = Infallible;
        async fn from_request_parts<'extract>(
            parts: &'extract mut crate::http::request::Parts,
            state: &'extract State,
        ) -> Result<Self, Self::Rejection>
        where
            Self: 'extract,
        {
            state
                .0
                .store((&parts.headers as *const _) as usize, Ordering::Relaxed);
            parts
                .headers
                .insert("x-observed", crate::http::HeaderValue::from_static("yes"));
            std::future::pending::<()>().await;
            Ok(Self)
        }
    }
    for poll_first in [false, true] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/generated?count=7")
            .header("x-kept", "retained")
            .body(body::Body::from("generated body"))
            .unwrap();
        request.extensions_mut().insert(42_u64);
        let (mut parts, body) = request.into_parts();
        let original_headers = &parts.headers as *const _ as usize;
        let state = State(AtomicUsize::new(0));
        {
            let future = PendingParts::from_request_parts(&mut parts, &state);
            let mut future = std::pin::pin!(future);
            if poll_first {
                let mut cx = Context::from_waker(Waker::noop());
                assert!(future.as_mut().poll(&mut cx).is_pending());
            }
        }
        assert_eq!(parts.headers["x-kept"], "retained");
        assert_eq!(parts.extensions.get::<u64>(), Some(&42));
        assert_eq!(parts.method, "POST");
        assert_eq!(parts.uri, "/generated?count=7");
        if poll_first {
            assert_eq!(state.0.load(Ordering::Relaxed), original_headers);
            assert_eq!(parts.headers["x-observed"], "yes");
        } else {
            assert_eq!(state.0.load(Ordering::Relaxed), 0);
        }
        assert_eq!(body.collect().await.unwrap().to_bytes(), "generated body");
    }
}

#[tokio::test]
async fn extractors_keep_non_static_markers_out_of_the_future_lifetime() {
    use crate::axum::extract::FromRequest;
    use std::{convert::Infallible, future::Future, pin::Pin};
    struct Extracted;
    impl<'marker> FromRequest<(), &'marker ()> for Extracted {
        fn from_request<'extract>(
            _: Request<body::Body>,
            _: &'extract (),
        ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send + 'extract
        where
            Self: 'extract,
        {
            async move { Ok(Self) }
        }
        type Rejection = Infallible;
    }
    fn erased<'marker>(
        _: &'marker (),
    ) -> Pin<Box<dyn Future<Output = Result<Extracted, Infallible>> + Send + 'static>> {
        Box::pin(<Extracted as FromRequest<(), &'marker ()>>::from_request(
            Request::new(body::Body::empty()),
            &(),
        ))
    }
    let future = {
        let marker = ();
        erased(&marker)
    };
    assert!(future.await.is_ok());
}
