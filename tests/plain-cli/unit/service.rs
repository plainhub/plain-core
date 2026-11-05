use super::*;
use plain_rs::features::access::AccessFeature;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn service_preserves_explicit_web_access_and_password_across_restart() {
    for configured in [Vec::<String>::new(), vec!["SMS".to_owned()]] {
        let directory = tempfile::tempdir().unwrap();
        let paths = AppPaths {
            data_dir: directory.path().join("data"),
            cache_dir: directory.path().join("cache"),
            tls_cert: directory.path().join("tls.pem"),
            tls_key: directory.path().join("tls.key"),
        };
        std::fs::create_dir(&paths.data_dir).unwrap();
        let prefs = plain_rs::features::prefs::Prefs::load(
            &plain_rs::features::prefs::default_path(&paths.data_dir),
        )
        .unwrap();
        prefs.set("http_port", 0).unwrap();
        prefs.set("https_port", 0).unwrap();
        prefs.set("api_permissions", &configured).unwrap();
        plain_rs::features::prefs::security::set_password_login(&prefs, "owned fixture password")
            .unwrap();
        let password = plain_rs::features::prefs::security::password_hash(&prefs).unwrap();
        let mut identity = None;
        for _ in 0..2 {
            let service = Service::start(
                &paths,
                Options {
                    file_index_defaults: Some(plain_rs::features::prefs::file_index::Defaults {
                        scan_directories: Vec::new(),
                        excluded_directories: Vec::new(),
                        excluded_names: Vec::new(),
                    }),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
            let prefs = service.core.shared_preferences().unwrap();
            assert_eq!(
                prefs.get::<Vec<String>>("api_permissions").unwrap(),
                Some(configured.clone())
            );
            assert_eq!(
                plain_rs::features::prefs::security::password_hash(&prefs).unwrap(),
                password
            );
            assert!(!plain_rs::features::services::device::access::is_enabled(
                &prefs,
                &[AccessFeature::Files]
            ));
            let current = (
                prefs.get::<String>("client_id").unwrap(),
                prefs.get::<String>("url_token").unwrap(),
            );
            if let Some(expected) = &identity {
                assert_eq!(&current, expected);
            }
            identity = Some(current);
            service.shutdown().await;
        }
    }
}

#[tokio::test]
async fn service_starts_real_listeners_and_releases_them_and_the_data_lock_on_shutdown_or_failure()
{
    let directory = tempfile::Builder::new()
        .prefix("cli-service-")
        .tempdir_in(plain_rs::infrastructure::platform::system::consts::home_directory().unwrap())
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
    std::fs::write(source.join("owned.txt"), b"owned command-line source").unwrap();
    let prefs = plain_rs::features::prefs::Prefs::load(&plain_rs::features::prefs::default_path(
        &paths.data_dir,
    ))
    .unwrap();
    prefs.set("http_port", 0).unwrap();
    prefs.set("https_port", 0).unwrap();
    let options = || Options {
        media_directories: vec![source.clone()],
        file_index_defaults: Some(plain_rs::features::prefs::file_index::Defaults {
            scan_directories: Vec::new(),
            excluded_directories: Vec::new(),
            excluded_names: Vec::new(),
        }),
        ..Default::default()
    };
    let service = Service::start(&paths, options()).await.unwrap();
    assert!(
        plain_rs::features::services::device::access::is_enabled(
            &service.core.shared_preferences().unwrap(),
            &[AccessFeature::Files]
        ),
        "CLI must initialize Web storage access without a mobile settings step"
    );
    let response = service
        .core
        .native_graphql()
        .unwrap()
        .execute("{app{access}}".into())
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["app"]["access"],
        serde_json::json!(["IMAGES", "VIDEOS", "AUDIO", "DOCS", "FILES"])
    );
    let catalog = service.core.native_media_catalog().unwrap();
    assert_eq!(
        catalog.source_directories().unwrap(),
        vec![source.to_string_lossy().into_owned()]
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while catalog
            .count(plain_rs::features::enums::DataType::Doc)
            .unwrap()
            != 1
        {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        std::fs::read(source.join("owned.txt")).unwrap(),
        b"owned command-line source"
    );
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
