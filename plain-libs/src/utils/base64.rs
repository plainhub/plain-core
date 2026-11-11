//! Base64 (RFC 4648) in both alphabets.
//!
//! Two alphabets exist because two kinds of payload travel through the app:
//! `+/` with padding for bytes (tokens, keys, image blobs) and `-_` for
//! values that go into URLs. Decoding accepts either — a string containing
//! `-`/`_` is URL-safe, one containing `+`/`/` is standard, and one with
//! neither decodes the same either way. Mixing both in one string is an
//! error rather than a guess.

/// A rejected string: the first bad character and what it was.
#[derive(Debug, PartialEq, Eq)]
pub struct Invalid {
    pub offset: usize,
    pub found: char,
}

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid base64 character {:?} at {}",
            self.found, self.offset
        )
    }
}

impl std::error::Error for Invalid {}

const STANDARD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL_SAFE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

#[inline]
fn append_triple(out: &mut Vec<u8>, bytes: [u8; 3], alphabet: &[u8; 64]) {
    let [a, b, c] = bytes;
    out.extend_from_slice(&[
        alphabet[usize::from(a >> 2)],
        alphabet[usize::from((a & 3) << 4 | b >> 4)],
        alphabet[usize::from((b & 15) << 2 | c >> 6)],
        alphabet[usize::from(c & 63)],
    ]);
}

#[inline]
fn encode_segments(out: &mut Vec<u8>, segments: &[&[u8]], alphabet: &[u8; 64]) {
    let mut tail = [0; 3];
    let mut count = 0;
    for &segment in segments {
        let mut bytes = segment;
        while count != 0 && !bytes.is_empty() {
            tail[count] = bytes[0];
            count += 1;
            bytes = &bytes[1..];
            if count == 3 {
                append_triple(out, tail, alphabet);
                count = 0;
            }
        }
        let (chunks, remainder) = bytes.as_chunks::<3>();
        for &chunk in chunks {
            append_triple(out, chunk, alphabet);
        }
        if !remainder.is_empty() {
            tail[..remainder.len()].copy_from_slice(remainder);
            count = remainder.len();
        }
    }
    if count != 0 {
        out.extend_from_slice(&[
            alphabet[usize::from(tail[0] >> 2)],
            alphabet[usize::from((tail[0] & 3) << 4 | if count == 2 { tail[1] >> 4 } else { 0 })],
            if count == 2 {
                alphabet[usize::from((tail[1] & 15) << 2)]
            } else {
                b'='
            },
            b'=',
        ]);
    }
}

pub(crate) fn base64_encode_append_segments(out: &mut Vec<u8>, segments: &[&[u8]]) {
    encode_segments(out, segments, STANDARD);
}

fn encode(bytes: &[u8], alphabet: &[u8; 64]) -> String {
    let mut out = Vec::with_capacity(bytes.len().div_ceil(3) * 4);
    encode_segments(&mut out, &[bytes], alphabet);
    String::from_utf8(out).expect("Base64 alphabet is ASCII")
}

/// Standard alphabet (`+` / `/`), padded with `=`.
pub fn base64_encode(bytes: &[u8]) -> String {
    encode(bytes, STANDARD)
}

/// URL-safe alphabet (`-` / `_`), padded with `=`.
pub fn base64_encode_url_safe(bytes: &[u8]) -> String {
    encode(bytes, URL_SAFE)
}

/// Decodes either alphabet, with or without `=` padding.
///
/// Padding is optional because plenty of producers drop it. A length of
/// `4n + 1` can never be valid and is rejected.
pub fn base64_decode_checked(input: &str) -> Result<Vec<u8>, Invalid> {
    decode_checked(input, false, false)
}

pub fn base64_decode_standard(input: &[u8]) -> Result<Vec<u8>, Invalid> {
    let text = std::str::from_utf8(input).map_err(|error| Invalid {
        offset: error.valid_up_to(),
        found: char::from(input[error.valid_up_to()]),
    })?;
    decode_checked(text, true, true)
}

