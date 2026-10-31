#[tokio::test]
async fn aware_requests_preserve_headers_body_bytes_and_length_hints() {
    use futures_util::StreamExt;
    use hyper::body::Body as _;
    for payload in [vec![], vec![0, 1, 0xff, 2]] {
        let request =
            super::aware_request("POST", "/peer_graphql", "actor", "channel", payload.clone())
                .unwrap();
        assert_eq!(request.method(), "POST");
        assert_eq!(request.uri(), "/peer_graphql");
        assert_eq!(request.headers()["host"], "plain-aware-peer");
        assert_eq!(request.headers()["c-id"], "actor");
        assert_eq!(request.headers()["c-cid"], "channel");
        assert_eq!(
            request.headers()["content-type"],
            "application/octet-stream"
        );
        assert_eq!(
            request.body().size_hint().exact(),
            Some(payload.len() as u64)
        );
        assert_eq!(request.body().is_end_stream(), payload.is_empty());
        let mut stream = axum::body::Body::new(request.into_body()).into_data_stream();
        let mut decoded = Vec::new();
        while let Some(chunk) = stream.next().await {
            decoded.extend_from_slice(&chunk.unwrap());
        }
        assert_eq!(decoded, payload);
    }
    assert!(
        super::aware_request("bad method", "/peer_graphql", "actor", "channel", vec![]).is_err()
    );
    assert!(super::aware_request("GET", "/", "actor\r\ninjected", "channel", vec![]).is_err());
}
