pub use array_chunks::*;
pub use convolution::*;
pub use cpu_extensions::CpuExtensions;
pub use errors::*;
pub use image_view::*;
pub use pixels::PixelType;
pub use resizer::Resizer;

use crate::image_resize::alpha::AlphaMulDiv;

mod utils;

mod alpha;
mod array_chunks;
mod convolution;
mod cpu_extensions;
mod errors;
mod image_view;
pub mod images;
#[cfg(target_arch = "aarch64")]
mod neon_utils;
pub mod pixels;
mod resizer;
#[cfg(target_arch = "x86_64")]
mod simd_utils;
#[cfg(feature = "thumbnails")]
pub(crate) mod threading;
#[cfg(target_arch = "wasm32")]
mod wasm32_utils;

pub trait PixelTrait: Convolution + AlphaMulDiv {}

impl<P: Convolution + AlphaMulDiv> PixelTrait for P {}
