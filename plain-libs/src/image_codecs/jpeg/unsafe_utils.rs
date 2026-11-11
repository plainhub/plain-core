#[cfg(all(all(), any(target_arch = "x86", target_arch = "x86_64")))]
pub use crate::image_codecs::jpeg::unsafe_utils_avx2::*;
#[cfg(all(all(), target_arch = "aarch64"))]
pub use crate::image_codecs::jpeg::unsafe_utils_neon::*;
