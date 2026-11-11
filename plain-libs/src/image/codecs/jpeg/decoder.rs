use crate::image::color::ColorType;
use crate::image::error::{
    DecodingError, ImageError, ImageResult, LimitError, UnsupportedError, UnsupportedErrorKind,
};
use crate::image::metadata::Orientation;
use crate::image::{ImageDecoder, ImageFormat, Limits};
use crate::image_codecs::zune::bytestream::ZCursor;
use std::io::{BufRead, Seek};
use std::marker::PhantomData;
type ZuneColorSpace = crate::image_codecs::zune::colorspace::ColorSpace;
pub struct JpegDecoder<R> {
    input: Vec<u8>,
    orig_color_space: ZuneColorSpace,
    width: u16,
    height: u16,
    limits: Limits,
    orientation: Option<Orientation>,
    phantom: PhantomData<R>,
}
impl<R: BufRead + Seek> JpegDecoder<R> {
    pub fn new(r: R) -> ImageResult<JpegDecoder<R>> {
        let mut input = Vec::new();
        let mut r = r;
        r.read_to_end(&mut input)?;
        let options = crate::image_codecs::zune::options::DecoderOptions::default()
            .set_strict_mode(false)
            .set_max_width(usize::MAX)
            .set_max_height(usize::MAX);
        let mut decoder = crate::image_codecs::jpeg::JpegDecoder::new_with_options(
            ZCursor::new(input.as_slice()),
            options,
        );
        decoder.decode_headers().map_err(ImageError::from_jpeg)?;
        let (width, height) = decoder.dimensions().unwrap();
        let width: u16 = width.try_into().unwrap();
        let height: u16 = height.try_into().unwrap();
        let orig_color_space = decoder.input_colorspace().expect("headers were decoded");
        decoder.set_options({
            let requested_color = match orig_color_space {
                ZuneColorSpace::RGB
                | ZuneColorSpace::RGBA
                | ZuneColorSpace::Luma
                | ZuneColorSpace::LumaA => orig_color_space,
                _ => ZuneColorSpace::RGB,
            };
            decoder.options().jpeg_set_out_colorspace(requested_color)
        });
        let limits = Limits::no_limits();
        Ok(JpegDecoder {
            input,
            orig_color_space,
            width,
            height,
            limits,
            orientation: None,
            phantom: PhantomData,
        })
    }
}
impl<R: BufRead + Seek> ImageDecoder for JpegDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        (u32::from(self.width), u32::from(self.height))
    }
    fn color_type(&self) -> ColorType {
        ColorType::from_jpeg(self.orig_color_space)
    }
    fn icc_profile(&mut self) -> ImageResult<Option<Vec<u8>>> {
        let options = crate::image_codecs::zune::options::DecoderOptions::default()
            .set_strict_mode(false)
            .set_max_width(usize::MAX)
            .set_max_height(usize::MAX);
        let mut decoder = crate::image_codecs::jpeg::JpegDecoder::new_with_options(
            ZCursor::new(&self.input),
            options,
        );
        decoder.decode_headers().map_err(ImageError::from_jpeg)?;
        Ok(decoder.icc_profile())
    }
    fn exif_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        let options = crate::image_codecs::zune::options::DecoderOptions::default()
            .set_strict_mode(false)
            .set_max_width(usize::MAX)
            .set_max_height(usize::MAX);
        let mut decoder = crate::image_codecs::jpeg::JpegDecoder::new_with_options(
            ZCursor::new(&self.input),
            options,
        );
        decoder.decode_headers().map_err(ImageError::from_jpeg)?;
        let exif = decoder.exif().cloned();
        self.orientation = Some(
            exif.as_ref()
                .and_then(|exif| Orientation::from_exif_chunk(exif))
                .unwrap_or(Orientation::NoTransforms),
        );
        Ok(exif)
    }
    fn xmp_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        let options = crate::image_codecs::zune::options::DecoderOptions::default()
            .set_strict_mode(false)
            .set_max_width(usize::MAX)
            .set_max_height(usize::MAX);
        let mut decoder = crate::image_codecs::jpeg::JpegDecoder::new_with_options(
            ZCursor::new(&self.input),
            options,
        );
        decoder.decode_headers().map_err(ImageError::from_jpeg)?;
        Ok(decoder.xmp().cloned())
    }
    fn iptc_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        let options = crate::image_codecs::zune::options::DecoderOptions::default()
            .set_strict_mode(false)
            .set_max_width(usize::MAX)
            .set_max_height(usize::MAX);
        let mut decoder = crate::image_codecs::jpeg::JpegDecoder::new_with_options(
            ZCursor::new(&self.input),
            options,
        );
        decoder.decode_headers().map_err(ImageError::from_jpeg)?;
        Ok(decoder.iptc().cloned())
    }
    fn orientation(&mut self) -> ImageResult<Orientation> {
        if self.orientation.is_none() {
            let _ = self.exif_metadata()?;
        }
        Ok(self.orientation.unwrap())
    }
    fn read_image(self, buf: &mut [u8]) -> ImageResult<()> {
        let advertised_len = self.total_bytes();
        let actual_len = buf.len() as u64;
        if actual_len != advertised_len {
            return Err(ImageError::Decoding(DecodingError::new(
                ImageFormat::Jpeg.into(),
                format!(
                    "Length of the decoded data {actual_len} \
                    doesn't match the advertised dimensions of the image \
                    that imply length {advertised_len}"
                ),
            )));
        }
        let mut decoder = new_zune_decoder(&self.input, self.orig_color_space, self.limits);
        decoder.decode_into(buf).map_err(ImageError::from_jpeg)?;
        Ok(())
    }
    fn set_limits(&mut self, limits: Limits) -> ImageResult<()> {
        limits.check_support(&crate::image::LimitSupport::default())?;
        let (width, height) = self.dimensions();
        limits.check_dimensions(width, height)?;
        self.limits = limits;
        Ok(())
    }
    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}
