#[cfg(feature = "Win32_Media_MediaFoundation")]
pub mod MediaFoundation;
#[cfg(any())]
core::include!("impl.rs");
