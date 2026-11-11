mod table;
use self::table::{DECODE_TABLE, ENCODE_TABLE};
use crate::h2::hpack::DecoderError;
use bytes::{BufMut, BytesMut};
const BRANCH: u16 = 0x8000;
const TABLE_INDEX_MASK: u16 = 0x7f00;
const TABLE_WIDTH: usize = 256;
pub fn decode(src: &[u8], buf: &mut BytesMut) -> Result<BytesMut, DecoderError> {
    buf.reserve(src.len() << 1);
    let mut table = 0;
    let mut acc = 0u32;
    let mut bits = 0;
    for &byte in src {
        acc = (acc << 8) | byte as u32;
        bits += 8;
        while bits >= 8 {
            let index = (acc >> (bits - 8)) as u8 as usize;
            let entry = DECODE_TABLE[table * TABLE_WIDTH + index];
            if entry & BRANCH == 0 {
                buf.put_u8(entry as u8);
                table = 0;
                bits -= (entry >> 8) as usize;
            } else {
                table = ((entry & TABLE_INDEX_MASK) >> 8) as usize;
                if table == 0 {
                    return Err(DecoderError::InvalidHuffmanCode);
                }
                bits -= 8;
            }
        }
    }
    while bits > 0 {
        debug_assert!(bits < 8);
        let padding = (1u32 << bits) - 1;
        if table == 0 && acc & padding == padding {
            break;
        }
        let index = (acc << (8 - bits)) as u8 as usize;
        let entry = DECODE_TABLE[table * TABLE_WIDTH + index];
        if entry & BRANCH != 0 {
            return Err(DecoderError::InvalidHuffmanCode);
        }
        let used = (entry >> 8) as usize;
        if used > bits {
            return Err(DecoderError::InvalidHuffmanCode);
        }
        buf.put_u8(entry as u8);
        table = 0;
        bits -= used;
    }
    if table == 0 {
        Ok(buf.split())
    } else {
        Err(DecoderError::InvalidHuffmanCode)
    }
}
pub fn encode(src: &[u8], dst: &mut BytesMut) {
    let mut bits: u64 = 0;
    let mut bits_left = 40;
    for &b in src {
        let (nbits, code) = ENCODE_TABLE[b as usize];
        bits |= code << (bits_left - nbits);
        bits_left -= nbits;
        while bits_left <= 32 {
            dst.put_u8((bits >> 32) as u8);
            bits <<= 8;
            bits_left += 8;
        }
    }
    if bits_left != 40 {
        bits |= (1 << bits_left) - 1;
        dst.put_u8((bits >> 32) as u8);
    }
}