impl ColorType {
    fn from_jpeg(colorspace: ZuneColorSpace) -> ColorType {
        let colorspace = to_supported_color_space(colorspace);
        use crate::image_codecs::zune::colorspace::ColorSpace::*;
        match colorspace {
            RGB => ColorType::Rgb8,
            RGBA => ColorType::Rgba8,
            Luma => ColorType::L8,
            LumaA => ColorType::La8,
            _ => unreachable!(),
        }
    }
}
fn to_supported_color_space(orig: ZuneColorSpace) -> ZuneColorSpace {
    use crate::image_codecs::zune::colorspace::ColorSpace::*;
    match orig {
        RGB | RGBA | Luma | LumaA => orig,
        _ => RGB,
    }
}
fn new_zune_decoder(
    input: &[u8],
    orig_color_space: ZuneColorSpace,
    limits: Limits,
) -> crate::image_codecs::jpeg::JpegDecoder<ZCursor<&[u8]>> {
    let target_color_space = to_supported_color_space(orig_color_space);
    let mut options = crate::image_codecs::zune::options::DecoderOptions::default()
        .jpeg_set_out_colorspace(target_color_space)
        .set_strict_mode(false);
    options = options.set_max_width(match limits.max_image_width {
        Some(max_width) => max_width as usize,
        None => usize::MAX,
    });
    options = options.set_max_height(match limits.max_image_height {
        Some(max_height) => max_height as usize,
        None => usize::MAX,
    });
    crate::image_codecs::jpeg::JpegDecoder::new_with_options(ZCursor::new(input), options)
}
impl ImageError {
    fn from_jpeg(err: crate::image_codecs::jpeg::errors::DecodeErrors) -> ImageError {
        use crate::image_codecs::jpeg::errors::DecodeErrors::*;
        match err {
            Unsupported(desc) => ImageError::Unsupported(UnsupportedError::from_format_and_kind(
                ImageFormat::Jpeg.into(),
                UnsupportedErrorKind::GenericFeature(format!("{desc:?}")),
            )),
            LargeDimensions(_) => ImageError::Limits(LimitError::from_kind(
                crate::image::error::LimitErrorKind::DimensionError,
            )),
            err => ImageError::Decoding(DecodingError::new(ImageFormat::Jpeg.into(), err)),
        }
    }
}
