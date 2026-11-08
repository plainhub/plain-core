use super::*;
use std::net::SocketAddr;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpListener,
};

async fn listener() -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    (listener, address)
}

async fn read_request(stream: &mut (impl AsyncRead + Unpin)) -> String {
    let mut headers = Vec::new();
    while !headers.ends_with(b"\r\n\r\n") {
        headers.push(stream.read_u8().await.unwrap());
    }
    String::from_utf8(headers).unwrap()
}

async fn write_reply(
    stream: &mut (impl AsyncWrite + Unpin),
    status: &str,
    headers: &str,
    body: &str,
) {
    stream
        .write_all(
            format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{headers}\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn forward_proxy_sends_absolute_uri_and_auth_and_can_be_bypassed() {
    let (proxy, proxy_address) = listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _) = proxy.accept().await.unwrap();
        let request = read_request(&mut stream).await.to_ascii_lowercase();
        assert!(request.starts_with("get http://example.invalid/asset?q=1 http/1.1\r\n"));
        assert!(request.contains("\r\nproxy-authorization: basic dxnlcjpwqhnz\r\n"));
        write_reply(&mut stream, "200 OK", "", "proxy").await;
    });
    let matcher = Matcher::builder()
        .http(format!("http://user:p%40ss@{proxy_address}"))
        .build();
    let client = builder().build_with_matcher(Some(matcher)).unwrap();
    let response = client
        .get("http://example.invalid/asset?q=1")
        .send()
        .await
        .unwrap();
    assert_eq!(response.text().await.unwrap(), "proxy");
    server.await.unwrap();

    let (origin, origin_address) = listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _) = origin.accept().await.unwrap();
        let request = read_request(&mut stream).await.to_ascii_lowercase();
        assert!(request.starts_with("get /asset http/1.1\r\n"));
        assert!(!request.contains("proxy-authorization"));
        write_reply(&mut stream, "200 OK", "", "direct").await;
    });
    let matcher = Matcher::builder()
        .http(format!("http://user:p%40ss@{proxy_address}"))
        .no("127.0.0.1")
        .build();
    let client = builder().build_with_matcher(Some(matcher)).unwrap();
    let response = client
        .get(format!("http://{origin_address}/asset"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.text().await.unwrap(), "direct");
    server.await.unwrap();
}

#[tokio::test]
async fn redirect_replays_body_on_307_and_strips_credentials_across_origins() {
    let (first, first_address) = listener().await;
    let (second, second_address) = listener().await;
    let first_server = tokio::spawn(async move {
        let (mut stream, _) = first.accept().await.unwrap();
        let request = read_request(&mut stream).await.to_ascii_lowercase();
        assert!(request.starts_with("post /start http/1.1\r\n"));
        assert!(request.contains("\r\nauthorization: basic dxnlcjpwqhnz\r\n"));
        let mut body = [0; 4];
        stream.read_exact(&mut body).await.unwrap();
        assert_eq!(&body, b"test");
        write_reply(
            &mut stream,
            "307 Temporary Redirect",
            &format!("Location: http://{second_address}/next\r\n"),
            "",
        )
        .await;
    });
    let second_server = tokio::spawn(async move {
        let (mut stream, _) = second.accept().await.unwrap();
        let request = read_request(&mut stream).await.to_ascii_lowercase();
        assert!(request.starts_with("post /next http/1.1\r\n"));
        assert!(!request.contains("authorization:"));
        assert!(!request.contains("cookie:"));
        assert!(request.contains(&format!("\r\nreferer: http://{first_address}/start\r\n")));
        let mut body = [0; 4];
        stream.read_exact(&mut body).await.unwrap();
        assert_eq!(&body, b"test");
        write_reply(
            &mut stream,
            "307 Temporary Redirect",
            "Location: /final\r\nConnection: close\r\n",
            "",
        )
        .await;
        drop(stream);

        let (mut stream, _) = second.accept().await.unwrap();
        let request = read_request(&mut stream).await.to_ascii_lowercase();
        assert!(request.starts_with("post /final http/1.1\r\n"));
        assert!(!request.contains("authorization:"));
        assert!(!request.contains("cookie:"));
        assert!(request.contains(&format!("\r\nreferer: http://{second_address}/next\r\n")));
        stream.read_exact(&mut body).await.unwrap();
        assert_eq!(&body, b"test");
        write_reply(&mut stream, "200 OK", "", "ok").await;
    });

    let response = builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
        .post(format!("http://user:p%40ss@{first_address}/start"))
        .header(header::COOKIE, "session=local")
        .body("test")
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.url().as_str(),
        format!("http://{second_address}/final")
    );
    assert_eq!(response.text().await.unwrap(), "ok");
    first_server.await.unwrap();
    second_server.await.unwrap();
}

