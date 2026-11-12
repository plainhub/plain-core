use rand_core::{CryptoRng, RngCore};
pub struct Stream {
    state: u64,
    pub bytes: usize,
}
impl Stream {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed,
            bytes: 0,
        }
    }
}
impl CryptoRng for Stream {}
impl RngCore for Stream {
    fn next_u32(&mut self) -> u32 {
        let mut b = [0; 4];
        self.fill_bytes(&mut b);
        u32::from_le_bytes(b)
    }
    fn next_u64(&mut self) -> u64 {
        let mut b = [0; 8];
        self.fill_bytes(&mut b);
        u64::from_le_bytes(b)
    }
    fn fill_bytes(&mut self, dst: &mut [u8]) {
        for b in dst.iter_mut() {
            self.state ^= self.state << 13;
            self.state ^= self.state >> 7;
            self.state ^= self.state << 17;
            *b = self.state as u8;
        }
        self.bytes += dst.len();
    }
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dst);
        Ok(())
    }
}
pub fn decode<const N: usize>(hex: &str) -> [u8; N] {
    assert_eq!(hex.len(), N * 2);
    let mut bytes = [0; N];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap();
    }
    bytes
}
