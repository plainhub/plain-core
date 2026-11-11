pub(crate) mod adler32;
pub(crate) mod crc32;
#[derive(Clone, Copy, Debug)]
pub(crate) struct Adler32(u32);
impl Adler32 {
    pub(crate) fn new() -> Self {
        Self(1)
    }
    pub(crate) fn write(&mut self, bytes: &[u8]) {
        self.0 = crate::checksum::adler32::adler32(self.0, bytes);
    }
    pub(crate) fn finish(&self) -> u32 {
        self.0
    }
}
#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/checksum.rs"]
mod tests;
