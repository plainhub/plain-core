use crate::image_codecs::gif::common::{Block, Frame};
use crate::image_codecs::gif::{AnyExtension, Extension, Repeat};
use core::convert::{TryFrom, TryInto};
use core::iter::FusedIterator;
use core::mem;
use core::num::NonZeroU64;
use std::borrow::Cow;
use std::io;
use std::io::prelude::*;
use std::vec::Vec;
mod converter;
mod decoder;
pub use self::converter::ColorOutput;
use self::converter::PixelConverter;
pub use self::decoder::{
    Decoded, DecodingError, DecodingFormatError, FrameDataType, FrameDecoder, OutputBuffer,
    PLTE_CHANNELS, StreamingDecoder, Version,
};
#[derive(Clone, Debug)]
pub enum MemoryLimit {
    Unlimited,
    Bytes(NonZeroU64),
}
impl MemoryLimit {
    fn check_size(&self, size: usize) -> Result<(), DecodingError> {
        match self {
            Self::Unlimited => Ok(()),
            Self::Bytes(limit) => {
                if size as u64 <= limit.get() {
                    Ok(())
                } else {
                    Err(DecodingError::MemoryLimit)
                }
            }
        }
    }
    fn buffer_size(&self, color: ColorOutput, width: u16, height: u16) -> Option<usize> {
        let pixels = u64::from(width) * u64::from(height);
        let bytes_per_pixel = match color {
            ColorOutput::Indexed => 1,
            ColorOutput::RGBA => 4,
        };
        let total_bytes = pixels * bytes_per_pixel;
        let usize_bytes = usize::try_from(total_bytes).ok()?;
        match self {
            Self::Unlimited => Some(usize_bytes),
            Self::Bytes(limit) => {
                if total_bytes > limit.get() {
                    None
                } else {
                    Some(usize_bytes)
                }
            }
        }
    }
    #[inline]
    fn try_reserve(&self, vec: &mut Vec<u8>, additional: usize) -> Result<(), DecodingError> {
        let len = vec
            .len()
            .checked_add(additional)
            .ok_or(DecodingError::MemoryLimit)?;
        self.check_size(len)?;
        vec.try_reserve(additional)
            .map_err(|_| DecodingError::OutOfMemory)?;
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct DecodeOptions {
    memory_limit: MemoryLimit,
    color_output: ColorOutput,
    check_frame_consistency: bool,
    skip_frame_decoding: bool,
    check_for_end_code: bool,
    allow_unknown_blocks: bool,
}
impl Default for DecodeOptions {
    fn default() -> Self {
        Self::new()
    }
}
impl DecodeOptions {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            memory_limit: MemoryLimit::Bytes(50_000_000.try_into().unwrap()),
            color_output: ColorOutput::Indexed,
            check_frame_consistency: false,
            skip_frame_decoding: false,
            check_for_end_code: false,
            allow_unknown_blocks: false,
        }
    }
    #[inline]
    pub fn set_color_output(&mut self, color: ColorOutput) {
        self.color_output = color;
    }
    pub fn set_memory_limit(&mut self, limit: MemoryLimit) {
        self.memory_limit = limit;
    }
    pub fn check_frame_consistency(&mut self, check: bool) {
        self.check_frame_consistency = check;
    }
    pub fn skip_frame_decoding(&mut self, skip: bool) {
        self.skip_frame_decoding = skip;
    }

