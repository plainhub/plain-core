use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn service_starts_real_listeners_and_releases_them_and_the_data_lock_on_shutdown_or_failure()
{
    let directory = tempfile::Builder::new()
        .prefix("cli-service-")
        .tempdir_in(plain_rs::platform::system::consts::home_directory().unwrap())
        .unwrap();
    let paths = AppPaths {
        data_dir: directory.path().join("data"),
        cache_dir: directory.path().join("cache"),
        tls_cert: directory.path().join("tls.pem"),
        tls_key: directory.path().join("tls.key"),
    };
    std::fs::create_dir(&paths.data_dir).unwrap();
    let source = directory.path().join("sources");
    std::fs::create_dir(&source).unwrap();
    let prefs =
        plain_rs::prefs::Prefs::load(&plain_rs::prefs::default_path(&paths.data_dir)).unwrap();
    prefs.set("http_port", 0).unwrap();
    prefs.set("https_port", 0).unwrap();
    let options = || Options {
        media_directories: vec![source.clone()],
        ..Default::default()
    };
    let service = Service::start(&paths, options()).await.unwrap();
    let (http, https) = service.core.http_ports().await.unwrap();
    assert_ne!(http, 0);
    assert_ne!(https, 0);
    let mut connection = tokio::net::TcpStream::connect(("127.0.0.1", http))
        .await
        .unwrap();
    connection
        .write_all(
            b"GET /__plain_server_ready HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await
        .unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        connection.read_to_end(&mut response),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        response.starts_with(b"HTTP/1.1 204"),
        "{}",
        String::from_utf8_lossy(&response)
    );
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    assert!(
        login::requests(&paths.data_dir, None)
            .await
            .unwrap()
            .requests
            .is_empty()
    );
    tokio::time::timeout(std::time::Duration::from_secs(10), service.shutdown())
        .await
        .unwrap();
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", http))
            .await
            .is_err()
    );
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", https))
            .await
            .is_err()
    );
    assert!(login::requests(&paths.data_dir, None).await.is_err());
    let mut invalid = options();
    invalid.web_root = Some(directory.path().join("missing-web"));
    assert!(Service::start(&paths, invalid).await.is_err());
    assert!(login::requests(&paths.data_dir, None).await.is_err());
    let reopened = ContentServer::open_with_cache(&paths.data_dir, &paths.cache_dir).unwrap();
    reopened.stop_core().await;
}
