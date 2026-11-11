use super::*;
#[test]
fn bytes_headers_and_uri_parts_keep_shared_storage_and_validation() {
    let bytes = bytes::Bytes::from(vec![b'a'; 512]);
    let header = HeaderValue::from_maybe_shared(bytes.clone()).unwrap();
    assert_eq!(header.as_bytes().as_ptr(), bytes.as_ptr());
    assert!(HeaderValue::from_bytes(b"ok\r\nx:bad").is_err());
    assert!(HeaderName::from_bytes(b"bad name").is_err());
    let uri: Uri = "https://example.test:8443/a%2Fb?q=1".parse().unwrap();
    assert_eq!(uri.host(), Some("example.test"));
    assert_eq!(uri.path_and_query().unwrap(), "/a%2Fb?q=1");
    let request = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("x-test", "yes")
        .body(bytes.clone())
        .unwrap();
    let (parts, body) = request.into_parts();
    let request = Request::from_parts(parts, body);
    assert_eq!(request.body().as_ptr(), bytes.as_ptr());
}
