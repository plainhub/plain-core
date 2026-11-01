/// Generate a short, human-friendly unique ID derived from a UUID v4 string.
///
/// Mirrors the Kotlin `StringHelper.shortUUID()` used in `plain-app`:
/// take a fresh UUID v4, read the first 8 bytes of its string form as a
/// big-endian `u64`, and render it as a base-36 (`Character.MAX_RADIX`)
/// number.
pub fn short_uuid() -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let bytes = id.as_bytes();
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[..8]);
    let n = u64::from_be_bytes(buf);
    encode_base36(n)
}

/// Render a `u64` as an unsigned base-36 string (digits `0-9`, `a-z`).
/// Matches Java's `Long.toString(Character.MAX_RADIX)`.
fn encode_base36(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut buf = Vec::with_capacity(13);
    while n > 0 {
        let digit = (n % 36) as u32;
        buf.push(std::char::from_digit(digit, 36).unwrap());
        n /= 36;
    }
    buf.reverse();
    buf.into_iter().collect()
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/utils/short_uuid.rs"]
mod tests;