#[tokio::test]
async fn streams_chunks_before_completion_and_read_timeout_applies_to_the_body() {
    let (origin, address) = listener().await;
    let (release, held) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = origin.accept().await.unwrap();
        read_request(&mut stream).await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\none\r\n")
            .await
            .unwrap();
        held.await.unwrap();
    });

    let mut response = builder()
        .no_proxy()
        .read_timeout(Duration::from_millis(500))
        .build()
        .unwrap()
        .get(format!("http://{address}"))
        .send()
        .await
        .unwrap();
    assert!(response.content_length().is_none());
    assert_eq!(response.chunk().await.unwrap().unwrap(), b"one"[..]);
    assert!(response.chunk().await.unwrap_err().is_timeout());
    release.send(()).unwrap();
    server.await.unwrap();
}

#[cfg(feature = "tls")]
#[tokio::test]
async fn authenticated_connect_uses_the_same_tls_roots_as_direct_connections() {
    for (host, authority) in [("localhost", "localhost"), ("::1", "[::1]")] {
        assert_authenticated_connect(host, authority, false).await;
    }
    assert_authenticated_connect("localhost", "localhost", true).await;
}

#[cfg(feature = "tls")]
async fn assert_authenticated_connect(host: &str, authority: &'static str, https_only: bool) {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
    let (cert, key) = crate::tls::generate_pem(&[host.to_owned()]).unwrap();
    let client_root = Certificate::from_pem(&cert).unwrap();
    let (proxy, address) = listener().await;
    let matcher = Matcher::builder()
        .https(format!("http://user:p%40ss@{address}"))
        .build();
    let client = builder()
        .add_root_certificate(client_root)
        .https_only(https_only)
        .timeout(Duration::from_secs(5))
        .build_with_matcher(Some(matcher))
        .unwrap();
    let tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from_pem_slice(&cert).unwrap()],
            PrivateKeyDer::from_pem_slice(&key).unwrap(),
        )
        .unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = proxy.accept().await.unwrap();
        let request = read_request(&mut stream).await.to_ascii_lowercase();
        assert!(request.starts_with(&format!("connect {authority}:443 http/1.1\r\n")));
        assert!(request.contains("\r\nproxy-authorization: basic dxnlcjpwqhnz\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await
            .unwrap();
        let mut tls = tokio_rustls::TlsAcceptor::from(Arc::new(tls))
            .accept(stream)
            .await
            .unwrap();
        let request = read_request(&mut tls).await.to_ascii_lowercase();
        assert!(request.starts_with("get /asset http/1.1\r\n"));
        assert!(!request.contains("proxy-authorization:"));
        if https_only {
            write_reply(
                &mut tls,
                "302 Found",
                "Location: http://127.0.0.1/asset\r\n",
                "",
            )
            .await;
        } else {
            write_reply(&mut tls, "200 OK", "", "tunnel").await;
        }
    });
    let response = client
        .get(format!("https://{authority}/asset"))
        .send()
        .await;
    if https_only {
        let Err(error) = response else {
            panic!("HTTPS-only request followed a plaintext redirect");
        };
        assert!(error.to_string().contains("redirect"));
    } else {
        assert_eq!(response.unwrap().text().await.unwrap(), "tunnel");
    }
    server.await.unwrap();
}
