use crate::httparse::{Bytes, is_header_name_token, is_header_value_token, is_uri_token};
const BLOCK_SIZE: usize = core::mem::size_of::<usize>();
type ByteBlock = [u8; BLOCK_SIZE];
#[inline]
pub fn match_uri_vectored(bytes: &mut Bytes) {
    loop {
        if let Some(bytes8) = bytes.peek_n::<ByteBlock>(BLOCK_SIZE) {
            let n = match_uri_char_8_swar(bytes8);
            unsafe {
                bytes.advance(n);
            }
            if n == BLOCK_SIZE {
                continue;
            }
        }
        if let Some(b) = bytes.peek() {
            if is_uri_token(b) {
                unsafe {
                    bytes.advance(1);
                }
                continue;
            }
        }
        break;
    }
}
#[inline]
pub fn match_header_value_vectored(bytes: &mut Bytes) {
    loop {
        if let Some(bytes8) = bytes.peek_n::<ByteBlock>(BLOCK_SIZE) {
            let n = match_header_value_char_8_swar(bytes8);
            unsafe {
                bytes.advance(n);
            }
            if n == BLOCK_SIZE {
                continue;
            }
        }
        if let Some(b) = bytes.peek() {
            if is_header_value_token(b) {
                unsafe {
                    bytes.advance(1);
                }
                continue;
            }
        }
        break;
    }
}
#[inline]
pub fn match_header_name_vectored(bytes: &mut Bytes) {
    while let Some(block) = bytes.peek_n::<ByteBlock>(BLOCK_SIZE) {
        let n = match_block(is_header_name_token, block);
        unsafe {
            bytes.advance(n);
        }
        if n != BLOCK_SIZE {
            return;
        }
    }
    unsafe { bytes.advance(match_tail(is_header_name_token, bytes.as_ref())) };
}
#[cold]
#[inline]
fn match_tail(f: impl Fn(u8) -> bool, bytes: &[u8]) -> usize {
    for (i, &b) in bytes.iter().enumerate() {
        if !f(b) {
            return i;
        }
    }
    bytes.len()
}
#[inline(always)]
fn match_block(f: impl Fn(u8) -> bool, block: ByteBlock) -> usize {
    for (i, &b) in block.iter().enumerate() {
        if !f(b) {
            return i;
        }
    }
    BLOCK_SIZE
}
const fn uniform_block(b: u8) -> usize {
    (b as u64 * 0x01_01_01_01_01_01_01_01) as usize
}
#[inline]
fn match_uri_char_8_swar(block: ByteBlock) -> usize {
    const M: u8 = 0x21;
    const BM: usize = uniform_block(M);
    const ONE: usize = uniform_block(0x01);
    const DEL: usize = uniform_block(0x7f);
    const M128: usize = uniform_block(128);
    let x = usize::from_ne_bytes(block);
    let lt = x.wrapping_sub(BM) & !x;
    let xor_del = x ^ DEL;
    let eq_del = xor_del.wrapping_sub(ONE) & !xor_del;
    offsetnz((lt | eq_del) & M128)
}
#[inline]
fn match_header_value_char_8_swar(block: ByteBlock) -> usize {
    const M: u8 = 0x20;
    const BM: usize = uniform_block(M);
    const ONE: usize = uniform_block(0x01);
    const DEL: usize = uniform_block(0x7f);
    const M128: usize = uniform_block(128);
    let x = usize::from_ne_bytes(block);
    let lt = x.wrapping_sub(BM) & !x;
    let xor_del = x ^ DEL;
    let eq_del = xor_del.wrapping_sub(ONE) & !xor_del;
    offsetnz((lt | eq_del) & M128)
}
#[inline]
fn offsetnz(block: usize) -> usize {
    if block == 0 {
        return BLOCK_SIZE;
    }
    for (i, b) in block.to_ne_bytes().iter().copied().enumerate() {
        if b != 0 {
            return i;
        }
    }
    unreachable!()
}
