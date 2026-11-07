use crate::image_resize::convolution::{self, FilterType};
use crate::image_resize::images::TypedImage;
use crate::image_resize::pixels::{self, InnerPixel};
use crate::image_resize::{
    CpuExtensions, ImageView, ImageViewMut, IntoImageView, IntoImageViewMut, PixelTrait, PixelType,
    ResizeError, image_view::try_pixel_type,
};

#[derive(Default, Debug, Clone)]
pub struct Resizer {
    cpu_extensions: CpuExtensions,
    alpha_buffer: Vec<u8>,
    convolution_buffer: Vec<u8>,
}
impl Resizer {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn resize(
        &mut self,
        src: &impl IntoImageView,
        dst: &mut impl IntoImageViewMut,
        filter: FilterType,
    ) -> Result<(), ResizeError> {
        let pixel_type = try_pixel_type(src)?;
        if pixel_type != try_pixel_type(dst)? {
            return Err(ResizeError::PixelTypesAreDifferent);
        }
        macro_rules! resize_as {
            ($pixel:ty) => {
                match (src.image_view::<$pixel>(), dst.image_view_mut::<$pixel>()) {
                    (Some(src), Some(mut dst)) => self.resize_typed(&src, &mut dst, filter),
                    _ => Err(ResizeError::PixelTypesAreDifferent),
                }
            };
        }
        match pixel_type {
            PixelType::U8x3 => resize_as!(pixels::U8x3),
            PixelType::U8x4 => resize_as!(pixels::U8x4),
        }
    }
    fn resize_typed<P: PixelTrait>(
        &mut self,
        src: &impl ImageView<Pixel = P>,
        dst: &mut impl ImageViewMut<Pixel = P>,
        filter: FilterType,
    ) -> Result<(), ResizeError> {
        if src.width() == 0 || src.height() == 0 || dst.width() == 0 || dst.height() == 0 {
            return Ok(());
        }
        if src.width() == dst.width() && src.height() == dst.height() {
            for (source, target) in src.iter_rows(0).zip(dst.iter_rows_mut(0)) {
                target.copy_from_slice(source);
            }
            return Ok(());
        }
        if P::pixel_type() == PixelType::U8x4 {
            let mut buffer = std::mem::take(&mut self.alpha_buffer);
            let mut premultiplied =
                get_temp_image_from_buffer(&mut buffer, src.width(), src.height());
            P::multiply_alpha(src, &mut premultiplied, self.cpu_extensions)?;
            self.do_convolution(&premultiplied, dst, filter);
            P::divide_alpha_inplace(dst, self.cpu_extensions)?;
            self.alpha_buffer = buffer;
        } else {
            self.do_convolution(src, dst, filter);
        }
        Ok(())
    }
    fn do_convolution<P: PixelTrait>(
        &mut self,
        src_view: &impl ImageView<Pixel = P>,
        dst_view: &mut impl ImageViewMut<Pixel = P>,
        filter_type: FilterType,
    ) {
        let (dst_width, dst_height) = (dst_view.width(), dst_view.height());
        if dst_width == 0 || dst_height == 0 {
            return;
        }

        let (filter_fn, filter_support) = convolution::get_filter_func(filter_type);

        let need_horizontal = dst_width as f64 != src_view.width() as f64;
        let horiz_coeffs = need_horizontal.then(|| {
            convolution::precompute_coefficients(
                src_view.width(),
                0.,
                src_view.width() as f64,
                dst_width,
                filter_fn,
                filter_support,
                true,
            )
        });

        let need_vertical = dst_height as f64 != src_view.height() as f64;
        let vert_coeffs = need_vertical.then(|| {
            convolution::precompute_coefficients(
                src_view.height(),
                0.,
                src_view.height() as f64,
                dst_height,
                filter_fn,
                filter_support,
                true,
            )
        });

        match (horiz_coeffs, vert_coeffs) {
            (Some(mut horiz_coeffs), Some(vert_coeffs)) => {
                // For u8-based images, it is faster to do the vertical pass first
                // instead of the horizontal.
                let x_first = horiz_coeffs.bounds[0].start;
                // Last used col in the source image
                let last_x_bound = horiz_coeffs.bounds.last().unwrap();
                let x_last = last_x_bound.start + last_x_bound.size;
                let temp_width = x_last - x_first;
                let mut temp_image = get_temp_image_from_buffer(
                    &mut self.convolution_buffer,
                    temp_width,
                    dst_height,
                );

                P::vert_convolution(
                    src_view,
                    &mut temp_image,
                    x_first,
                    vert_coeffs,
                    self.cpu_extensions,
                );

                // Shift bounds for the horizontal pass
                horiz_coeffs
                    .bounds
                    .iter_mut()
                    .for_each(|b| b.start -= x_first);

                P::horiz_convolution(&temp_image, dst_view, 0, horiz_coeffs, self.cpu_extensions);
            }
            (Some(horiz_coeffs), None) => {
                P::horiz_convolution(src_view, dst_view, 0, horiz_coeffs, self.cpu_extensions);
            }
            (None, Some(vert_coeffs)) => {
                P::vert_convolution(src_view, dst_view, 0, vert_coeffs, self.cpu_extensions);
            }
            _ => {}
        }
    }
}

fn get_temp_image_from_buffer<P: PixelTrait>(
    buffer: &mut Vec<u8>,
    width: u32,
    height: u32,
) -> TypedImage<'_, P> {
    let size = crate::image_resize::utils::image_size(width, height, P::size())
        .expect("Temporary image dimensions exceed address space");
    if buffer.len() < size {
        buffer.resize(size, 0);
    }
    TypedImage::from_buffer(width, height, &mut buffer[..size])
        .expect("Temporary image buffer has the required pixel layout")
}
