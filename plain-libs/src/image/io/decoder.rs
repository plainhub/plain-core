use crate::image::animation::Frames;
use crate::image::color::{ColorType, ExtendedColorType};
use crate::image::error::ImageResult;
use crate::image::metadata::{LoopCount, Orientation};
pub trait ImageDecoder {
    fn dimensions(&self) -> (u32, u32);
    fn color_type(&self) -> ColorType;
    fn original_color_type(&self) -> ExtendedColorType {
        self.color_type().into()
    }
    fn icc_profile(&mut self) -> ImageResult<Option<Vec<u8>>> {
        Ok(None)
    }
    fn exif_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        Ok(None)
    }
    fn xmp_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        Ok(None)
    }
    fn iptc_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        Ok(None)
    }
    fn orientation(&mut self) -> ImageResult<Orientation> {
        Ok(self
            .exif_metadata()?
            .and_then(|chunk| Orientation::from_exif_chunk(&chunk))
            .unwrap_or(Orientation::NoTransforms))
    }
    fn total_bytes(&self) -> u64 {
        let dimensions = self.dimensions();
        let total_pixels = u64::from(dimensions.0) * u64::from(dimensions.1);
        let bytes_per_pixel = u64::from(self.color_type().bytes_per_pixel());
        total_pixels.saturating_mul(bytes_per_pixel)
    }
    fn read_image(self, buf: &mut [u8]) -> ImageResult<()>
    where
        Self: Sized;
    fn set_limits(&mut self, limits: crate::image::Limits) -> ImageResult<()> {
        limits.check_support(&crate::image::LimitSupport::default())?;
        let (width, height) = self.dimensions();
        limits.check_dimensions(width, height)?;
        Ok(())
    }
    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()>;
}
impl<T: ?Sized + ImageDecoder> ImageDecoder for Box<T> {
    fn dimensions(&self) -> (u32, u32) {
        (**self).dimensions()
    }
    fn color_type(&self) -> ColorType {
        (**self).color_type()
    }
    fn original_color_type(&self) -> ExtendedColorType {
        (**self).original_color_type()
    }
    fn icc_profile(&mut self) -> ImageResult<Option<Vec<u8>>> {
        (**self).icc_profile()
    }
    fn exif_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        (**self).exif_metadata()
    }
    fn xmp_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        (**self).xmp_metadata()
    }
    fn iptc_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        (**self).iptc_metadata()
    }
    fn orientation(&mut self) -> ImageResult<Orientation> {
        (**self).orientation()
    }
    fn total_bytes(&self) -> u64 {
        (**self).total_bytes()
    }
    fn read_image(self, buf: &mut [u8]) -> ImageResult<()>
    where
        Self: Sized,
    {
        T::read_image_boxed(self, buf)
    }
    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        T::read_image_boxed(*self, buf)
    }
    fn set_limits(&mut self, limits: crate::image::Limits) -> ImageResult<()> {
        (**self).set_limits(limits)
    }
}
pub trait ImageDecoderRect: ImageDecoder {
    fn read_rect(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        buf: &mut [u8],
        row_pitch: usize,
    ) -> ImageResult<()>;
}
pub trait AnimationDecoder<'a> {
    fn into_frames(self) -> Frames<'a>;
    fn loop_count(&self) -> LoopCount {
        LoopCount::Finite(core::num::NonZeroU32::new(1).unwrap())
    }
}
