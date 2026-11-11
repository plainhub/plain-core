#[cfg(target_arch = "x86_64")]
mod avx2;
mod generic;
#[cfg(target_arch = "aarch64")]
mod neon;
#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
mod wasm;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod x86;
pub fn adler32(start_checksum: u32, data: &[u8]) -> u32 {
    #[cfg(target_arch = "x86_64")]
    if crate::cpu_features::is_enabled_avx2_and_bmi2() {
        return avx2::adler32_avx2(start_checksum, data);
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if let Some(checksum) = x86::update(start_checksum, data) {
        return checksum;
    }
    #[cfg(target_arch = "aarch64")]
    if crate::cpu_features::is_enabled_neon() {
        return self::neon::adler32_neon(start_checksum, data);
    }
    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    if crate::cpu_features::is_enabled_simd128() {
        return self::wasm::adler32_wasm(start_checksum, data);
    }
    generic::adler32_rust(start_checksum, data)
}
pub(crate) fn adler32_fold_copy(start_checksum: u32, dst: &mut [u8], src: &[u8]) -> u32 {
    debug_assert!(dst.len() >= src.len(), "{} < {}", dst.len(), src.len());
    dst[..src.len()].copy_from_slice(src);
    adler32(start_checksum, src)
}
const BASE: u32 = 65521;
const NMAX: u32 = 5552;
