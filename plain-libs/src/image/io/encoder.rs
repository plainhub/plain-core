use crate::image::error::{ImageFormatHint, ImageResult, UnsupportedError, UnsupportedErrorKind};
use crate::image::{ColorType, DynamicImage, ExtendedColorType};
#[derive(Clone, Copy)]
pub struct MethodSealedToImage;
pub trait ImageEncoder {
    fn write_image(
        self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()>;
    fn set_icc_profile(&mut self, icc_profile: Vec<u8>) -> Result<(), UnsupportedError> {
        let _ = icc_profile;
        Err(UnsupportedError::from_format_and_kind(
            ImageFormatHint::Unknown,
            UnsupportedErrorKind::GenericFeature(
                "ICC profiles are not supported for this format".into(),
            ),
        ))
    }
    fn set_exif_metadata(&mut self, exif: Vec<u8>) -> Result<(), UnsupportedError> {
        let _ = exif;
        Err(UnsupportedError::from_format_and_kind(
            ImageFormatHint::Unknown,
            UnsupportedErrorKind::GenericFeature(
                "EXIF metadata is not supported for this format".into(),
            ),
        ))
    }
    fn make_compatible_img(
        &self,
        _: MethodSealedToImage,
        _input: &DynamicImage,
    ) -> Option<DynamicImage> {
        None
    }
}
pub(crate) trait ImageEncoderBoxed: ImageEncoder {
    fn write_image(
        self: Box<Self>,
        buf: &'_ [u8],
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<()>;
}
impl<T: ImageEncoder> ImageEncoderBoxed for T {
    fn write_image(
        self: Box<Self>,
        buf: &'_ [u8],
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<()> {
        (*self).write_image(buf, width, height, color)
    }
}
pub(crate) fn dynimage_conversion_8bit(img: &DynamicImage) -> Option<DynamicImage> {
    use ColorType::*;
    match img.color() {
        Rgb8 | Rgba8 | L8 | La8 => None,
        L16 => Some(img.to_luma8().into()),
        La16 => Some(img.to_luma_alpha8().into()),
        Rgb16 | Rgb32F => Some(img.to_rgb8().into()),
        Rgba16 | Rgba32F => Some(img.to_rgba8().into()),
    }
}
