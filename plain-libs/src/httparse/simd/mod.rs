mod swar;
#[cfg(not(all(
    all(),
    any(
        target_arch = "x86",
        target_arch = "x86_64",
        all(target_arch = "aarch64", all(),)
    ),
)))]
pub use self::swar::*;
#[cfg(all(
    all(),
    any(any(), not(any()),),
    any(target_arch = "x86", target_arch = "x86_64",),
))]
mod avx2;
#[cfg(all(
    all(),
    not(any(any(), any(),)),
    any(target_arch = "x86", target_arch = "x86_64",),
))]
mod runtime;
#[cfg(all(all(), not(any()), any(target_arch = "x86", target_arch = "x86_64",),))]
mod sse42;
#[cfg(all(
    all(),
    not(any(any(), any(),)),
    any(target_arch = "x86", target_arch = "x86_64",),
))]
pub use self::runtime::*;
#[cfg(all(all(), target_arch = "aarch64", all(),))]
mod neon;
#[cfg(all(all(), target_arch = "aarch64", all(),))]
pub use self::neon::*;
