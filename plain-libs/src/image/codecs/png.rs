use crate::image::animation::{Delay, Frame, Frames, Ratio};
use crate::image::color::{Blend, ColorType, ExtendedColorType};
use crate::image::error::{
    DecodingError, ImageError, ImageResult, LimitError, LimitErrorKind, ParameterError,
    ParameterErrorKind, UnsupportedError, UnsupportedErrorKind,
};
use crate::image::metadata::LoopCount;
use crate::image::utils::vec_try_with_capacity;
use crate::image::{
    AnimationDecoder, DynamicImage, GenericImage, GenericImageView, ImageBuffer, ImageDecoder,
    ImageEncoder, ImageFormat, Limits, Luma, LumaA, Rgb, Rgba, RgbaImage,
};
use crate::image_codecs::png::{BlendOp, DeflateCompression, DisposeOp};
use std::borrow::Cow;
use std::io::{BufRead, Seek, Write};
use std::num::NonZeroU32;
pub(crate) const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const XMP_KEY: &str = "XML:com.adobe.xmp";
const IPTC_KEYS: &[&str] = &["Raw profile type iptc", "Raw profile type 8bim"];
pub struct PngDecoder<R: BufRead + Seek> {
    color_type: ColorType,
    reader: crate::image_codecs::png::Reader<R>,
    limits: Limits,
}
impl<R: BufRead + Seek> PngDecoder<R> {
    pub fn new(r: R) -> ImageResult<PngDecoder<R>> {
        Self::with_limits(r, Limits::no_limits())
    }
    pub fn with_limits(r: R, limits: Limits) -> ImageResult<PngDecoder<R>> {
        limits.check_support(&crate::image::LimitSupport::default())?;
        let max_bytes = usize::try_from(limits.max_alloc.unwrap_or(u64::MAX)).unwrap_or(usize::MAX);
        let mut decoder = crate::image_codecs::png::Decoder::new_with_limits(
            r,
            crate::image_codecs::png::Limits { bytes: max_bytes },
        );
        decoder.set_ignore_text_chunk(false);
        let info = decoder.read_header_info().map_err(ImageError::from_png)?;
        limits.check_dimensions(info.width, info.height)?;
        decoder.set_transformations(crate::image_codecs::png::Transformations::EXPAND);
        let reader = decoder.read_info().map_err(ImageError::from_png)?;
        let (color_type, bits) = reader.output_color_type();
        let color_type = match (color_type, bits) {
            (
                crate::image_codecs::png::ColorType::Grayscale,
                crate::image_codecs::png::BitDepth::Eight,
            ) => ColorType::L8,
            (
                crate::image_codecs::png::ColorType::Grayscale,
                crate::image_codecs::png::BitDepth::Sixteen,
            ) => ColorType::L16,
            (
                crate::image_codecs::png::ColorType::GrayscaleAlpha,
                crate::image_codecs::png::BitDepth::Eight,
            ) => ColorType::La8,
            (
                crate::image_codecs::png::ColorType::GrayscaleAlpha,
                crate::image_codecs::png::BitDepth::Sixteen,
            ) => ColorType::La16,
            (
                crate::image_codecs::png::ColorType::Rgb,
                crate::image_codecs::png::BitDepth::Eight,
            ) => ColorType::Rgb8,
            (
                crate::image_codecs::png::ColorType::Rgb,
                crate::image_codecs::png::BitDepth::Sixteen,
            ) => ColorType::Rgb16,
            (
                crate::image_codecs::png::ColorType::Rgba,
                crate::image_codecs::png::BitDepth::Eight,
            ) => ColorType::Rgba8,
            (
                crate::image_codecs::png::ColorType::Rgba,
                crate::image_codecs::png::BitDepth::Sixteen,
            ) => ColorType::Rgba16,
            (
                crate::image_codecs::png::ColorType::Grayscale,
                crate::image_codecs::png::BitDepth::One,
            ) => {
                return Err(unsupported_color(ExtendedColorType::L1));
            }
            (
                crate::image_codecs::png::ColorType::GrayscaleAlpha,
                crate::image_codecs::png::BitDepth::One,
            ) => {
                return Err(unsupported_color(ExtendedColorType::La1));
            }
            (crate::image_codecs::png::ColorType::Rgb, crate::image_codecs::png::BitDepth::One) => {
                return Err(unsupported_color(ExtendedColorType::Rgb1));
            }
            (
                crate::image_codecs::png::ColorType::Rgba,
                crate::image_codecs::png::BitDepth::One,
            ) => {
                return Err(unsupported_color(ExtendedColorType::Rgba1));
            }
            (
                crate::image_codecs::png::ColorType::Grayscale,
                crate::image_codecs::png::BitDepth::Two,
            ) => {
                return Err(unsupported_color(ExtendedColorType::L2));
            }
            (
                crate::image_codecs::png::ColorType::GrayscaleAlpha,
                crate::image_codecs::png::BitDepth::Two,
            ) => {
                return Err(unsupported_color(ExtendedColorType::La2));
            }
            (crate::image_codecs::png::ColorType::Rgb, crate::image_codecs::png::BitDepth::Two) => {
                return Err(unsupported_color(ExtendedColorType::Rgb2));
            }
            (
                crate::image_codecs::png::ColorType::Rgba,
                crate::image_codecs::png::BitDepth::Two,
            ) => {
                return Err(unsupported_color(ExtendedColorType::Rgba2));
            }
            (
                crate::image_codecs::png::ColorType::Grayscale,
                crate::image_codecs::png::BitDepth::Four,
            ) => {
                return Err(unsupported_color(ExtendedColorType::L4));
            }
            (
                crate::image_codecs::png::ColorType::GrayscaleAlpha,
                crate::image_codecs::png::BitDepth::Four,
            ) => {
                return Err(unsupported_color(ExtendedColorType::La4));
            }
            (
                crate::image_codecs::png::ColorType::Rgb,
                crate::image_codecs::png::BitDepth::Four,
            ) => {
                return Err(unsupported_color(ExtendedColorType::Rgb4));
            }
            (
                crate::image_codecs::png::ColorType::Rgba,
                crate::image_codecs::png::BitDepth::Four,
            ) => {
                return Err(unsupported_color(ExtendedColorType::Rgba4));
            }
            (crate::image_codecs::png::ColorType::Indexed, bits) => {
                return Err(unsupported_color(ExtendedColorType::Unknown(bits as u8)));
            }
        };
        Ok(PngDecoder {
            color_type,
            reader,
            limits,
        })
    }

