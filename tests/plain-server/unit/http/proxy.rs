use plain_libs::protocols::http_client;
use super::*;

#[test]
fn only_http_peer_urls_can_be_proxied() {
    assert_eq!(
        target_url("http://192.0.2.9:8080/fs?id=1").unwrap(),
        "http://192.0.2.9:8080/fs?id=1"
    );
    assert_eq!(
        target_url("  https://peer.local/fs  ").unwrap(),
        "https://peer.local/fs"
    );
    // The Kotlin original used `startsWith("http")`, which also accepted these.
    assert!(target_url("httpfoo://peer/fs").is_err());
    assert!(target_url("file:///etc/passwd").is_err());
    assert!(target_url("ftp://peer/fs").is_err());
    assert!(target_url("http://").is_err());
    assert!(target_url("").is_err());
}

#[test]
fn hop_by_hop_and_reencoded_headers_are_not_proxied() {
    for name in SKIPPED {
        assert!(
            [
                "connection",
                "transfer-encoding",
                "upgrade",
                "content-encoding",
                "content-length"
            ]
            .contains(name),
            "{name} must stay out of the proxied response"
        );
    }
    assert!(!SKIPPED.contains(&"content-type"));
    assert!(!SKIPPED.contains(&"etag"));
}

#[test]
fn resource_proxy_uses_the_requested_operation_and_named_size() {
    let parameters = crate::http::fs::Parameters {
        id: "opaque".into(),
        action: plain_rs::features::services::files::content::Action::Download,
        size: None,
        version: Some("updated".into()),
    };
    let url = http_client::Url::parse(
        &resource_target(
            "https://device.invalid/fs?id=encrypted%2Breference",
            &parameters,
        )
        .unwrap(),
    )
    .unwrap();
    let query = url
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(query["id"], "encrypted+reference");
    assert_eq!(query["action"], "download");
    assert_eq!(query["v"], "updated");
    assert!(!query.contains_key("size"));
    assert!(resource_target("https://device.invalid/other?id=reference", &parameters).is_err());
}
