// CPU detection retained from zlib-rs 0.6.8; see compression/backend/SOURCE.json.
pub struct CpuFeatures;
impl CpuFeatures {
    pub const NONE: usize = 0;
    pub const AVX2: usize = 1;
}

#[inline(always)]
pub fn is_enabled_avx2_and_bmi2() -> bool {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    {
        #[cfg(all(
            target_feature = "avx2",
            target_feature = "bmi1",
            target_feature = "bmi2"
        ))]
        return true;
        #[cfg(not(all(
            target_feature = "avx2",
            target_feature = "bmi1",
            target_feature = "bmi2"
        )))]
        {
            use std::sync::atomic::{AtomicU32, Ordering};
            static CACHE: AtomicU32 = AtomicU32::new(2);
            return match CACHE.load(Ordering::Relaxed) {
                0 => false,
                1 => true,
                _ => {
                    let detected = std::is_x86_feature_detected!("avx2")
                        && std::is_x86_feature_detected!("bmi1")
                        && std::is_x86_feature_detected!("bmi2");
                    CACHE.store(u32::from(detected), Ordering::Relaxed);
                    detected
                }
            };
        }
    }
    false
}



macro_rules ! cfg_select { ({ $ ($ tt : tt) * }) => { { $ crate::cpu_features::cfg_select ! { $ ($ tt) * } } } ; (_ => { $ ($ output : tt) * }) => { $ ($ output) * } ; ($ cfg : meta => $ output : tt $ ($ ($ rest : tt) +) ?) => { # [cfg ($ cfg)] $ crate::cpu_features::cfg_select ! { _ => $ output } $ (# [cfg (not ($ cfg))] $ crate::cpu_features::cfg_select ! { $ ($ rest) + }) ? } }
pub(crate) use cfg_select;

#[inline(always)]
pub(crate) fn is_enabled_avx2() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}

#[inline(always)]
pub(crate) fn is_enabled_sse() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::is_x86_feature_detected!("sse")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}

#[inline(always)]
pub(crate) fn is_enabled_sse42() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::is_x86_feature_detected!("sse4.2")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}





#[inline(always)]
pub(crate) fn is_enabled_neon() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        std::arch::is_aarch64_feature_detected!("neon")
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        false
    }
}



#[inline(always)]
pub(crate) fn is_enabled_simd128() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        cfg!(target_feature = "simd128")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}
