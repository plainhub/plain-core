use super::*;

#[test]
fn parse_simple_http() {
    let u = parse_http_url("http://example.com/foo").unwrap();
    assert_eq!(u.scheme, "http");
    assert_eq!(u.host, "example.com");
    assert_eq!(u.port, None);
    assert_eq!(u.path, "/foo");
    assert_eq!(u.query, "");
}

#[test]
fn parse_https_with_port_and_query() {
    let u = parse_http_url("HTTPS://Example.COM:8443/fs?id=abc&x=1").unwrap();
    assert_eq!(u.scheme, "https");
    assert_eq!(u.host, "example.com");
    assert_eq!(u.port, Some(8443));
    assert_eq!(u.path, "/fs");
    assert_eq!(u.query, "id=abc&x=1");
    assert_eq!(u.query_param("id").as_deref(), Some("abc"));
    assert_eq!(u.query_param("missing"), None);
}

#[test]
fn parse_rejects_non_http_scheme() {
    assert!(parse_http_url("ftp://example.com/").is_none());
    assert!(parse_http_url("file:///etc/passwd").is_none());
}

#[test]
fn parse_rejects_malformed() {
    assert!(parse_http_url("not a url").is_none());
    assert!(parse_http_url("http://").is_none());
    assert!(parse_http_url("http://example.com:abc/").is_none());
}

#[test]
fn parse_ipv6() {
    let u = parse_http_url("http://[::1]:8080/x").unwrap();
    assert_eq!(u.host, "::1");
    assert_eq!(u.port, Some(8080));
    assert_eq!(u.path, "/x");
}

#[test]
fn query_param_first_match_wins() {
    let u = parse_http_url("http://e.com/?id=one&id=two").unwrap();
    assert_eq!(u.query_param("id").as_deref(), Some("one"));
}

#[test]
fn join_absolute_reference_unchanged() {
    assert_eq!(
        join("http://e.com/a/", "http://other.com/x").as_deref(),
        Some("http://other.com/x")
    );
}

#[test]
fn join_path_absolute() {
    assert_eq!(
        join("http://e.com/a/b", "/c").as_deref(),
        Some("http://e.com/c")
    );
}

#[test]
fn join_relative_and_dot_segments() {
    assert_eq!(
        join("http://e.com/a/b", "../c/d").as_deref(),
        Some("http://e.com/c/d")
    );
}

#[test]
fn build_url_omits_default_ports() {
    assert_eq!(
        build_url("http", "example.com", 80, ""),
        "http://example.com"
    );
    assert_eq!(
        build_url("https", "example.com", 443, ""),
        "https://example.com"
    );
    assert_eq!(
        build_url("ws", "example.com", 80, "/x"),
        "ws://example.com/x"
    );
}

#[test]
fn build_url_keeps_non_default_ports() {
    assert_eq!(
        build_url("https", "203.0.113.5", 8443, "/fs?id=1"),
        "https://203.0.113.5:8443/fs?id=1"
    );
}
