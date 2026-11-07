use crate::image_resize::{CpuExtensions, ImageError, ImageView, ImageViewMut, pixels};

#[macro_use]
mod common;

mod u8x4;

pub(crate) trait AlphaMulDiv: pixels::InnerPixel {
    fn multiply_alpha(
        src_view: &impl ImageView<Pixel = Self>,
        dst_view: &mut impl ImageViewMut<Pixel = Self>,
        cpu_extensions: CpuExtensions,
    ) -> Result<(), ImageError> {
        Err(ImageError::UnsupportedPixelType)
    }

    fn divide_alpha_inplace(
        image_view: &mut impl ImageViewMut<Pixel = Self>,
        cpu_extensions: CpuExtensions,
    ) -> Result<(), ImageError> {
        Err(ImageError::UnsupportedPixelType)
    }
}

impl AlphaMulDiv for pixels::U8x3 {}
