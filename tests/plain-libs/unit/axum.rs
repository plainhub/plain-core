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
