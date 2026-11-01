use super::*;
use RangeParse::{Full, Partial, Unsatisfiable};

#[test]
fn parse_range_start_end() {
    assert_eq!(parse_range_header("bytes=0-499", 1000), Partial(0, 499));
    assert_eq!(parse_range_header("bytes=100-199", 1000), Partial(100, 199));
}

#[test]
fn parse_range_open_end() {
    assert_eq!(parse_range_header("bytes=500-", 1000), Partial(500, 999));
    assert_eq!(parse_range_header("bytes=0-", 1000), Partial(0, 999));
}

#[test]
fn parse_range_suffix() {
    assert_eq!(parse_range_header("bytes=-500", 1000), Partial(500, 999));
    assert_eq!(parse_range_header("bytes=-2000", 1000), Partial(0, 999));
}

#[test]
fn parse_range_clamps_end_to_file_size() {
    assert_eq!(
        parse_range_header("bytes=900-2000", 1000),
        Partial(900, 999)
    );
}

#[test]
fn parse_range_out_of_bounds_is_unsatisfiable() {
    // RFC 7233 §4.4: syntactically valid but beyond EOF → 416 material.
    assert_eq!(parse_range_header("bytes=1000-", 1000), Unsatisfiable);
    assert_eq!(parse_range_header("bytes=2000-3000", 1000), Unsatisfiable);
    assert_eq!(parse_range_header("bytes=-0", 1000), Unsatisfiable);
    assert_eq!(parse_range_header("bytes=999-", 1000), Partial(999, 999));
}

#[test]
fn parse_range_malformed_is_full() {
    // Unknown units, unparsable numbers and inverted ranges are ignored
    // (RFC 7233 §3.1) — the caller serves the whole representation.
    assert_eq!(parse_range_header("", 1000), Full);
    assert_eq!(parse_range_header("items=0-10", 1000), Full);
    assert_eq!(parse_range_header("bytes=abc-def", 1000), Full);
    assert_eq!(parse_range_header("bytes=-", 1000), Full);
    assert_eq!(parse_range_header("bytes=5-2", 1000), Full);
    assert_eq!(parse_range_header("bytes=0-10", 0), Full);
}

#[test]
fn parse_range_multi_range_serves_first() {
    assert_eq!(parse_range_header("bytes=0-10,20-30", 1000), Partial(0, 10));
}

#[test]
fn content_disposition_inline_and_attachment() {
    assert_eq!(
        content_disposition("inline", "cat.jpg"),
        "inline; filename=\"cat.jpg\"; filename*=utf-8''cat.jpg"
    );
    assert_eq!(
        content_disposition("attachment", "r port.pdf"),
        "attachment; filename=\"r%20port.pdf\"; filename*=utf-8''r%20port.pdf"
    );
}

#[test]
fn content_disposition_encodes_non_ascii() {
    assert_eq!(
        content_disposition("inline", "中文.pdf"),
        "inline; filename=\"%E4%B8%AD%E6%96%87.pdf\"; filename*=utf-8''%E4%B8%AD%E6%96%87.pdf"
    );
}
