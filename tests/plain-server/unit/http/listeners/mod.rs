use plain_libs::protocols::http_client;
use super::*;
use axum::{
    extract::ConnectInfo,
    routing::{get, post},
};

fn health_router() -> Router {
    Router::new().route("/health", get(|| async { "OK" }))
}

fn certificate() -> (Vec<u8>, Vec<u8>) {
    let (cert_pem, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    (
        cert_pem,
        cert_key,
    )
}

#[tokio::test]
async fn serves_http_and_tls_then_releases_both_ports() {
    let (cert, key) = certificate();
    let router = Router::new().route(
        "/health",
        get(|ConnectInfo(addr): ConnectInfo<SocketAddr>| async move { addr.ip().to_string() }),
    );
    let server = HttpListeners::start(router, 0, 0, cert, key).await.unwrap();
    let http = server.http_port;
    let https = server.https_port;
    let client = http_client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    for scheme_port in [("http", http), ("https", https)] {
        let response = client
            .get(format!(
                "{}://127.0.0.1:{}/health",
                scheme_port.0, scheme_port.1
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.text().await.unwrap(), "127.0.0.1");
    }
    server.shutdown().await;
    assert!(std::net::TcpListener::bind(("0.0.0.0", http)).is_ok());
    assert!(std::net::TcpListener::bind(("0.0.0.0", https)).is_ok());
}

#[tokio::test]
async fn readiness_requires_ok_from_both_health_listeners_without_private_route() {
    use axum::http::StatusCode;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    let probes = Arc::new([AtomicUsize::new(0), AtomicUsize::new(0)]);
    let observed = probes.clone();
    let router = Router::new()
        .route(
            "/health",
            get(
                move |Extension(ConnectionScheme(scheme)): Extension<ConnectionScheme>| {
                    let observed = observed.clone();
                    async move {
                        let index = usize::from(scheme == "https");
                        if observed[index].fetch_add(1, Ordering::SeqCst) == 0 {
                            StatusCode::NO_CONTENT
                        } else {
                            StatusCode::OK
                        }
                    }
                },
            ),
        )
        .fallback(|| async { StatusCode::GONE });
    let (cert, key) = certificate();
    let server = HttpListeners::start(router, 0, 0, cert, key).await.unwrap();
    assert!(probes[0].load(Ordering::SeqCst) >= 2);
    assert!(probes[1].load(Ordering::SeqCst) >= 2);
    let client = http_client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    for (scheme, port) in [("http", server.http_port), ("https", server.https_port)] {
        let response = client
            .get(format!("{scheme}://127.0.0.1:{port}/__plain_server_ready"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::GONE);
    }
    server.shutdown().await;
}

#[tokio::test]
async fn invalid_tls_never_opens_http_listener() {
    let probe = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);
    let result = HttpListeners::start(health_router(), port, 0, vec![], vec![]).await;
    assert!(result.is_err());
    assert!(std::net::TcpListener::bind(("0.0.0.0", port)).is_ok());
}

#[tokio::test]
async fn drop_releases_listeners() {
    let (cert, key) = certificate();
    let server = HttpListeners::start(health_router(), 0, 0, cert, key)
        .await
        .unwrap();
    let ports = [server.http_port, server.https_port];
    drop(server);
    tokio::task::yield_now().await;
    for port in ports {
        assert!(std::net::TcpListener::bind(("0.0.0.0", port)).is_ok());
    }
}

#[tokio::test]
async fn listener_task_abort_is_reported_and_shutdown_releases_ports() {
    let (cert, key) = certificate();
    let server = HttpListeners::start(health_router(), 0, 0, cert, key)
        .await
        .unwrap();
    let ports = [server.http_port, server.https_port];
    let mut failure = server.failures();
    server.tasks[1].abort();
    tokio::time::timeout(Duration::from_secs(1), failure.changed())
        .await
        .unwrap()
        .unwrap();
    assert!(failure.borrow().as_ref().unwrap().contains("HTTPS"));
    assert!(server.check_health().await.is_err());
    server.shutdown().await;
    for port in ports {
        assert!(std::net::TcpListener::bind(("0.0.0.0", port)).is_ok());
    }
}

/// The nearby wire speaks over the generated-TLS listener: plain-rs's
/// client must probe, deliver a pairing cancel, read a rejection as
/// `false`, honour its own deadline, and fail once the listener is gone.
#[tokio::test]
async fn nearby_client_handles_probe_accept_reject_timeout_and_offline() {
    use axum::http::{HeaderMap, StatusCode};
    use plain_rs::features::discovery::nearby_http::{Message, post_url, probe, send, url};
    use plain_rs::features::peers::pairing::protocol::PairingCancel;

    let (cert, key) = certificate();
    let router = health_router().route(
        "/nearby",
        post(|headers: HeaderMap, body: String| async move {
            assert_eq!(headers["content-type"], "application/json");
            if body == "DISCOVER:" {
                return StatusCode::OK;
            }
            if body == "slow" {
                tokio::time::sleep(Duration::from_millis(100)).await;
                return StatusCode::OK;
            }
            let value: serde_json::Value =
                serde_json::from_str(body.strip_prefix("PAIR_CANCEL:").unwrap()).unwrap();
            if value["toId"] == "reject" {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::OK
            }
        }),
    );
    let server = HttpListeners::start(router, 0, 0, cert, key).await.unwrap();
    assert!(probe("127.0.0.1", server.https_port).await.unwrap());
    let cancel = |to: &str| {
        Message::PairCancel(PairingCancel {
            request_signature: "request".into(),
            signature_public_key: "public".into(),
            timestamp: 1,
            signature: "proof".into(),
            from_id: "fixture".into(),
            to_id: to.into(),
        })
    };
    assert!(
        send("127.0.0.1", server.https_port, &cancel("accept"), None)
            .await
            .unwrap()
    );
    assert!(
        !send("127.0.0.1", server.https_port, &cancel("reject"), None)
            .await
            .unwrap()
    );
    assert!(
        !post_url(
            &url("127.0.0.1", server.https_port).unwrap(),
            "slow".into(),
            Duration::from_millis(10),
            None
        )
        .await
        .unwrap()
    );
    let port = server.https_port;
    server.shutdown().await;
    assert!(!probe("127.0.0.1", port).await.unwrap());
}

#[tokio::test]
async fn occupied_ports_fall_back_for_both_listeners() {
    let occupied_http = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
    let occupied_https = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
    let http = occupied_http.local_addr().unwrap().port();
    let https = occupied_https.local_addr().unwrap().port();
    let (cert, key) = certificate();
    let listeners = HttpListeners::start(health_router(), http, https, cert, key)
        .await
        .unwrap();
    assert_ne!(listeners.http_port, http);
    assert_ne!(listeners.https_port, https);
    listeners.check_health().await.unwrap();
    listeners.shutdown().await;
}

#[tokio::test]
async fn https_connections_disable_nagle_before_application_data() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (certificate, key) = certificate();
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let incoming = tokio::net::TcpListener::from_std(listener.try_clone().unwrap()).unwrap();
    let config = std::sync::Arc::new(tls_config(&certificate, &key).unwrap());
    let client = tokio::spawn(async move {
        http_client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap()
            .get(format!("https://{address}/"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    });
    let (stream, _) = incoming.accept().await.unwrap();
    assert!(!stream.nodelay().unwrap());
    let mut stream = https::accept_tls(config, stream).await.unwrap();
    assert!(stream.get_ref().0.nodelay().unwrap());
    let mut request = Vec::new();
    let mut buffer = [0; 1024];
    while !request.ends_with(b"\r\n\r\n") {
        let read = stream.read(&mut buffer).await.unwrap();
        assert_ne!(read, 0);
        request.extend_from_slice(&buffer[..read]);
    }
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
        .await
        .unwrap();
    stream.shutdown().await.unwrap();
    assert_eq!(client.await.unwrap(), "ok");
}