    pub fn allow_unknown_blocks(&mut self, check: bool) {
        self.allow_unknown_blocks = check;
    }
    pub fn read_info<R: Read>(self, r: R) -> Result<Decoder<R>, DecodingError> {
        Decoder::with_no_init(r, StreamingDecoder::with_options(&self), self).init()
    }
}
struct ReadDecoder<R: Read> {
    reader: io::BufReader<R>,
    decoder: StreamingDecoder,
    at_eof: bool,
}
impl<R: Read> ReadDecoder<R> {
    #[inline(never)]
    fn decode_next(
        &mut self,
        write_into: &mut OutputBuffer<'_>,
    ) -> Result<Option<Decoded>, DecodingError> {
        while !self.at_eof {
            let (consumed, result) = {
                let buf = self.reader.fill_buf()?;
                if buf.is_empty() {
                    return Err(DecodingError::UnexpectedEof);
                }
                self.decoder.update(buf, write_into)?
            };
            self.reader.consume(consumed);
            match result {
                Decoded::Nothing => (),
                Decoded::BlockStart(Block::Trailer) => {
                    self.at_eof = true;
                }
                result => return Ok(Some(result)),
            }
        }
        Ok(None)
    }
    fn into_inner(self) -> io::BufReader<R> {
        self.reader
    }
    fn decode_next_bytes(&mut self, out: &mut OutputBuffer<'_>) -> Result<usize, DecodingError> {
        match self.decode_next(out)? {
            Some(Decoded::BytesDecoded(len)) => Ok(len.get()),
            Some(Decoded::DataEnd) => Ok(0),
            _ => Err(DecodingError::format("unexpected data")),
        }
    }
}
const EXT_NAME_NETSCAPE: &[u8] = b"NETSCAPE2.0";
const EXT_NAME_XMP: &[u8] = b"XMP DataXMP";
const EXT_NAME_ICC: &[u8] = b"ICCRGBG1012";
enum AppExtensionState {
    None,
    Netscape,
    Xmp,
    Icc,
    Skip,
}
pub struct Decoder<R: Read> {
    decoder: ReadDecoder<R>,
    pixel_converter: PixelConverter,
    memory_limit: MemoryLimit,
    bg_color: Option<u8>,
    repeat: Repeat,
    current_frame: Frame<'static>,
    current_frame_data_type: FrameDataType,
    app_extension_state: AppExtensionState,
    xmp_metadata: Option<Vec<u8>>,
    icc_profile: Option<Vec<u8>>,
}
impl<R> Decoder<R>
where
    R: Read,
{
    #[inline]
    pub fn new(reader: R) -> Result<Self, DecodingError> {
        DecodeOptions::new().read_info(reader)
    }
    #[must_use]
    #[inline]
    pub fn build() -> DecodeOptions {
        DecodeOptions::new()
    }
    fn with_no_init(reader: R, decoder: StreamingDecoder, options: DecodeOptions) -> Self {
        Self {
            decoder: ReadDecoder {
                reader: io::BufReader::new(reader),
                decoder,
                at_eof: false,
            },
            bg_color: None,
            pixel_converter: PixelConverter::new(options.color_output),
            memory_limit: options.memory_limit.clone(),
            repeat: Repeat::default(),
            current_frame: Frame::default(),
            current_frame_data_type: FrameDataType::Pixels,
            app_extension_state: AppExtensionState::None,
            xmp_metadata: None,
            icc_profile: None,
        }
    }
    fn init(mut self) -> Result<Self, DecodingError> {
        const APP_EXTENSION: AnyExtension = AnyExtension(Extension::Application as u8);
        loop {
            match self.decoder.decode_next(&mut OutputBuffer::None)? {
                Some(Decoded::BackgroundColor(bg_color)) => {
                    self.bg_color = Some(bg_color);
                }
                Some(Decoded::GlobalPalette(palette)) => {
                    self.pixel_converter.set_global_palette(palette.into());
                }
                Some(Decoded::SubBlock {
                    ext: APP_EXTENSION,
                    is_last,
                }) => {
                    self.read_application_extension(is_last)?;
                }
                Some(Decoded::HeaderEnd) => break,
                Some(_) => {
                    continue;
                }
                None => {
                    return Err(DecodingError::format(
                        "file does not contain any image data",
                    ));
                }
            }
        }
        if let Some(palette) = self.pixel_converter.global_palette() {
            if self.bg_color.unwrap_or(0) as usize >= (palette.len() / PLTE_CHANNELS) {
                self.bg_color = None;
            }
        }
        Ok(self)
    }
    fn read_application_extension(&mut self, is_last: bool) -> Result<(), DecodingError> {
        let data = self.decoder.decoder.last_ext_sub_block();
        match self.app_extension_state {
            AppExtensionState::None => {
                self.app_extension_state = match data {
                    EXT_NAME_NETSCAPE => AppExtensionState::Netscape,
                    EXT_NAME_XMP => {
                        self.xmp_metadata = Some(Vec::new());
                        AppExtensionState::Xmp
                    }
                    EXT_NAME_ICC => {
                        self.icc_profile = Some(Vec::new());
                        AppExtensionState::Icc
                    }
                    _ => AppExtensionState::Skip,
                }
            }
            AppExtensionState::Netscape => {
                if let [1, rest @ ..] = data {
                    if let Ok(repeat) = rest.try_into().map(u16::from_le_bytes) {
                        self.repeat = if repeat == 0 {
                            Repeat::Infinite
                        } else {
                            Repeat::Finite(repeat)
                        };
                    }
                }
                self.app_extension_state = AppExtensionState::Skip;
            }
            AppExtensionState::Xmp => {
                if let Some(xmp_metadata) = &mut self.xmp_metadata {
                    self.memory_limit
                        .try_reserve(xmp_metadata, 1 + data.len())?;
                    xmp_metadata.push(data.len() as u8);
                    xmp_metadata.extend_from_slice(data);
                    if is_last {
                        const RAMP_SIZE: usize = 257;
                        if xmp_metadata.len() >= RAMP_SIZE
                            && xmp_metadata.ends_with(&[0x03, 0x02, 0x01, 0x00])
                            && xmp_metadata[xmp_metadata.len() - RAMP_SIZE..]
                                .starts_with(&[0x01, 0x0ff])
                        {
                            xmp_metadata.truncate(xmp_metadata.len() - RAMP_SIZE);
                        }
                    }
                }
            }
            AppExtensionState::Icc => {
                if let Some(icc) = &mut self.icc_profile {
                    self.memory_limit.try_reserve(icc, data.len())?;
                    icc.extend_from_slice(data);
                }
            }
            AppExtensionState::Skip => {}
        };
        if is_last {
            self.app_extension_state = AppExtensionState::None;
        }
        Ok(())
    }
    pub fn next_frame_info(&mut self) -> Result<Option<&Frame<'static>>, DecodingError> {
        loop {
            match self.decoder.decode_next(&mut OutputBuffer::None)? {
                Some(Decoded::FrameMetadata(frame_data_type)) => {
                    self.current_frame = self.decoder.decoder.current_frame_mut().take();
                    self.current_frame_data_type = frame_data_type;
                    if self.current_frame.palette.is_none() && self.global_palette().is_none() {
                        return Err(DecodingError::format(
                            "no color table available for current frame",
                        ));
                    }
                    break;
                }
                Some(_) => (),
                None => return Ok(None),
            }
        }
        Ok(Some(&self.current_frame))
    }

    pub fn read_next_frame(&mut self) -> Result<Option<&Frame<'static>>, DecodingError> {
        if self.next_frame_info()?.is_some() {
            match self.current_frame_data_type {
                FrameDataType::Pixels => {
                    self.pixel_converter.read_frame(
                        &mut self.current_frame,
                        &mut |out| self.decoder.decode_next_bytes(out),
                        &self.memory_limit,
                    )?;
                }
                FrameDataType::Lzw { min_code_size } => {
                    let mut vec = if matches!(self.current_frame.buffer, Cow::Owned(_)) {
                        let mut vec =
                            mem::replace(&mut self.current_frame.buffer, Cow::Borrowed(&[]))
                                .into_owned();
                        vec.clear();
                        vec
                    } else {
                        Vec::new()
                    };
                    vec.try_reserve(
                        usize::from(self.current_frame.width)
                            * usize::from(self.current_frame.height)
                            / 4,
                    )
                    .map_err(|_| DecodingError::OutOfMemory)?;
                    self.copy_lzw_into_buffer(min_code_size, &mut vec)?;
                    self.current_frame.buffer = Cow::Owned(vec);
                }
            }
            Ok(Some(&self.current_frame))
        } else {
            Ok(None)
        }
    }
    fn take_current_frame(&mut self) -> Option<Frame<'static>> {
        if self.current_frame.buffer.is_empty() {
            return None;
        }
        Some(self.current_frame.take())
    }
    pub fn read_into_buffer(&mut self, buf: &mut [u8]) -> Result<(), DecodingError> {
        self.pixel_converter
            .read_into_buffer(&self.current_frame, buf, &mut |out| {
                self.decoder.decode_next_bytes(out)
            })
    }
    fn copy_lzw_into_buffer(
        &mut self,
        min_code_size: u8,
        buf: &mut Vec<u8>,
    ) -> Result<(), DecodingError> {
        buf.push(min_code_size);
        loop {
            match self.decoder.decode_next(&mut OutputBuffer::Vec(buf))? {
                Some(Decoded::LzwDataCopied(_len)) => {}
                Some(Decoded::DataEnd) => return Ok(()),
                _ => return Err(DecodingError::format("unexpected data")),
            }
        }
    }
    pub fn fill_buffer(&mut self, buf: &mut [u8]) -> Result<bool, DecodingError> {
        self.pixel_converter
            .fill_buffer(&self.current_frame, buf, &mut |out| {
                self.decoder.decode_next_bytes(out)
            })
    }
    pub fn buffer_size(&self) -> usize {
        self.pixel_converter
            .buffer_size(&self.current_frame)
            .unwrap()
    }
    pub fn line_length(&self) -> usize {
        self.pixel_converter.line_length(&self.current_frame)
    }
    #[inline]
    pub fn palette(&self) -> Result<&[u8], DecodingError> {
        Ok(match self.current_frame.palette {
            Some(ref table) => table,
            None => self.global_palette().ok_or_else(|| {
                DecodingError::format("no color table available for current frame")
            })?,
        })
    }
    pub fn global_palette(&self) -> Option<&[u8]> {
        self.pixel_converter.global_palette()
    }
    #[inline]
    pub fn width(&self) -> u16 {
        self.decoder.decoder.width()
    }
    #[inline]
    pub fn height(&self) -> u16 {
        self.decoder.decoder.height()
    }
    #[inline]
    #[must_use]
    pub fn xmp_metadata(&self) -> Option<&[u8]> {
        self.xmp_metadata.as_deref()
    }
    #[inline]
    #[must_use]
    pub fn icc_profile(&self) -> Option<&[u8]> {
        self.icc_profile.as_deref()
    }
    pub fn into_inner(self) -> io::BufReader<R> {
        self.decoder.into_inner()
    }
    pub fn bg_color(&self) -> Option<usize> {
        self.bg_color.map(|v| v as usize)
    }
    #[inline]
    pub fn repeat(&self) -> Repeat {
        self.repeat
    }
}
impl<R: Read> IntoIterator for Decoder<R> {
    type Item = Result<Frame<'static>, DecodingError>;
    type IntoIter = DecoderIter<R>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        DecoderIter {
            inner: self,
            ended: false,
        }
    }
}
pub struct DecoderIter<R: Read> {
    inner: Decoder<R>,
    ended: bool,
}
impl<R: Read> DecoderIter<R> {
    pub fn into_inner(self) -> io::BufReader<R> {
        self.inner.into_inner()
    }
}
impl<R: Read> FusedIterator for DecoderIter<R> {}
impl<R: Read> Iterator for DecoderIter<R> {
    type Item = Result<Frame<'static>, DecodingError>;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.ended {
            match self.inner.read_next_frame() {
                Ok(Some(_)) => self.inner.take_current_frame().map(Ok),
                Ok(None) => {
                    self.ended = true;
                    None
                }
                Err(err) => {
                    self.ended = true;
                    Some(Err(err))
                }
            }
        } else {
            None
        }
    }
}
