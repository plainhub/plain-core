use super::*;
#[test]
fn websocket_key_validation_preserves_strict_base64_rules() {
    let good = crate::http::Request::builder()
        .uri("/ws")
        .header("connection", "Upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(())
        .unwrap();
    let response = handshake::server::create_response(&good).unwrap();
    assert_eq!(
        response.headers()["sec-websocket-accept"],
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    );
    for key in [
        "dGhlIHNhbXBsZSBub25jZQ=",
        "dGhlIHNhbXBsZSBub25jZR==",
        "______________________==",
    ] {
        let mut request = good.clone();
        request.headers_mut().insert(
            "sec-websocket-key",
            crate::http::HeaderValue::from_str(key).unwrap(),
        );
        assert!(
            handshake::server::create_response(&request).is_err(),
            "{key}"
        );
    }
}
#[test]
fn server_rejects_unmasked_frames_and_invalid_utf8() {
    use std::io::Cursor;
    let mut socket = WebSocket::from_raw_socket(
        Cursor::new(vec![0x81, 1, b'x']),
        protocol::Role::Server,
        None,
    );
    assert!(matches!(socket.read(), Err(Error::Protocol(_))));
    let mut socket = WebSocket::from_raw_socket(
        Cursor::new(vec![0x81, 0x81, 0, 0, 0, 0, 0xff]),
        protocol::Role::Server,
        None,
    );
    assert!(socket.read().is_err());
}
