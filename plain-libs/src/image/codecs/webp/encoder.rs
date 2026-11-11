use crate::image::error::{EncodingError, UnsupportedError, UnsupportedErrorKind};
use crate::image::{
    DynamicImage, ExtendedColorType, ImageEncoder, ImageError, ImageFormat, ImageResult,
};
use std::io::Write;
pub struct WebPEncoder<W> {
    inner: crate::image_codecs::webp::WebPEncoder<W>,
}
impl<W: Write> WebPEncoder<W> {
    pub fn new_lossless(w: W) -> Self {
        Self {
            inner: crate::image_codecs::webp::WebPEncoder::new(w),
        }
    }
    #[track_caller]
    pub fn encode(
        self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        let expected_buffer_len = color_type.buffer_size(width, height);
        assert_eq!(
            expected_buffer_len,
            buf.len() as u64,
            "Invalid buffer length: expected {expected_buffer_len} got {} for {width}x{height} image",
            buf.len(),
        );
        let color_type = match color_type {
            ExtendedColorType::L8 => crate::image_codecs::webp::ColorType::L8,
            ExtendedColorType::La8 => crate::image_codecs::webp::ColorType::La8,
            ExtendedColorType::Rgb8 => crate::image_codecs::webp::ColorType::Rgb8,
            ExtendedColorType::Rgba8 => crate::image_codecs::webp::ColorType::Rgba8,
            _ => {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::WebP.into(),
                        UnsupportedErrorKind::Color(color_type),
                    ),
                ));
            }
        };
        self.inner
            .encode(buf, width, height, color_type)
            .map_err(ImageError::from_webp_encode)
    }
}
impl<W: Write> ImageEncoder for WebPEncoder<W> {
    #[track_caller]
    fn write_image(
        self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        self.encode(buf, width, height, color_type)
    }
    fn set_icc_profile(&mut self, icc_profile: Vec<u8>) -> Result<(), UnsupportedError> {
        self.inner.set_icc_profile(icc_profile);
        Ok(())
    }
    fn set_exif_metadata(&mut self, exif: Vec<u8>) -> Result<(), UnsupportedError> {
        self.inner.set_exif_metadata(exif);
        Ok(())
    }
    fn make_compatible_img(
        &self,
        _: crate::image::io::encoder::MethodSealedToImage,
        img: &DynamicImage,
    ) -> Option<DynamicImage> {
        crate::image::io::encoder::dynimage_conversion_8bit(img)
    }
}
impl ImageError {
    fn from_webp_encode(e: crate::image_codecs::webp::EncodingError) -> Self {
        match e {
            crate::image_codecs::webp::EncodingError::IoError(e) => ImageError::IoError(e),
            _ => ImageError::Encoding(EncodingError::new(ImageFormat::WebP.into(), e)),
        }
    }
}
