use std::collections::HashMap;

/// Percent-decode a string. Replaces every `+` with a space and every
/// `%XX` byte triplet with the matching raw byte. Invalid sequences
/// (truncated `%` or non-hex digits) are passed through verbatim.
///
/// Hand-rolled to avoid pulling in the `percent-encoding` crate just
/// for a few call sites in the local server and HTTP proxy.
pub fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_nibble(bytes[i + 1]);
                let lo = hex_nibble(bytes[i + 2]);
                match (hi, lo) {
                    (Some(h), Some(l)) => {
                        out.push((h << 4) | l);
                        i += 3;
                    }
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Parse a `key=value&key=value` query string into a `HashMap`. Both
/// keys and values are percent-decoded (so `id=foo%20bar` becomes
/// `foo bar`, and `+` becomes a space).
pub fn parse_query(q: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for kv in q.split('&') {
        if kv.is_empty() {
            continue;
        }
        let (k, v) = match kv.split_once('=') {
            Some(pair) => pair,
            None => (kv, ""),
        };
        out.insert(percent_decode(k), percent_decode(v));
    }
    out
}

/// Look up a single query parameter from a path like `/foo?a=1&b=2`.
/// Returns the percent-decoded value, or `None` if the key is missing
/// or the path has no query string.
pub fn query_get(path: &str, key: &str) -> Option<String> {
    let (_, query) = path.split_once('?')?;
    parse_query(query).remove(key)
}

/// Percent-encode a string for use in a URL component. Mirrors plain-app's
/// `String.urlEncode()` (`lib/extensions/UrlEncode.kt`): unreserved chars
/// per RFC 3986 (`A–Z a–z 0–9 - _ . ~`) are passed through; every other
/// byte (the input is already UTF-8) is encoded as `%XX` with uppercase
/// hex. Used for `Content-Disposition` filenames and query-param values.
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/utils/query.rs"]
mod tests;
