#[cfg(target_arch = "aarch64")]
pub mod aarch64;
pub mod all;
pub(crate) mod generic;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub mod wasm32;
#[cfg(target_arch = "x86_64")]
pub mod x86_64;
