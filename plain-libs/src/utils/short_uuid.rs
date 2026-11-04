use rand::RngCore;

// Preserves the existing 32-bit UUID text prefix contract; not a secret token.
pub fn short_uuid() -> String {
    let mut bytes = [0; 4];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    encode_base36(u64::from_be_bytes(format_prefix(&bytes)))
}

// Adapted from uuid 1.26.1 src/fmt.rs; MIT license in licenses/uuid-MIT.txt.
const fn format_prefix(src: &[u8; 4]) -> [u8; 8] {
    let mut dst = [0; 8];
    let mut i = 0;
    while i < 4 {
        let x = src[i];
        dst[i * 2] = nibble_to_hex(x >> 4);
        dst[i * 2 + 1] = nibble_to_hex(x & 0x0f);
        i += 1;
    }
    dst
}

const fn nibble_to_hex(nibble: u8) -> u8 {
    nibble + b'0' + if nibble > 9 { 0x27 } else { 0 }
}

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
