use crate::image::animation::{self, Ratio};
use crate::image::color::{ColorType, Rgba};
use crate::image::error::{
    DecodingError, EncodingError, ImageError, ImageResult, LimitError, LimitErrorKind,
    ParameterError, ParameterErrorKind, UnsupportedError, UnsupportedErrorKind,
};
use crate::image::metadata::LoopCount;
use crate::image::traits::Pixel;
use crate::image::{
    AnimationDecoder, ExtendedColorType, ImageBuffer, ImageDecoder, ImageEncoder, ImageFormat,
    Limits,
};
use crate::image_codecs::gif::ColorOutput;
use crate::image_codecs::gif::{DisposalMethod, Frame};
use std::io::{self, BufRead, Cursor, Read, Seek, Write};
use std::marker::PhantomData;
use std::mem;
use std::num::NonZeroU32;
pub struct GifDecoder<R: Read> {
    reader: crate::image_codecs::gif::Decoder<R>,
    limits: Limits,
}
impl<R: Read> GifDecoder<R> {
    pub fn new(r: R) -> ImageResult<GifDecoder<R>> {
        let mut decoder = crate::image_codecs::gif::DecodeOptions::new();
        decoder.set_color_output(ColorOutput::RGBA);
        Ok(GifDecoder {
            reader: decoder.read_info(r).map_err(ImageError::from_decoding)?,
            limits: Limits::no_limits(),
        })
    }
}

impl<R: BufRead + Seek> ImageDecoder for GifDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        (
            u32::from(self.reader.width()),
            u32::from(self.reader.height()),
        )
    }
    fn color_type(&self) -> ColorType {
        ColorType::Rgba8
    }
    fn set_limits(&mut self, limits: Limits) -> ImageResult<()> {
        limits.check_support(&crate::image::LimitSupport::default())?;
        let (width, height) = self.dimensions();
        limits.check_dimensions(width, height)?;
        self.limits = limits;
        Ok(())
    }
    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));
        let frame = match self
            .reader
            .next_frame_info()
            .map_err(ImageError::from_decoding)?
        {
            Some(frame) => FrameInfo::new_from_frame(frame),
            None => {
                return Err(ImageError::Parameter(ParameterError::from_kind(
                    ParameterErrorKind::NoMoreData,
                )));
            }
        };
        let (width, height) = self.dimensions();
        if frame.left == 0
            && frame.width == width
            && (u64::from(frame.top) + u64::from(frame.height) <= u64::from(height))
        {
            let line_length = usize::try_from(width)
                .unwrap()
                .checked_mul(self.color_type().bytes_per_pixel() as usize)
                .unwrap();
            let (blank_top, rest) =
                buf.split_at_mut(line_length.checked_mul(frame.top as usize).unwrap());
            let (buf, blank_bottom) =
                rest.split_at_mut(line_length.checked_mul(frame.height as usize).unwrap());
            debug_assert_eq!(buf.len(), self.reader.buffer_size());
            for b in blank_top {
                *b = 0;
            }
            self.reader
                .read_into_buffer(buf)
                .map_err(ImageError::from_decoding)?;
            for b in blank_bottom {
                *b = 0;
            }
        } else {
            let buffer_size = (frame.width as usize)
                .checked_mul(frame.height as usize)
                .and_then(|s| s.checked_mul(4))
                .ok_or(ImageError::Limits(LimitError::from_kind(
                    LimitErrorKind::InsufficientMemory,
                )))?;
            self.limits.reserve_usize(buffer_size)?;
            let mut frame_buffer = vec![0; buffer_size];
            self.limits.free_usize(buffer_size);
            self.reader
                .read_into_buffer(&mut frame_buffer[..])
                .map_err(ImageError::from_decoding)?;
            let frame_buffer = ImageBuffer::from_raw(frame.width, frame.height, frame_buffer);
            let image_buffer = ImageBuffer::from_raw(width, height, buf);
            if frame_buffer.is_none() || image_buffer.is_none() {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Gif.into(),
                        UnsupportedErrorKind::GenericFeature(format!(
                            "Image dimensions ({}, {}) are too large",
                            frame.width, frame.height
                        )),
                    ),
                ));
            }
            let frame_buffer = frame_buffer.unwrap();
            let mut image_buffer = image_buffer.unwrap();
            for (x, y, pixel) in image_buffer.enumerate_pixels_mut() {
                let frame_x = x.wrapping_sub(frame.left);
                let frame_y = y.wrapping_sub(frame.top);
                if frame_x < frame.width && frame_y < frame.height {
                    *pixel = *frame_buffer.get_pixel(frame_x, frame_y);
                } else {
                    *pixel = Rgba([0, 0, 0, 0]);
                }
            }
        }
        Ok(())
    }
    fn icc_profile(&mut self) -> ImageResult<Option<Vec<u8>>> {
        Ok(self.reader.icc_profile().map(Vec::from))
    }
    fn xmp_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        Ok(self.reader.xmp_metadata().map(Vec::from))
    }
    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}
