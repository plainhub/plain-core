use super::*;
#[tokio::test]
async fn http1_client_server_keep_body_bytes_headers_and_path() {
    use crate::http_body_util::{BodyExt, Full};
    use crate::hyper_util::rt::TokioIo;
    use bytes::Bytes;
    let (client, server) = tokio::io::duplex(4096);
    let server = tokio::spawn(async move {
        server::conn::http1::Builder::new()
            .serve_connection(
                TokioIo::new(server),
                service::service_fn(|request: Request<body::Incoming>| async move {
                    assert_eq!(request.uri().path(), "/resource");
                    assert_eq!(
                        request.into_body().collect().await.unwrap().to_bytes(),
                        b"request".as_slice()
                    );
                    Ok::<_, std::convert::Infallible>(
                        Response::builder()
                            .header("x-answer", "yes")
                            .body(Full::new(Bytes::from_static(b"response")))
                            .unwrap(),
                    )
                }),
            )
            .await
    });
    let (mut sender, connection) = client::conn::http1::handshake(TokioIo::new(client))
        .await
        .unwrap();
    let driver = tokio::spawn(connection);
    let request = Request::builder()
        .method(Method::POST)
        .uri("/resource")
        .header("host", "example.test")
        .body(Full::new(Bytes::from_static(b"request")))
        .unwrap();
    let response = sender.send_request(request).await.unwrap();
    assert_eq!(response.headers()["x-answer"], "yes");
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        b"response".as_slice()
    );
    drop(sender);
    driver.abort();
    server.abort();
    let _ = driver.await;
    let _ = server.await;
}
