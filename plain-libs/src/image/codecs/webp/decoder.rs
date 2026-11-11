use crate::image::buffer::ConvertBuffer;
use crate::image::error::{DecodingError, ImageError, ImageResult};
use crate::image::metadata::Orientation;
use crate::image::{
    AnimationDecoder, ColorType, Delay, Frame, Frames, ImageDecoder, ImageFormat, RgbImage, Rgba,
    RgbaImage,
};
use crate::image_codecs::webp::LoopCount;
use std::io::{BufRead, Read, Seek};
pub struct WebPDecoder<R> {
    inner: crate::image_codecs::webp::WebPDecoder<R>,
    orientation: Option<Orientation>,
}
impl<R: BufRead + Seek> WebPDecoder<R> {
    pub fn new(r: R) -> ImageResult<Self> {
        Ok(Self {
            inner: crate::image_codecs::webp::WebPDecoder::new(r)
                .map_err(ImageError::from_webp_decode)?,
            orientation: None,
        })
    }

    pub fn set_background_color(&mut self, color: Rgba<u8>) -> ImageResult<()> {
        self.inner
            .set_background_color(color.0)
            .map_err(ImageError::from_webp_decode)
    }
}
impl<R: BufRead + Seek> ImageDecoder for WebPDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        self.inner.dimensions()
    }
    fn color_type(&self) -> ColorType {
        if self.inner.has_alpha() {
            ColorType::Rgba8
        } else {
            ColorType::Rgb8
        }
    }
    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));
        self.inner
            .read_image(buf)
            .map_err(ImageError::from_webp_decode)
    }
    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
    fn icc_profile(&mut self) -> ImageResult<Option<Vec<u8>>> {
        self.inner
            .icc_profile()
            .map_err(ImageError::from_webp_decode)
    }
    fn exif_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        let exif = self
            .inner
            .exif_metadata()
            .map_err(ImageError::from_webp_decode)?;
        self.orientation = Some(
            exif.as_ref()
                .and_then(|exif| Orientation::from_exif_chunk(exif))
                .unwrap_or(Orientation::NoTransforms),
        );
        Ok(exif)
    }
    fn xmp_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        self.inner
            .xmp_metadata()
            .map_err(ImageError::from_webp_decode)
    }
    fn orientation(&mut self) -> ImageResult<Orientation> {
        if self.orientation.is_none() {
            let _ = self.exif_metadata()?;
        }
        Ok(self.orientation.unwrap())
    }
}
impl<'a, R: 'a + BufRead + Seek> AnimationDecoder<'a> for WebPDecoder<R> {
    fn loop_count(&self) -> crate::image::metadata::LoopCount {
        match self.inner.loop_count() {
            LoopCount::Forever => crate::image::metadata::LoopCount::Infinite,
            LoopCount::Times(n) => crate::image::metadata::LoopCount::Finite(n.into()),
        }
    }
    fn into_frames(self) -> Frames<'a> {
        struct FramesInner<R: Read + Seek> {
            decoder: WebPDecoder<R>,
            current: u32,
        }
        impl<R: BufRead + Seek> Iterator for FramesInner<R> {
            type Item = ImageResult<Frame>;
            fn next(&mut self) -> Option<Self::Item> {
                if self.current == self.decoder.inner.num_frames() {
                    return None;
                }
                self.current += 1;
                let (width, height) = self.decoder.inner.dimensions();
                let (img, delay) = if self.decoder.inner.has_alpha() {
                    let mut img = RgbaImage::new(width, height);
                    match self.decoder.inner.read_frame(&mut img) {
                        Ok(delay) => (img, delay),
                        Err(crate::image_codecs::webp::DecodingError::NoMoreFrames) => return None,
                        Err(e) => return Some(Err(ImageError::from_webp_decode(e))),
                    }
                } else {
                    let mut img = RgbImage::new(width, height);
                    match self.decoder.inner.read_frame(&mut img) {
                        Ok(delay) => (img.convert(), delay),
                        Err(crate::image_codecs::webp::DecodingError::NoMoreFrames) => return None,
                        Err(e) => return Some(Err(ImageError::from_webp_decode(e))),
                    }
                };
                Some(Ok(Frame::from_parts(
                    img,
                    0,
                    0,
                    Delay::from_numer_denom_ms(delay, 1),
                )))
            }
        }
        Frames::new(Box::new(FramesInner {
            decoder: self,
            current: 0,
        }))
    }
}
impl ImageError {
    fn from_webp_decode(e: crate::image_codecs::webp::DecodingError) -> Self {
        match e {
            crate::image_codecs::webp::DecodingError::IoError(e) => ImageError::IoError(e),
            _ => ImageError::Decoding(DecodingError::new(ImageFormat::WebP.into(), e)),
        }
    }
}