    pub fn apng(self) -> ImageResult<ApngDecoder<R>> {
        Ok(ApngDecoder::new(self))
    }
}
fn unsupported_color(ect: ExtendedColorType) -> ImageError {
    ImageError::Unsupported(UnsupportedError::from_format_and_kind(
        ImageFormat::Png.into(),
        UnsupportedErrorKind::Color(ect),
    ))
}
impl<R: BufRead + Seek> ImageDecoder for PngDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        self.reader.info().size()
    }
    fn color_type(&self) -> ColorType {
        self.color_type
    }
    fn icc_profile(&mut self) -> ImageResult<Option<Vec<u8>>> {
        Ok(self.reader.info().icc_profile.as_ref().map(|x| x.to_vec()))
    }
    fn exif_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        Ok(self
            .reader
            .info()
            .exif_metadata
            .as_ref()
            .map(|x| x.to_vec()))
    }
    fn xmp_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        if let Some(mut itx_chunk) = self
            .reader
            .info()
            .utf8_text
            .iter()
            .find(|chunk| chunk.keyword.contains(XMP_KEY))
            .cloned()
        {
            itx_chunk.decompress_text().map_err(ImageError::from_png)?;
            return itx_chunk
                .get_text()
                .map(|text| Some(text.as_bytes().to_vec()))
                .map_err(ImageError::from_png);
        }
        Ok(None)
    }
    fn iptc_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        if let Some(mut text_chunk) = self
            .reader
            .info()
            .compressed_latin1_text
            .iter()
            .find(|chunk| IPTC_KEYS.iter().any(|key| chunk.keyword.contains(key)))
            .cloned()
        {
            text_chunk.decompress_text().map_err(ImageError::from_png)?;
            return text_chunk
                .get_text()
                .map(|text| Some(text.as_bytes().to_vec()))
                .map_err(ImageError::from_png);
        }
        if let Some(text_chunk) = self
            .reader
            .info()
            .uncompressed_latin1_text
            .iter()
            .find(|chunk| IPTC_KEYS.iter().any(|key| chunk.keyword.contains(key)))
            .cloned()
        {
            return Ok(Some(text_chunk.text.into_bytes()));
        }
        Ok(None)
    }
    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));
        self.reader.next_frame(buf).map_err(ImageError::from_png)?;
        let bpc = self.color_type().bytes_per_pixel() / self.color_type().channel_count();
        match bpc {
            1 => (),
            2 => buf.as_chunks_mut::<2>().0.iter_mut().for_each(|c| {
                *c = u16::from_be_bytes(*c).to_ne_bytes();
            }),
            _ => unreachable!(),
        }
        Ok(())
    }
    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
    fn set_limits(&mut self, limits: Limits) -> ImageResult<()> {
        limits.check_support(&crate::image::LimitSupport::default())?;
        let info = self.reader.info();
        limits.check_dimensions(info.width, info.height)?;
        self.limits = limits;
        Ok(())
    }
}
pub struct ApngDecoder<R: BufRead + Seek> {
    inner: PngDecoder<R>,
    current: Option<RgbaImage>,
    previous: Option<RgbaImage>,
    dispose: DisposeOp,
    dispose_region: Option<(u32, u32, u32, u32)>,
    remaining: u32,
    has_thumbnail: bool,
}
impl<R: BufRead + Seek> ApngDecoder<R> {
    fn new(inner: PngDecoder<R>) -> Self {
        let info = inner.reader.info();
        let remaining = match info.animation_control() {
            Some(actl) => actl.num_frames,
            None => 0,
        };
        let has_thumbnail = info.frame_control.is_none();
        ApngDecoder {
            inner,
            current: None,
            previous: None,
            dispose: DisposeOp::Background,
            dispose_region: None,
            remaining,
            has_thumbnail,
        }
    }
    fn mix_next_frame(&mut self) -> Result<Option<&RgbaImage>, ImageError> {
        const COLOR_TYPE: ColorType = ColorType::Rgba8;
        let (width, height) = self.inner.dimensions();
        {
            let limits = &mut self.inner.limits;
            if self.previous.is_none() {
                limits.reserve_buffer(width, height, COLOR_TYPE)?;
                self.previous = Some(RgbaImage::new(width, height));
            }
            if self.current.is_none() {
                limits.reserve_buffer(width, height, COLOR_TYPE)?;
                self.current = Some(RgbaImage::new(width, height));
            }
        }
        self.remaining = match self.remaining.checked_sub(1) {
            None => return Ok(None),
            Some(next) => next,
        };
        let remaining = self.remaining;
        self.remaining = 0;
        if self.has_thumbnail {
            let mut limits = self.inner.limits.clone();
            let buffer_size = self.inner.reader.output_buffer_size().ok_or_else(|| {
                ImageError::Limits(LimitError::from_kind(LimitErrorKind::InsufficientMemory))
            })?;
            limits.reserve_usize(buffer_size)?;
            let mut buffer = vec![0; buffer_size];
            self.inner
                .reader
                .next_frame(&mut buffer)
                .map_err(ImageError::from_png)?;
            self.has_thumbnail = false;
        }
        self.animatable_color_type()?;
        let previous = self.previous.as_mut().unwrap();
        let current = self.current.as_mut().unwrap();
        match self.dispose {
            DisposeOp::None => {
                previous.clone_from(current);
            }
            DisposeOp::Background => {
                previous.clone_from(current);
                if let Some((px, py, width, height)) = self.dispose_region {
                    let mut region_current = current.sub_image(px, py, width, height);
                    let pixels: Vec<_> = region_current.pixels().collect();
                    for (x, y, _) in &pixels {
                        region_current.put_pixel(*x, *y, Rgba::from([0, 0, 0, 0]));
                    }
                } else {
                    current.pixels_mut().for_each(|pixel| {
                        *pixel = Rgba::from([0, 0, 0, 0]);
                    });
                }
            }
            DisposeOp::Previous => {
                let (px, py, width, height) = self
                    .dispose_region
                    .expect("The first frame must not set dispose=Previous");
                let region_previous = previous.sub_image(px, py, width, height);
                current
                    .copy_from(&region_previous.to_image(), px, py)
                    .unwrap();
            }
        }
        let mut limits = self.inner.limits.clone();
        let raw_frame_size = self.inner.reader.output_buffer_size().ok_or_else(|| {
            ImageError::Limits(LimitError::from_kind(LimitErrorKind::InsufficientMemory))
        })?;
        limits.reserve_usize(raw_frame_size)?;
        let mut buffer = vec![0; raw_frame_size];
        self.inner
            .reader
            .next_frame(&mut buffer)
            .map_err(ImageError::from_png)?;
        let info = self.inner.reader.info();
        let (width, height, px, py, blend);
        match info.frame_control() {
            None => {
                width = info.width;
                height = info.height;
                px = 0;
                py = 0;
                blend = BlendOp::Source;
            }
            Some(fc) => {
                width = fc.width;
                height = fc.height;
                px = fc.x_offset;
                py = fc.y_offset;
                blend = fc.blend_op;
                self.dispose = fc.dispose_op;
            }
        }
        self.dispose_region = Some((px, py, width, height));
        limits.reserve_buffer(width, height, COLOR_TYPE)?;
        let source = match self.inner.color_type {
            ColorType::L8 => {
                let image = ImageBuffer::<Luma<_>, _>::from_raw(width, height, buffer).unwrap();
                DynamicImage::ImageLuma8(image).into_rgba8()
            }
            ColorType::La8 => {
                let image = ImageBuffer::<LumaA<_>, _>::from_raw(width, height, buffer).unwrap();
                DynamicImage::ImageLumaA8(image).into_rgba8()
            }
            ColorType::Rgb8 => {
                let image = ImageBuffer::<Rgb<_>, _>::from_raw(width, height, buffer).unwrap();
                DynamicImage::ImageRgb8(image).into_rgba8()
            }
            ColorType::Rgba8 => ImageBuffer::<Rgba<_>, _>::from_raw(width, height, buffer).unwrap(),
            ColorType::L16 | ColorType::Rgb16 | ColorType::La16 | ColorType::Rgba16 => {
                unreachable!("16-bit apng not yet support")
            }
            _ => unreachable!("Invalid png color"),
        };
        limits.free_usize(raw_frame_size);
        match blend {
            BlendOp::Source => {
                current
                    .copy_from(&source, px, py)
                    .expect("Invalid png image not detected in png");
            }
            BlendOp::Over => {
                for (x, y, p) in source.enumerate_pixels() {
                    current.get_pixel_mut(x + px, y + py).blend(p);
                }
            }
        }
        self.remaining = remaining;
        Ok(Some(self.current.as_ref().unwrap()))
    }
    fn animatable_color_type(&self) -> Result<(), ImageError> {
        match self.inner.color_type {
            ColorType::L8 | ColorType::Rgb8 | ColorType::La8 | ColorType::Rgba8 => Ok(()),
            ColorType::L16 | ColorType::Rgb16 | ColorType::La16 | ColorType::Rgba16 => {
                Err(unsupported_color(self.inner.color_type.into()))
            }
            _ => unreachable!("{:?} not a valid png color", self.inner.color_type),
        }
    }
}
impl<'a, R: BufRead + Seek + 'a> AnimationDecoder<'a> for ApngDecoder<R> {
    fn loop_count(&self) -> LoopCount {
        match self.inner.reader.info().animation_control() {
            None => LoopCount::Infinite,
            Some(actl) => match NonZeroU32::new(actl.num_plays) {
                None => LoopCount::Infinite,
                Some(n) => LoopCount::Finite(n),
            },
        }
    }
    fn into_frames(self) -> Frames<'a> {
        struct FrameIterator<R: BufRead + Seek>(ApngDecoder<R>);
        impl<R: BufRead + Seek> Iterator for FrameIterator<R> {
            type Item = ImageResult<Frame>;
            fn next(&mut self) -> Option<Self::Item> {
                let image = match self.0.mix_next_frame() {
                    Ok(Some(image)) => image.clone(),
                    Ok(None) => return None,
                    Err(err) => return Some(Err(err)),
                };
                let info = self.0.inner.reader.info();
                let fc = info.frame_control().unwrap();
                let num = u32::from(fc.delay_num) * 1_000u32;
                let denom = match fc.delay_den {
                    0 => 100,
                    d => u32::from(d),
                };
                let delay = Delay::from_ratio(Ratio::new(num, denom));
                Some(Ok(Frame::from_parts(image, 0, 0, delay)))
            }
        }
        Frames::new(Box::new(FrameIterator(self)))
    }
}
pub struct PngEncoder<W: Write> {
    w: W,
    compression: CompressionType,
    filter: FilterType,
    icc_profile: Vec<u8>,
    exif_metadata: Vec<u8>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
#[derive(Default)]
pub enum CompressionType {
    Default,
    #[default]
    Fast,
    Best,
    Uncompressed,
    Level(u8),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
#[derive(Default)]
pub enum FilterType {
    NoFilter,
    Sub,
    Up,
    Avg,
    Paeth,
    #[default]
    Adaptive,
}
impl<W: Write> PngEncoder<W> {
    pub fn new(w: W) -> PngEncoder<W> {
        PngEncoder {
            w,
            compression: CompressionType::default(),
            filter: FilterType::default(),
            icc_profile: Vec::new(),
            exif_metadata: Vec::new(),
        }
    }
    pub fn new_with_quality(
        w: W,
        compression: CompressionType,
        filter: FilterType,
    ) -> PngEncoder<W> {
        PngEncoder {
            w,
            compression,
            filter,
            icc_profile: Vec::new(),
            exif_metadata: Vec::new(),
        }
    }
    fn encode_inner(
        self,
        data: &[u8],
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<()> {
        let (ct, bits) = match color {
            ExtendedColorType::L8 => (
                crate::image_codecs::png::ColorType::Grayscale,
                crate::image_codecs::png::BitDepth::Eight,
            ),
            ExtendedColorType::L16 => (
                crate::image_codecs::png::ColorType::Grayscale,
                crate::image_codecs::png::BitDepth::Sixteen,
            ),
            ExtendedColorType::La8 => (
                crate::image_codecs::png::ColorType::GrayscaleAlpha,
                crate::image_codecs::png::BitDepth::Eight,
            ),
            ExtendedColorType::La16 => (
                crate::image_codecs::png::ColorType::GrayscaleAlpha,
                crate::image_codecs::png::BitDepth::Sixteen,
            ),
            ExtendedColorType::Rgb8 => (
                crate::image_codecs::png::ColorType::Rgb,
                crate::image_codecs::png::BitDepth::Eight,
            ),
            ExtendedColorType::Rgb16 => (
                crate::image_codecs::png::ColorType::Rgb,
                crate::image_codecs::png::BitDepth::Sixteen,
            ),
            ExtendedColorType::Rgba8 => (
                crate::image_codecs::png::ColorType::Rgba,
                crate::image_codecs::png::BitDepth::Eight,
            ),
            ExtendedColorType::Rgba16 => (
                crate::image_codecs::png::ColorType::Rgba,
                crate::image_codecs::png::BitDepth::Sixteen,
            ),
            _ => {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Png.into(),
                        UnsupportedErrorKind::Color(color),
                    ),
                ));
            }
        };
        let comp = match self.compression {
            CompressionType::Default => crate::image_codecs::png::Compression::Balanced,
            CompressionType::Best => crate::image_codecs::png::Compression::High,
            CompressionType::Fast => crate::image_codecs::png::Compression::Fast,
            CompressionType::Uncompressed => crate::image_codecs::png::Compression::NoCompression,
            CompressionType::Level(0) => crate::image_codecs::png::Compression::NoCompression,
            CompressionType::Level(_) => crate::image_codecs::png::Compression::Fast,
        };
        let advanced_comp = match self.compression {
            CompressionType::Level(n @ 1..) => Some(DeflateCompression::Level(n)),
            _ => None,
        };
        let filter = match self.filter {
            FilterType::NoFilter => crate::image_codecs::png::Filter::NoFilter,
            FilterType::Sub => crate::image_codecs::png::Filter::Sub,
            FilterType::Up => crate::image_codecs::png::Filter::Up,
            FilterType::Avg => crate::image_codecs::png::Filter::Avg,
            FilterType::Paeth => crate::image_codecs::png::Filter::Paeth,
            FilterType::Adaptive => crate::image_codecs::png::Filter::Adaptive,
        };
        let mut info = crate::image_codecs::png::Info::with_size(width, height);
        if !self.icc_profile.is_empty() {
            info.icc_profile = Some(Cow::Borrowed(&self.icc_profile));
        }
        if !self.exif_metadata.is_empty() {
            info.exif_metadata = Some(Cow::Borrowed(&self.exif_metadata));
        }
        let mut encoder = crate::image_codecs::png::Encoder::with_info(self.w, info)
            .map_err(|e| ImageError::IoError(e.into()))?;
        encoder.set_color(ct);
        encoder.set_depth(bits);
        encoder.set_compression(comp);
        if let Some(compression) = advanced_comp {
            encoder.set_deflate_compression(compression);
        }
        encoder.set_filter(filter);
        let mut writer = encoder
            .write_header()
            .map_err(|e| ImageError::IoError(e.into()))?;
        writer
            .write_image_data(data)
            .map_err(|e| ImageError::IoError(e.into()))
    }
}
impl<W: Write> ImageEncoder for PngEncoder<W> {
    #[track_caller]
    fn write_image(
        self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        use ExtendedColorType::*;
        let expected_buffer_len = color_type.buffer_size(width, height);
        assert_eq!(
            expected_buffer_len,
            buf.len() as u64,
            "Invalid buffer length: expected {expected_buffer_len} got {} for {width}x{height} image",
            buf.len(),
        );
        match color_type {
            L8 | La8 | Rgb8 | Rgba8 => self.encode_inner(buf, width, height, color_type),
            L16 | La16 | Rgb16 | Rgba16 => {
                let mut reordered;
                let buf = if cfg!(target_endian = "little") {
                    reordered = vec_try_with_capacity(buf.len())?;
                    reordered.extend(buf.as_chunks::<2>().0.iter().flat_map(|le| [le[1], le[0]]));
                    &reordered
                } else {
                    buf
                };
                self.encode_inner(buf, width, height, color_type)
            }
            _ => Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Png.into(),
                    UnsupportedErrorKind::Color(color_type),
                ),
            )),
        }
    }
    fn set_icc_profile(&mut self, icc_profile: Vec<u8>) -> Result<(), UnsupportedError> {
        self.icc_profile = icc_profile;
        Ok(())
    }
    fn set_exif_metadata(&mut self, exif: Vec<u8>) -> Result<(), UnsupportedError> {
        self.exif_metadata = exif;
        Ok(())
    }
}
impl ImageError {
    fn from_png(err: crate::image_codecs::png::DecodingError) -> ImageError {
        use crate::image_codecs::png::DecodingError::*;
        match err {
            IoError(err) => ImageError::IoError(err),
            err @ Format(_) => {
                ImageError::Decoding(DecodingError::new(ImageFormat::Png.into(), err))
            }
            err @ Parameter(_) => ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::Generic(err.to_string()),
            )),
            LimitsExceeded => {
                ImageError::Limits(LimitError::from_kind(LimitErrorKind::InsufficientMemory))
            }
        }
    }
}
