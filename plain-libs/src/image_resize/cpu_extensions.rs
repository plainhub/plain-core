#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuExtensions {
    None,
    #[cfg(target_arch = "x86_64")]
    Sse4_1,
    #[cfg(target_arch = "x86_64")]
    Avx2,
    #[cfg(target_arch = "aarch64")]
    Neon,
    #[cfg(target_arch = "wasm32")]
    Simd128,
}

impl CpuExtensions {
    pub fn is_supported(&self) -> bool {
        match self {
            #[cfg(target_arch = "x86_64")]
            Self::Avx2 => is_x86_feature_detected!("avx2"),
            #[cfg(target_arch = "x86_64")]
            Self::Sse4_1 => is_x86_feature_detected!("sse4.1"),
            #[cfg(target_arch = "aarch64")]
            Self::Neon => std::arch::is_aarch64_feature_detected!("neon"),
            #[cfg(target_arch = "wasm32")]
            Self::Simd128 => true,
            Self::None => true,
        }
    }
}

impl Default for CpuExtensions {
    #[cfg(target_arch = "x86_64")]
    fn default() -> Self {
        if is_x86_feature_detected!("avx2") {
            Self::Avx2
        } else if is_x86_feature_detected!("sse4.1") {
            Self::Sse4_1
        } else {
            Self::None
        }
    }

    #[cfg(target_arch = "aarch64")]
    fn default() -> Self {
        if std::arch::is_aarch64_feature_detected!("neon") {
            Self::Neon
        } else {
            Self::None
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn default() -> Self {
        Self::Simd128
    }

    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "wasm32"
    )))]
    fn default() -> Self {
        Self::None
    }
}
