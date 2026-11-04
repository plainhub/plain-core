// Port of the Go implementation `internal/pkg/shortid`.

use rand::RngCore;

const ALPHABET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

pub fn new_id() -> String {
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    // UUID v4 marker bits preserve the existing format and leave 122 random bits.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    encode(&bytes)
}

fn encode(uuid_bytes: &[u8]) -> String {
    let mut num: Vec<u32> = vec![0]; // little-endian big-int
    for &b in uuid_bytes {
        let mut carry = b as u32;
        for digit in num.iter_mut() {
            let v = (*digit << 8) | carry;
            *digit = v % 57;
            carry = v / 57;
        }
        while carry > 0 {
            num.push(carry % 57);
            carry /= 57;
        }
    }
    // Big-endian to readable string (most significant first)
    let mut encoded = String::new();
    for d in num.iter().rev() {
        encoded.push(ALPHABET[*d as usize] as char);
    }
    if encoded.len() < 22 {
        let pad = ALPHABET[0] as char;
        let mut s = String::new();
        for _ in 0..(22 - encoded.len()) {
            s.push(pad);
        }
        s.push_str(&encoded);
        encoded = s;
    }
    encoded
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/utils/shortid.rs"]
mod tests;
