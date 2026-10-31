use super::*;

#[test]
fn page_metadata_returns_page_values_and_resolves_icons_without_caching() {
    let metadata = parse_page_metadata(
        "<title>Fallback</title><meta property='og:title' content=' Chosen &amp; title '><link rel='shortcut icon' href='../icon.svg'>",
        "https://example.com/pages/article",
    );
    assert_eq!(metadata.title.as_deref(), Some("Chosen & title"));
    assert_eq!(
        metadata.favicon_url.as_deref(),
        Some("https://example.com/icon.svg")
    );
}

#[test]
fn missing_page_values_remain_optional_and_title_limits_count_characters() {
    let empty = parse_page_metadata("<html></html>", "https://example.com/page");
    assert_eq!(empty.title, None);
    assert_eq!(
        empty.favicon_url.as_deref(),
        Some("https://example.com/favicon.ico")
    );
    let metadata = parse_page_metadata(
        &format!("<title> {} </title>", "界".repeat(250)),
        "https://example.com/page",
    );
    assert_eq!(metadata.title.unwrap().chars().count(), 200);
}

#[tokio::test]
async fn unsupported_page_urls_are_rejected_before_fetching() {
    for url in ["file:///page.html", "ftp://example.com/page", "invalid"] {
        assert!(fetch_page_metadata(url).await.is_err(), "{url}");
    }
}
