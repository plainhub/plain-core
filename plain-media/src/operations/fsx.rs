//! Filesystem-related helpers that don't deserve their own crate and that
//! we'd rather not pull in a third-party dependency for. Mirrors bits of
//! `internal/pkg/pathx` and the per-op resolvers in
//! `internal/graph/files_*_api.go` + `internal/graph/helpers_local.go` from
//! the Go side.
//!
//! Inlined here: `percent_decode_path` (used in HTTP query params).
//! Inlined elsewhere: per-file-id short id, mount table parsing, etc.

pub fn percent_decode_path(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Map a file extension to an IANA MIME type. Falls back to
/// `application/octet-stream` for anything we don't recognise.
///
/// We don't try to be comprehensive — the Go side uses Go's `mime`
/// package which carries ~1000 entries, but for the server front-end the
/// ~50 entries below cover 99% of served files. Adding the rest is
/// trivial when the need arises.
pub fn guess_mime(path: &std::path::Path) -> String {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let mime = plain_rs::utils::mime::mime_from_ext(filename);
    if mime.starts_with("text/") {
        format!("{mime}; charset=utf-8")
    } else {
        mime.to_string()
    }
}

pub use plain_rs::files::filesystem::image_format::is_animated_image_or_svg;

// ---------------------------------------------------------------------------
// File operations
//
// The functions below are 1:1 ports of the resolvers in
// `internal/graph/files_*_api.go` and the helpers in
// `internal/graph/helpers_local.go` + `helpers/files_helper.go` from the
// Go side. They are async because every caller in the GraphQL resolvers
// is `async fn` and we want the syscall work to run on tokio's blocking
// pool (via `tokio::fs::File`) rather than blocking the reactor.
// ---------------------------------------------------------------------------

pub use plain_rs::files::filesystem::*;

#[cfg(test)]
use plain_rs::files::filesystem::count_dir_entries_fast;

#[cfg(test)]
#[path = "../../tests/unit/operations/fsx.rs"]
mod tests;
