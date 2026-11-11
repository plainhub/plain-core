pub(super) mod avx2;
pub(super) mod sse2;
pub(super) mod ssse3;
type Adler32Imp = fn(u16, u16, &[u8]) -> (u16, u16);
#[inline]
const fn mm_shuffle(z: u32, y: u32, x: u32, w: u32) -> i32 {
    ((z << 6) | (y << 4) | (x << 2) | w) as i32
}
pub(super) fn update(start: u32, data: &[u8]) -> Option<u32> {
    let implementation = avx2::get_imp()
        .or_else(ssse3::get_imp)
        .or_else(sse2::get_imp)?;
    let (a, b) = implementation(start as u16, (start >> 16) as u16, data);
    Some((u32::from(b) << 16) | u32::from(a))
}
