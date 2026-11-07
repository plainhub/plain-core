use super::{Coefficients, Convolution};
use crate::image_resize::convolution::optimisations::Normalizer16;
use crate::image_resize::convolution::vertical_u8::vert_convolution_u8;
use crate::image_resize::pixels::U8x4;
use crate::image_resize::{CpuExtensions, ImageView, ImageViewMut};

#[cfg(target_arch = "x86_64")]
mod avx2;
#[cfg(target_arch = "aarch64")]
mod neon;
#[cfg(target_arch = "x86_64")]
mod sse4;
#[cfg(target_arch = "wasm32")]
mod wasm32;

impl_u8_convolution!(U8x4, 4);
