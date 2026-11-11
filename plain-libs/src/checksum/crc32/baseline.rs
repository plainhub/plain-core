use crate::checksum::crc32::table::CRC32_TABLE;
#[derive(Clone)]
pub struct State {
    state: u32,
}
impl State {
    pub fn new(state: u32) -> Self {
        State { state }
    }
    pub fn update(&mut self, buf: &[u8]) {
        self.state = update_fast_16(self.state, buf);
    }
    pub fn finalize(self) -> u32 {
        self.state
    }
    pub fn reset(&mut self) {
        self.state = 0;
    }
}
#[inline(always)]
fn fold_16(crc: u32, block: &[u8]) -> u32 {
    let w0 = u32::from_le_bytes([block[0], block[1], block[2], block[3]]) ^ crc;
    CRC32_TABLE[0x0][block[0xf] as usize]
        ^ CRC32_TABLE[0x1][block[0xe] as usize]
        ^ CRC32_TABLE[0x2][block[0xd] as usize]
        ^ CRC32_TABLE[0x3][block[0xc] as usize]
        ^ CRC32_TABLE[0x4][block[0xb] as usize]
        ^ CRC32_TABLE[0x5][block[0xa] as usize]
        ^ CRC32_TABLE[0x6][block[0x9] as usize]
        ^ CRC32_TABLE[0x7][block[0x8] as usize]
        ^ CRC32_TABLE[0x8][block[0x7] as usize]
        ^ CRC32_TABLE[0x9][block[0x6] as usize]
        ^ CRC32_TABLE[0xa][block[0x5] as usize]
        ^ CRC32_TABLE[0xb][block[0x4] as usize]
        ^ CRC32_TABLE[0xc][(w0 >> 24) as usize]
        ^ CRC32_TABLE[0xd][((w0 >> 16) & 0xFF) as usize]
        ^ CRC32_TABLE[0xe][((w0 >> 8) & 0xFF) as usize]
        ^ CRC32_TABLE[0xf][(w0 & 0xFF) as usize]
}
pub(crate) fn update_fast_16(prev: u32, mut buf: &[u8]) -> u32 {
    const UNROLL: usize = 4;
    const BYTES_AT_ONCE: usize = 16 * UNROLL;
    let mut crc = !prev;
    while buf.len() >= BYTES_AT_ONCE {
        for _ in 0..UNROLL {
            crc = fold_16(crc, buf);
            buf = &buf[16..];
        }
    }
    while buf.len() >= 16 {
        crc = fold_16(crc, buf);
        buf = &buf[16..];
    }
    update_slow(!crc, buf)
}
pub(crate) fn update_slow(prev: u32, buf: &[u8]) -> u32 {
    let mut crc = !prev;
    for &byte in buf.iter() {
        crc = CRC32_TABLE[0][((crc as u8) ^ byte) as usize] ^ (crc >> 8);
    }
    !crc
}
