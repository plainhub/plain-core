use super::*;
#[tokio::test]
async fn http2_request_headers_data_trailers_and_shutdown_round_trip() {
    let (client_io, server_io) = tokio::io::duplex(4096);
    let server = tokio::spawn(async move {
        let mut connection = server::handshake(server_io).await.unwrap();
        let (request, mut response) = connection.accept().await.unwrap().unwrap();
        assert_eq!(request.uri().path(), "/resource");
        assert_eq!(request.headers()["x-test"], "request");
        let mut send = response
            .send_response(
                crate::http::Response::builder()
                    .header("x-answer", "yes")
                    .body(())
                    .unwrap(),
                false,
            )
            .unwrap();
        send.send_data(bytes::Bytes::from_static(b"payload"), false)
            .unwrap();
        let mut trailers = crate::http::HeaderMap::new();
        trailers.insert("x-check", crate::http::HeaderValue::from_static("done"));
        send.send_trailers(trailers).unwrap();
        while connection.accept().await.is_some() {}
    });
    let (mut sender, connection) = client::handshake(client_io).await.unwrap();
    let driver = tokio::spawn(connection);
    let request = crate::http::Request::builder()
        .uri("https://example.test/resource")
        .header("x-test", "request")
        .body(())
        .unwrap();
    let (response, _) = sender.send_request(request, true).unwrap();
    let response = response.await.unwrap();
    assert_eq!(response.headers()["x-answer"], "yes");
    let mut body = response.into_body();
    let data = body.data().await.unwrap().unwrap();
    assert_eq!(data, b"payload".as_slice());
    body.flow_control().release_capacity(data.len()).unwrap();
    assert!(body.data().await.is_none());
    assert_eq!(body.trailers().await.unwrap().unwrap()["x-check"], "done");
    drop(body);
    drop(sender);
    driver.abort();
    server.abort();
    let _ = driver.await;
    let _ = server.await;
}
#[tokio::test]
async fn invalid_client_preface_is_rejected() {
    use tokio::io::AsyncWriteExt;
    let (mut client, server) = tokio::io::duplex(128);
    client
        .write_all(b"INVALID * HTTP/2.0\r\n\r\nSM\r\n\r\n")
        .await
        .unwrap();
    drop(client);
    assert!(server::handshake::<_>(server).await.is_err());
}
