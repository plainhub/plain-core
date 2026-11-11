use crate::checksum::crc32::Hasher;
pub(crate) fn crc32(start: u32, bytes: &[u8]) -> u32 {
    let mut crc = Hasher::new_with_initial(start);
    crc.update(bytes);
    crc.finalize()
}
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Crc32Fold(u32);
impl Crc32Fold {
    pub(crate) const fn new() -> Self {
        Self(0)
    }
    pub(crate) fn fold(&mut self, bytes: &[u8], _start: u32) {
        self.0 = crc32(self.0, bytes);
    }
    pub(crate) fn fold_copy(&mut self, dst: &mut [u8], src: &[u8]) {
        self.fold(src, 0);
        dst[..src.len()].copy_from_slice(src);
    }
    pub(crate) fn finish(self) -> u32 {
        self.0
    }
}
