/// Outcome of parsing an HTTP `Range` header against a representation size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeParse {
    /// No usable `Range` header (absent, non-`bytes` unit, or malformed) —
    /// serve the full 200 response, per RFC 7233 §3.1 ("an origin server
    /// MUST ignore a Range header field that contains a range unit it does
    /// not understand" and malformed ranges are likewise ignored).
    Full,
    /// A satisfiable byte range: inclusive `(start, end)`.
    Partial(u64, u64),
    /// Syntactically valid but unsatisfiable (start ≥ size, or a zero-length
    /// suffix) — respond 416 with `Content-Range: bytes */size`, per
    /// RFC 7233 §4.4.
    Unsatisfiable,
}

/// Parse an HTTP `Range` header (RFC 7233 §2.1) against a file size.
///
/// Supported forms:
///   * `bytes=0-499`   → first 500 bytes
///   * `bytes=500-`    → from byte 500 to end
///   * `bytes=-500`    → last 500 bytes
///
/// Multi-range requests (`bytes=0-10,20-30`) are not supported — the
/// first range is served and the rest ignored, which keeps downloads
/// working without the multipart/byteranges dance.
pub fn parse_range_header(range: &str, file_size: u64) -> RangeParse {
    use RangeParse::*;
    let Some(spec) = range.strip_prefix("bytes=") else {
        return Full;
    };
    let Some(spec) = spec.split(',').next() else {
        return Full;
    };
    let spec = spec.trim();
    let Some((start_s, end_s)) = spec.split_once('-') else {
        return Full;
    };
    let start_s = start_s.trim();
    let end_s = end_s.trim();
    if file_size == 0 {
        // Nothing can be served partially; a 200 with an empty body is the
        // least surprising response and matches the pre-tri-state behavior.
        return Full;
    }
    match (start_s.is_empty(), end_s.is_empty()) {
        (false, false) => {
            let (Ok(start), Ok(end)) = (start_s.parse::<u64>(), end_s.parse::<u64>()) else {
                return Full;
            };
            if start > end {
                // Inverted range: invalid per RFC 7233 §2.1, ignore the header.
                return Full;
            }
            if start >= file_size {
                return Unsatisfiable;
            }
            Partial(start, end.min(file_size - 1))
        }
        (false, true) => {
            let Ok(start) = start_s.parse::<u64>() else {
                return Full;
            };
            if start >= file_size {
                return Unsatisfiable;
            }
            Partial(start, file_size - 1)
        }
        (true, false) => {
            let Ok(n) = end_s.parse::<u64>() else {
                return Full;
            };
            if n == 0 {
                return Unsatisfiable;
            }
            Partial(file_size.saturating_sub(n), file_size - 1)
        }
        (true, true) => Full, // "bytes=-": malformed, ignore
    }
}

/// Build a `Content-Disposition` header value (RFC 6266 / RFC 5987).
/// `kind` is `"inline"` or `"attachment"`; the filename is URL-encoded for
/// both the legacy `filename="…"` and the `filename*=utf-8''…` forms —
/// matching plain-app exactly so non-ASCII names round-trip correctly.
pub fn content_disposition(kind: &str, filename: &str) -> String {
    let encoded = crate::utils::query::url_encode(filename);
    format!("{kind}; filename=\"{encoded}\"; filename*=utf-8''{encoded}")
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/utils/http.rs"]
mod tests;
