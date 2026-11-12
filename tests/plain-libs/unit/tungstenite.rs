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


#[test]
fn errors_preserve_status_text_from_conversions_and_exact_source_chains() {
    use std::error::Error as _;
    let error = Error::from(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "generated disconnect"));
    assert_eq!(error.to_string(), "IO error: generated disconnect");
    assert_eq!(error.source().unwrap().downcast_ref::<std::io::Error>().unwrap().kind(), std::io::ErrorKind::BrokenPipe);
    let error = Error::Http(Box::new(crate::http::Response::builder().status(403).body(None).unwrap()));
    assert_eq!(error.to_string(), "HTTP error: 403 Forbidden");
    assert!(error.source().is_none());
    let error = Error::Tls(error::TlsError::InvalidDnsName);
    assert_eq!(error.to_string(), "TLS error: Invalid DNS name");
    assert!(error.source().unwrap().downcast_ref::<error::TlsError>().is_some());
    assert!(error.source().unwrap().source().is_none());
    let error = error::ProtocolError::from(crate::httparse::Error::Token);
    assert!(error.source().unwrap().downcast_ref::<crate::httparse::Error>().is_some());
    let error = error::CapacityError::MessageTooLong { size: 42, max_size: 7 };
    assert_eq!(error.to_string(), "Message too long: 42 > 7");
}
