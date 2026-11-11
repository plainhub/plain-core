mod kernel;
pub(super) fn adler32_neon(start: u32, bytes: &[u8]) -> u32 {
    let (a, b) = unsafe { kernel::update(start as u16, (start >> 16) as u16, bytes) };
    (u32::from(b) << 16) | u32::from(a)
}
