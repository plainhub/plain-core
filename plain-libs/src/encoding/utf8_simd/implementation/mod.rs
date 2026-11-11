#[macro_use]
mod algorithm;
#[cfg(target_arch = "aarch64")]
pub(crate) mod aarch64;
pub(crate) mod helpers;
#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm32;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod x86;
