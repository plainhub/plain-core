use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn serve(
    body: String,
    status: u16,
    declared_length: Option<usize>,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/rss", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 2048];
        stream.read(&mut request).await.unwrap();
        let length = declared_length
            .map(|n| format!("Content-Length: {n}\r\n"))
            .unwrap_or_default();
        let response =
            format!("HTTP/1.1 {status} Fixture\r\n{length}Connection: close\r\n\r\n{body}");
        let _ = stream.write_all(response.as_bytes()).await;
    });
    (url, task)
}

#[tokio::test]
async fn rss_and_atom_preview_preserve_title_semantics_without_persistence() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    for (body, expected) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../plain-rs/testdata/feeds/preview-rss.xml"
            )),
            "A & x &amp; y &#8217; B",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../plain-rs/testdata/feeds/preview-atom.xml"
            )),
            "Atom preview ’",
        ),
    ] {
        let (url, server) = serve(body.into(), 200, Some(body.len())).await;
        assert_eq!(title(&db, &url).await.unwrap(), expected);
        tokio::time::timeout(std::time::Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
        assert!(db.feeds_list().unwrap().is_empty());
        assert_eq!(db.feed_entry_count("").unwrap(), 0);
    }
}

#[tokio::test]
async fn duplicate_and_invalid_urls_are_rejected_before_network_io() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/existing", listener.local_addr().unwrap());
    db.feed_save("existing", "Kept", &url, false, &plain_rs::feeds::now())
        .unwrap();
    tokio::select! {
        result = title(&db, &url) => assert!(result.unwrap_err().to_string().contains("feed_already_exists")),
        _ = listener.accept() => panic!("Duplicate preview must not fetch"),
    }
    for url in ["", " ", "file:///fixture/feed", "ftp://example.org/feed"] {
        assert!(
            title(&db, url)
                .await
                .unwrap_err()
                .to_string()
                .contains("invalid_feed_url")
        );
    }
    assert_eq!(db.feeds_list().unwrap().len(), 1);
}

#[tokio::test]
async fn http_parse_and_size_failures_do_not_create_feeds() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    for (body, status, length, expected) in [
        ("unavailable".to_owned(), 503, Some(11), "server:503"),
        (
            "<html>not a feed</html>".to_owned(),
            200,
            None,
            "invalid_feed_content",
        ),
        (
            String::new(),
            200,
            Some(4 * 1024 * 1024 + 1),
            "response_too_large",
        ),
        (
            "x".repeat(4 * 1024 * 1024 + 1),
            200,
            None,
            "response_too_large",
        ),
    ] {
        let (url, server) = serve(body, status, length).await;
        assert!(
            title(&db, &url)
                .await
                .unwrap_err()
                .to_string()
                .to_lowercase()
                .contains(expected),
            "{expected}"
        );
        tokio::time::timeout(std::time::Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
        assert!(db.feeds_list().unwrap().is_empty());
    }
}

#[tokio::test]
async fn creation_uses_a_custom_name_and_concurrent_duplicates_stay_unique() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/rss", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 2048];
            stream.read(&mut request).await.unwrap();
            let body = "<rss><channel><title>Original title</title></channel></rss>";
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        }
    });
    let (first, second) = tokio::join!(
        super::super::create_without_sync(&db, &url, false, Some("First custom")),
        super::super::create_without_sync(&db, &url, false, Some("Second custom")),
    );
    assert_ne!(first.is_ok(), second.is_ok());
    let (created, rejected) = match (first, second) {
        (Ok(created), Err(error)) | (Err(error), Ok(created)) => (created, error),
        _ => panic!("Exactly one creation must succeed"),
    };
    assert!(matches!(
        created.name.as_str(),
        "First custom" | "Second custom"
    ));
    assert!(rejected.to_string().contains("feed_already_exists"));
    assert_eq!(db.feeds_list().unwrap().len(), 1);
    tokio::time::timeout(std::time::Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
}

#[test]
fn migrated_character_data_fixtures_preserve_entry_titles_links_and_descriptions() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let feed = db
        .feed_save(
            "fixture",
            "Fixture",
            "https://example.org/rss",
            false,
            &plain_rs::feeds::now(),
        )
        .unwrap();
    let rss = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/feeds/preview-rss.xml"
    ));
    let parsed = super::super::parse_feed(rss).unwrap();
    let rows: Vec<_> = parsed
        .entries
        .into_iter()
        .map(|entry| super::super::item_to_row(&feed, entry))
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (&*rows[0].title, &*rows[0].url, &*rows[0].description),
        ("示例标题", "https://example.org/one", "hello")
    );
    assert_eq!(
        (&*rows[1].title, &*rows[1].url),
        ("Second ’ title", "https://example.org/two")
    );
}