struct GifFrameIterator<R: Read> {
    reader: crate::image_codecs::gif::Decoder<R>,
    width: u32,
    height: u32,
    non_disposed_frame: Option<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    limits: Limits,
    is_end: bool,
}
impl<R: BufRead + Seek> GifFrameIterator<R> {
    fn new(decoder: GifDecoder<R>) -> GifFrameIterator<R> {
        let (width, height) = decoder.dimensions();
        let limits = decoder.limits.clone();
        GifFrameIterator {
            reader: decoder.reader,
            width,
            height,
            non_disposed_frame: None,
            limits,
            is_end: false,
        }
    }
}
impl<R: Read> Iterator for GifFrameIterator<R> {
    type Item = ImageResult<animation::Frame>;
    fn next(&mut self) -> Option<ImageResult<animation::Frame>> {
        if self.is_end {
            return None;
        }
        const COLOR_TYPE: ColorType = ColorType::Rgba8;
        if self.non_disposed_frame.is_none() {
            if let Err(e) = self
                .limits
                .reserve_buffer(self.width, self.height, COLOR_TYPE)
            {
                return Some(Err(e));
            }
            self.non_disposed_frame = Some(ImageBuffer::from_pixel(
                self.width,
                self.height,
                Rgba([0, 0, 0, 0]),
            ));
        }
        let non_disposed_frame = self.non_disposed_frame.as_mut().unwrap();
        let frame = match self.reader.next_frame_info() {
            Ok(frame_info) => {
                if let Some(frame) = frame_info {
                    FrameInfo::new_from_frame(frame)
                } else {
                    return None;
                }
            }
            Err(err) => match err {
                crate::image_codecs::gif::DecodingError::Io(ref e) => {
                    if e.kind() == io::ErrorKind::UnexpectedEof {
                        self.is_end = true;
                    }
                    return Some(Err(ImageError::from_decoding(err)));
                }
                _ => {
                    return Some(Err(ImageError::from_decoding(err)));
                }
            },
        };
        let mut local_limits = self.limits.clone();
        if let Err(e) = local_limits.reserve_buffer(frame.width, frame.height, COLOR_TYPE) {
            return Some(Err(e));
        }
        let mut vec = vec![0; self.reader.buffer_size()];
        if let Err(err) = self.reader.read_into_buffer(&mut vec) {
            return Some(Err(ImageError::from_decoding(err)));
        }
        let Some(mut frame_buffer) = ImageBuffer::from_raw(frame.width, frame.height, vec) else {
            return Some(Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Gif.into(),
                    UnsupportedErrorKind::GenericFeature(format!(
                        "Image dimensions ({}, {}) are too large",
                        frame.width, frame.height
                    )),
                ),
            )));
        };
        fn blend_and_dispose_pixel(
            dispose: DisposalMethod,
            previous: &mut Rgba<u8>,
            current: &mut Rgba<u8>,
        ) {
            let pixel_alpha = current.channels()[3];
            if pixel_alpha == 0 {
                *current = *previous;
            }
            match dispose {
                DisposalMethod::Any | DisposalMethod::Keep => {
                    *previous = *current;
                }
                DisposalMethod::Background => {
                    *previous = Rgba([0, 0, 0, 0]);
                }
                DisposalMethod::Previous => {}
            }
        }
        let image_buffer = if (frame.left, frame.top) == (0, 0)
            && (self.width, self.height) == frame_buffer.dimensions()
        {
            for (x, y, pixel) in frame_buffer.enumerate_pixels_mut() {
                let previous_pixel = non_disposed_frame.get_pixel_mut(x, y);
                blend_and_dispose_pixel(frame.disposal_method, previous_pixel, pixel);
            }
            frame_buffer
        } else {
            if let Err(e) = local_limits.reserve_buffer(self.width, self.height, COLOR_TYPE) {
                return Some(Err(e));
            }
            ImageBuffer::from_fn(self.width, self.height, |x, y| {
                let frame_x = x.wrapping_sub(frame.left);
                let frame_y = y.wrapping_sub(frame.top);
                let previous_pixel = non_disposed_frame.get_pixel_mut(x, y);
                if frame_x < frame_buffer.width() && frame_y < frame_buffer.height() {
                    let mut pixel = *frame_buffer.get_pixel(frame_x, frame_y);
                    blend_and_dispose_pixel(frame.disposal_method, previous_pixel, &mut pixel);
                    pixel
                } else {
                    *previous_pixel
                }
            })
        };
        Some(Ok(animation::Frame::from_parts(
            image_buffer,
            0,
            0,
            frame.delay,
        )))
    }
}
impl<'a, R: BufRead + Seek + 'a> AnimationDecoder<'a> for GifDecoder<R> {
    fn loop_count(&self) -> LoopCount {
        match self.reader.repeat() {
            crate::image_codecs::gif::Repeat::Finite(n @ 1..) => {
                LoopCount::Finite(NonZeroU32::new(n.into()).expect("repeat is non-zero"))
            }
            crate::image_codecs::gif::Repeat::Finite(0)
            | crate::image_codecs::gif::Repeat::Infinite => LoopCount::Infinite,
        }
    }
    fn into_frames(self) -> animation::Frames<'a> {
        animation::Frames::new(Box::new(GifFrameIterator::new(self)))
    }
}
struct FrameInfo {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    disposal_method: DisposalMethod,
    delay: animation::Delay,
}
impl FrameInfo {
    fn new_from_frame(frame: &Frame) -> FrameInfo {
        FrameInfo {
            left: u32::from(frame.left),
            top: u32::from(frame.top),
            width: u32::from(frame.width),
            height: u32::from(frame.height),
            disposal_method: frame.dispose,
            delay: animation::Delay::from_ratio(Ratio::new(u32::from(frame.delay) * 10, 1)),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Repeat {
    Finite(u16),
    Infinite,
}
impl Repeat {
    pub(crate) fn to_gif_enum(self) -> crate::image_codecs::gif::Repeat {
        match self {
            Repeat::Finite(n) => crate::image_codecs::gif::Repeat::Finite(n),
            Repeat::Infinite => crate::image_codecs::gif::Repeat::Infinite,
        }
    }
}
pub struct GifEncoder<W: Write> {
    w: Option<W>,
    gif_encoder: Option<crate::image_codecs::gif::Encoder<W>>,
    speed: i32,
    repeat: Option<Repeat>,
}
impl<W: Write> GifEncoder<W> {
    pub fn new(w: W) -> GifEncoder<W> {
        Self::new_with_speed(w, 1)
    }
    pub fn new_with_speed(w: W, speed: i32) -> GifEncoder<W> {
        assert!(
            (1..=30).contains(&speed),
            "speed needs to be in the range [1, 30]"
        );
        GifEncoder {
            w: Some(w),
            gif_encoder: None,
            speed,
            repeat: None,
        }
    }
    pub fn set_repeat(&mut self, repeat: Repeat) -> ImageResult<()> {
        if let Some(ref mut encoder) = self.gif_encoder {
            encoder
                .set_repeat(repeat.to_gif_enum())
                .map_err(ImageError::from_encoding)?;
        }
        self.repeat = Some(repeat);
        Ok(())
    }
    pub fn encode(
        &mut self,
        data: &[u8],
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<()> {
        let (width, height) = self.gif_dimensions(width, height)?;
        match color {
            ExtendedColorType::Rgb8 => {
                self.encode_gif(Frame::from_rgb_speed(width, height, data, self.speed))
            }
            ExtendedColorType::Rgba8 => self.encode_gif(Frame::from_rgba_speed(
                width,
                height,
                &mut data.to_owned(),
                self.speed,
            )),
            _ => Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Gif.into(),
                    UnsupportedErrorKind::Color(color),
                ),
            )),
        }
    }
    pub fn encode_frame(&mut self, img_frame: animation::Frame) -> ImageResult<()> {
        let frame = self.convert_frame(img_frame)?;
        self.encode_gif(frame)
    }

    pub(crate) fn convert_frame(
        &mut self,
        img_frame: animation::Frame,
    ) -> ImageResult<Frame<'static>> {
        let frame_delay = img_frame.delay().into_ratio().to_integer();
        let mut rbga_frame = img_frame.into_buffer();
        let (width, height) = self.gif_dimensions(rbga_frame.width(), rbga_frame.height())?;
        let mut frame = Frame::from_rgba_speed(width, height, &mut rbga_frame, self.speed);
        frame.delay = (frame_delay / 10).try_into().unwrap_or(u16::MAX);
        Ok(frame)
    }
    fn gif_dimensions(&self, width: u32, height: u32) -> ImageResult<(u16, u16)> {
        fn inner_dimensions(width: u32, height: u32) -> Option<(u16, u16)> {
            let width = u16::try_from(width).ok()?;
            let height = u16::try_from(height).ok()?;
            Some((width, height))
        }
        inner_dimensions(width, height).ok_or_else(|| {
            ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::DimensionMismatch,
            ))
        })
    }
    pub(crate) fn encode_gif(&mut self, mut frame: Frame) -> ImageResult<()> {
        let gif_encoder;
        if let Some(ref mut encoder) = self.gif_encoder {
            gif_encoder = encoder;
        } else {
            let writer = self.w.take().unwrap();
            let mut encoder =
                crate::image_codecs::gif::Encoder::new(writer, frame.width, frame.height, &[])
                    .map_err(ImageError::from_encoding)?;
            if let Some(ref repeat) = self.repeat {
                encoder
                    .set_repeat(repeat.to_gif_enum())
                    .map_err(ImageError::from_encoding)?;
            }
            self.gif_encoder = Some(encoder);
            gif_encoder = self.gif_encoder.as_mut().unwrap();
        }
        frame.dispose = DisposalMethod::Background;
        gif_encoder
            .write_frame(&frame)
            .map_err(ImageError::from_encoding)
    }
}
impl<W: Write> ImageEncoder for GifEncoder<W> {
    fn write_image(
        mut self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        self.encode(buf, width, height, color_type)
    }
}
impl ImageError {
    fn from_decoding(err: crate::image_codecs::gif::DecodingError) -> ImageError {
        use crate::image_codecs::gif::DecodingError::*;
        match err {
            Io(io_err) => ImageError::IoError(io_err),
            other => ImageError::Decoding(DecodingError::new(ImageFormat::Gif.into(), other)),
        }
    }
    fn from_encoding(err: crate::image_codecs::gif::EncodingError) -> ImageError {
        use crate::image_codecs::gif::EncodingError::*;
        match err {
            Io(io_err) => ImageError::IoError(io_err),
            other => ImageError::Encoding(EncodingError::new(ImageFormat::Gif.into(), other)),
        }
    }
}