#[cfg(feature = "ai-image-search")]
pub(crate) fn base64_decode_standard_optional_padding(input: &str) -> Result<Vec<u8>, Invalid> {
    decode_checked(input, true, false)
}

const DECODE: [u8; 256] = {
    let mut table = [255; 256];
    let mut index = 0;
    while index < 64 {
        table[STANDARD[index] as usize] = index as u8;
        index += 1;
    }
    table[b'+' as usize] |= 128;
    table[b'/' as usize] |= 128;
    table[b'-' as usize] = 62 | 64;
    table[b'_' as usize] = 63 | 64;
    table
};

fn decode_checked(input: &str, standard: bool, require_padding: bool) -> Result<Vec<u8>, Invalid> {
    let body = input.trim_end_matches('=');
    if standard {
        let padding = input.len() - body.len();
        if (require_padding && input.len() % 4 != 0)
            || padding > 2
            || (padding != 0 && (body.len() % 4 < 2 || body.len() % 4 + padding > 4))
        {
            return Err(Invalid {
                offset: body.len(),
                found: '=',
            });
        }
    }
    if let Some(offset) = body.find('=') {
        // Padding only ever belongs at the end.
        return Err(Invalid { offset, found: '=' });
    }
    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    let mut alphabet = 0_u8;
    let invalid_mask = if standard { 64 } else { 192 };
    let (chunks, remainder) = body.as_bytes().as_chunks::<4>();
    for (block, bytes) in chunks.iter().enumerate() {
        let values = bytes.map(|byte| DECODE[usize::from(byte)]);
        let combined = alphabet | values[0] | values[1] | values[2] | values[3];
        if combined & invalid_mask == invalid_mask {
            for (index, &value) in values.iter().enumerate() {
                alphabet |= value & 192;
                if value == 255 || alphabet & invalid_mask == invalid_mask {
                    let offset = block * 4 + index;
                    return Err(Invalid {
                        offset,
                        found: body[offset..].chars().next().unwrap(),
                    });
                }
            }
        }
        alphabet = combined & 192;
        let [a, b, c, d] = values.map(|value| value & 63);
        out.extend_from_slice(&[(a << 2) | (b >> 4), (b << 4) | (c >> 2), (c << 6) | d]);
    }
    let mut tail = [0_u8; 3];
    for (index, &byte) in remainder.iter().enumerate() {
        let value = DECODE[usize::from(byte)];
        alphabet |= value & 192;
        if value == 255 || alphabet & invalid_mask == invalid_mask {
            let offset = chunks.len() * 4 + index;
            return Err(Invalid {
                offset,
                found: body[offset..].chars().next().unwrap(),
            });
        }
        tail[index] = value & 63;
    }
    if remainder.len() == 1 {
        return Err(Invalid {
            offset: body.len(),
            found: '\0',
        });
    }
    if standard && !remainder.is_empty() {
        let mask = if remainder.len() == 2 { 15 } else { 3 };
        if tail[remainder.len() - 1] & mask != 0 {
            return Err(Invalid {
                offset: body.len() - 1,
                found: char::from(body.as_bytes()[body.len() - 1]),
            });
        }
    }
    if remainder.len() >= 2 {
        out.push((tail[0] << 2) | (tail[1] >> 4));
        if remainder.len() == 3 {
            out.push((tail[1] << 4) | (tail[2] >> 2));
        }
    }
    Ok(out)
}

/// Convenience wrapper for call sites that only need the bytes: an invalid
/// string decodes to nothing. Use [`base64_decode_checked`] when "this input
/// was garbage" has to be distinguishable from "this input was empty".
pub fn base64_decode(input: &str) -> Vec<u8> {
    base64_decode_checked(input).unwrap_or_default()
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/utils/base64.rs"]
mod tests;
