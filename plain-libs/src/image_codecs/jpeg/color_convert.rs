#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub use crate::image_codecs::jpeg::color_convert::avx::{ycbcr_to_rgb_avx2, ycbcr_to_rgba_avx2};
use crate::image_codecs::jpeg::decoder::ColorConvert16Ptr;
use crate::image_codecs::zune::colorspace::ColorSpace;
use crate::image_codecs::zune::options::DecoderOptions;
pub use scalar::ycbcr_to_grayscale;
mod avx;
mod neon64;
mod scalar;
pub fn choose_ycbcr_to_rgb_convert_func(
    type_need: ColorSpace,
    options: &DecoderOptions,
) -> Option<ColorConvert16Ptr> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        use crate::image_codecs::zune::log::debug;
        if options.use_avx2() {
            debug!("Using AVX optimised color conversion functions");
            match type_need {
                ColorSpace::RGB => return Some(ycbcr_to_rgb_avx2),
                ColorSpace::RGBA => return Some(ycbcr_to_rgba_avx2),
                _ => (),
            };
        }
    }
    #[cfg(all(all(), target_arch = "aarch64"))]
    {
        if options.use_neon() {
            use crate::image_codecs::jpeg::color_convert::neon64::{
                ycbcr_to_rgb_neon, ycbcr_to_rgba_neon,
            };
            match type_need {
                ColorSpace::RGB => return Some(ycbcr_to_rgb_neon),
                ColorSpace::RGBA => return Some(ycbcr_to_rgba_neon),
                _ => (),
            };
        }
    }
    return match type_need {
        ColorSpace::RGB => Some(scalar::ycbcr_to_rgb_inner_16_scalar::<false>),
        ColorSpace::RGBA => Some(scalar::ycbcr_to_rgba_inner_16_scalar::<false>),
        ColorSpace::BGRA => Some(scalar::ycbcr_to_rgba_inner_16_scalar::<true>),
        ColorSpace::BGR => Some(scalar::ycbcr_to_rgb_inner_16_scalar::<true>),
        _ => None,
    };
}
