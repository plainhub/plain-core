use std::borrow::Cow;

pub(super) enum ContentDispositionAttr {
    Name,
    FileName,
}

fn trim_ascii_ws_start(bytes: &[u8]) -> &[u8] {
    bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .map_or_else(|| &bytes[bytes.len()..], |i| &bytes[i..])
}

fn trim_ascii_ws_then(bytes: &[u8], char: u8) -> Option<&[u8]> {
    match trim_ascii_ws_start(bytes) {
        [first, rest @ ..] if *first == char => Some(rest),
        _ => None,
    }
}

impl ContentDispositionAttr {
    pub(super) fn extract_from<'h>(&self, mut header: &'h [u8]) -> Option<Cow<'h, str>> {
        let prefix = match self {
            ContentDispositionAttr::Name => &b"name"[..],
            ContentDispositionAttr::FileName => &b"filename"[..],
        };

        while let Some(i) = crate::memchr::memmem::find(header, prefix) {
            // Check if we found a superstring of `prefix`; continue if so.
            let suffix = &header[(i + prefix.len())..];
            if i > 0 && !(header[i - 1].is_ascii_whitespace() || header[i - 1] == b';') {
                header = suffix;
                continue;
            }

            // Now find and trim the `=`. Handle quoted strings first.
            let rest = trim_ascii_ws_then(suffix, b'=')?;
            let (bytes, is_escaped) = if let Some(rest) = trim_ascii_ws_then(rest, b'"') {
                let (mut k, mut escaped) = (crate::memchr::memchr(b'"', rest)?, false);
                while k > 0 && rest[k - 1] == b'\\' {
                    escaped = true;
                    k = k + 1 + crate::memchr::memchr(b'"', &rest[(k + 1)..])?;
                }

                (&rest[..k], escaped)
            } else {
                let rest = trim_ascii_ws_start(rest);
                let j = crate::memchr::memchr2(b';', b' ', rest).unwrap_or(rest.len());
                (&rest[..j], false)
            };

            return match std::str::from_utf8(bytes).ok()? {
                name if is_escaped => Some(name.replace(r#"\""#, "\"").into()),
                name => Some(name.into()),
            };
        }

        None
    }
}
