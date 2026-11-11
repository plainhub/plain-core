pub fn slide_hash(state: &mut crate::compression::backend::deflate::State) {
    let wsize = state.w_size as u16;
    slide_hash_chain(state.head.as_mut_slice(), wsize);
    slide_hash_chain(state.prev.as_mut_slice(), wsize);
}
fn slide_hash_chain(table: &mut [u16], wsize: u16) {
    #[cfg(target_arch = "x86_64")]
    if crate::cpu_features::is_enabled_avx2_and_bmi2() {
        return unsafe { avx2::slide_hash_chain(table, wsize) };
    }
    #[cfg(target_arch = "aarch64")]
    if crate::cpu_features::is_enabled_neon() {
        return unsafe { neon::slide_hash_chain(table, wsize) };
    }
    #[cfg(target_arch = "wasm32")]
    if crate::cpu_features::is_enabled_simd128() {
        return unsafe { wasm::slide_hash_chain(table, wsize) };
    }
    rust::slide_hash_chain(table, wsize);
}
#[inline(always)]
fn generic_slide_hash_chain<const N: usize>(table: &mut [u16], wsize: u16) {
    debug_assert_eq!(table.len() % N, 0);
    for chunk in table.chunks_exact_mut(N) {
        for m in chunk.iter_mut() {
            *m = m.saturating_sub(wsize);
        }
    }
}
mod rust {
    pub fn slide_hash_chain(table: &mut [u16], wsize: u16) {
        super::generic_slide_hash_chain::<32>(table, wsize);
    }
}
#[cfg(target_arch = "x86_64")]
mod avx2 {
    #[target_feature(enable = "avx2")]
    #[target_feature(enable = "bmi2")]
    #[target_feature(enable = "bmi1")]
    pub unsafe fn slide_hash_chain(table: &mut [u16], wsize: u16) {
        unsafe {
            super::generic_slide_hash_chain::<64>(table, wsize);
        }
    }
}
#[cfg(target_arch = "aarch64")]
mod neon {
    #[target_feature(enable = "neon")]
    pub unsafe fn slide_hash_chain(table: &mut [u16], wsize: u16) {
        {
            super::generic_slide_hash_chain::<32>(table, wsize);
        }
    }
}
#[cfg(target_arch = "wasm32")]
mod wasm {
    #[target_feature(enable = "simd128")]
    pub unsafe fn slide_hash_chain(table: &mut [u16], wsize: u16) {
        unsafe {
            super::generic_slide_hash_chain::<32>(table, wsize);
        }
    }
}
