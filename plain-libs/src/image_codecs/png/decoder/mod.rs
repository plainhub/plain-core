mod interlace_info;
mod read_decoder;
pub(crate) mod stream;
pub(crate) mod transform;
mod unfiltering_buffer;
mod zlib;
use self::read_decoder::{ImageDataCompletionStatus, ReadDecoder};
use self::stream::{DecodeOptions, DecodingError, FormatErrorInner};
use self::transform::{TransformFn, create_transform_fn};
use self::unfiltering_buffer::UnfilteringBuffer;
use crate::image_codecs::png::FrameControl;
use crate::image_codecs::png::adam7::Adam7Info;
use crate::image_codecs::png::common::{
    BitDepth, BytesPerPixel, ColorType, Info, ParameterErrorKind, Transformations,
};
pub use interlace_info::InterlaceInfo;
use interlace_info::InterlaceInfoIter;
use std::io::{BufRead, Seek};
use std::mem;
pub use zlib::{UnfilterBuf, UnfilterRegion};
#[derive(Debug, PartialEq, Eq)]
pub struct OutputInfo {
    pub width: u32,
    pub height: u32,
    pub color_type: ColorType,
    pub bit_depth: BitDepth,
    pub line_size: usize,
}
impl OutputInfo {
    pub fn buffer_size(&self) -> usize {
        self.line_size * self.height as usize
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
}
impl Limits {
    pub(crate) fn reserve_bytes(&mut self, bytes: usize) -> Result<(), DecodingError> {
        if self.bytes >= bytes {
            self.bytes -= bytes;
            Ok(())
        } else {
            Err(DecodingError::LimitsExceeded)
        }
    }
}
impl Default for Limits {
    fn default() -> Limits {
        Limits {
            bytes: 1024 * 1024 * 64,
        }
    }
}
pub struct Decoder<R: BufRead + Seek> {
    read_decoder: ReadDecoder<R>,
    transform: Transformations,
}
#[derive(Clone, Copy, Debug)]
pub struct InterlacedRow<'data> {
    data: &'data [u8],
    interlace: InterlaceInfo,
}
impl<'data> InterlacedRow<'data> {
    pub fn data(&self) -> &'data [u8] {
        self.data
    }
    pub fn interlace(&self) -> &InterlaceInfo {
        &self.interlace
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Row<'data> {
    data: &'data [u8],
}
impl<'data> Row<'data> {
    pub fn data(&self) -> &'data [u8] {
        self.data
    }
}
impl<R: BufRead + Seek> Decoder<R> {
    pub fn new(r: R) -> Decoder<R> {
        Decoder::new_with_limits(r, Limits::default())
    }
    pub fn new_with_limits(r: R, limits: Limits) -> Decoder<R> {
        let mut read_decoder = ReadDecoder::new(r);
        read_decoder.set_limits(limits);
        Decoder {
            read_decoder,
            transform: Transformations::IDENTITY,
        }
    }
    pub fn new_with_options(r: R, decode_options: DecodeOptions) -> Decoder<R> {
        let mut read_decoder = ReadDecoder::with_options(r, decode_options);
        read_decoder.set_limits(Limits::default());
        Decoder {
            read_decoder,
            transform: Transformations::IDENTITY,
        }
    }
    pub fn set_limits(&mut self, limits: Limits) {
        self.read_decoder.set_limits(limits);
    }
    pub fn read_header_info(&mut self) -> Result<&Info<'static>, DecodingError> {
        self.read_decoder.read_header_info()
    }
    pub fn read_info(mut self) -> Result<Reader<R>, DecodingError> {
        let info = self.read_header_info()?;
        let unfiltering_buffer = UnfilteringBuffer::new(info);
        let mut reader = Reader {
            decoder: self.read_decoder,
            bpp: BytesPerPixel::One,
            subframe: SubframeInfo::not_yet_init(),
            remaining_frames: 0,
            unfiltering_buffer,
            transform: self.transform,
            transform_fn: None,
            scratch_buffer: Vec::new(),
            finished: false,
        };
        if reader.info().checked_raw_row_length().is_none() {
            return Err(DecodingError::LimitsExceeded);
        }
        if reader.output_buffer_size().is_none() {
            return Err(DecodingError::LimitsExceeded);
        }
        reader.read_until_image_data()?;
        reader.remaining_frames = match reader.info().animation_control.as_ref() {
            None => 1,
            Some(animation) => {
                let mut num_frames = animation.num_frames;
                if reader.info().frame_control.is_none() {
                    num_frames += 1;
                }
                num_frames
            }
        };
        Ok(reader)
    }
    pub fn set_transformations(&mut self, transform: Transformations) {
        self.transform = transform;
    }
    pub fn set_ignore_text_chunk(&mut self, ignore_text_chunk: bool) {
        self.read_decoder.set_ignore_text_chunk(ignore_text_chunk);
    }
    pub fn set_ignore_iccp_chunk(&mut self, ignore_iccp_chunk: bool) {
        self.read_decoder.set_ignore_iccp_chunk(ignore_iccp_chunk);
    }
    pub fn ignore_checksums(&mut self, ignore_checksums: bool) {
        self.read_decoder.ignore_checksums(ignore_checksums);
    }
}
pub struct Reader<R: BufRead + Seek> {
    decoder: ReadDecoder<R>,
    bpp: BytesPerPixel,
    subframe: SubframeInfo,
    remaining_frames: u32,
    unfiltering_buffer: UnfilteringBuffer,
    transform: Transformations,
    transform_fn: Option<TransformFn>,
    scratch_buffer: Vec<u8>,
    finished: bool,
}
struct SubframeInfo {
    width: u32,
    height: u32,
    rowlen: usize,
    current_interlace_info: Option<InterlaceInfo>,
    interlace_info_iter: InterlaceInfoIter,
    consumed_and_flushed: bool,
}
impl<R: BufRead + Seek> Reader<R> {
    pub fn next_frame_info(&mut self) -> Result<&FrameControl, DecodingError> {
        let remaining_frames = if self.subframe.consumed_and_flushed {
            self.remaining_frames
        } else {
            self.remaining_frames - 1
        };
        if remaining_frames == 0 {
            return Err(DecodingError::Parameter(
                ParameterErrorKind::PolledAfterEndOfImage.into(),
            ));
        }
        if !self.subframe.consumed_and_flushed {
            self.subframe.current_interlace_info = None;
            self.finish_decoding()?;
        }
        self.read_until_image_data()?;
        Ok(self.info().frame_control.as_ref().unwrap())
    }
    fn read_until_image_data(&mut self) -> Result<(), DecodingError> {
        self.decoder.read_until_image_data()?;
        self.subframe = SubframeInfo::new(self.info());
        self.bpp = self.info().bpp_in_prediction();
        let frame_bytes = if self.info().interlaced {
            let mut bytes = 0u64;
            for pass in crate::image_codecs::png::adam7::PassConstants::PASSES {
                bytes += self
                    .info()
                    .raw_row_length_from_width(pass.count_samples(self.subframe.width))
                    as u64
                    * pass.count_lines(self.subframe.height) as u64;
            }
            bytes
        } else {
            (self.subframe.rowlen as u64) * self.subframe.height as u64
        };
        self.unfiltering_buffer.start_frame(frame_bytes);
        let buflen = self.unguarded_output_line_size(self.subframe.width);
        self.decoder.reserve_bytes(buflen)?;
        Ok(())
    }
    pub fn info(&self) -> &Info<'static> {
        self.decoder.info().unwrap()
    }
    pub fn next_frame(&mut self, buf: &mut [u8]) -> Result<OutputInfo, DecodingError> {
        if self.remaining_frames == 0 {
            return Err(DecodingError::Parameter(
                ParameterErrorKind::PolledAfterEndOfImage.into(),
            ));
        } else if self.subframe.consumed_and_flushed {
            self.read_until_image_data()?;
        }
        let required_len = self
            .output_buffer_size()
            .ok_or(DecodingError::LimitsExceeded)?;
        if buf.len() < required_len {
            return Err(DecodingError::Parameter(
                ParameterErrorKind::ImageBufferSize {
                    expected: required_len,
                    actual: buf.len(),
                }
                .into(),
            ));
        }
        let (color_type, bit_depth) = self.output_color_type();
        let output_info = OutputInfo {
            width: self.subframe.width,
            height: self.subframe.height,
            color_type,
            bit_depth,
            line_size: self.unguarded_output_line_size(self.subframe.width),
        };
        if self.info().interlaced {
            let stride = self.unguarded_output_line_size(self.info().width);
            let samples = color_type.samples() as u8;
            let bits_pp = samples * (bit_depth as u8);
            let expand = crate::image_codecs::png::adam7::expand_pass;
            while let Some(InterlacedRow {
                data: row,
                interlace,
                ..
            }) = self.next_interlaced_row()?
            {
                let adam7info = interlace.get_adam7_info().unwrap();
                expand(buf, stride, row, adam7info, bits_pp);
            }
        } else {
            let current_interlace_info = self.subframe.current_interlace_info.as_ref();
            let already_done_rows = current_interlace_info
                .map(|info| info.line_number())
                .unwrap_or(self.subframe.height);
            for row in buf
                .chunks_exact_mut(output_info.line_size)
                .take(self.subframe.height as usize)
                .skip(already_done_rows as usize)
            {
                self.next_interlaced_row_impl(self.subframe.rowlen, row)?;
            }
        }
        self.finish_decoding()?;
        Ok(output_info)
    }
    fn mark_subframe_as_consumed_and_flushed(&mut self) {
        assert!(self.remaining_frames > 0);
        self.remaining_frames -= 1;
        self.subframe.consumed_and_flushed = true;
    }
    fn finish_decoding(&mut self) -> Result<(), DecodingError> {
        assert!(self.subframe.current_interlace_info.is_none());
        if !self.subframe.consumed_and_flushed {
            self.decoder.finish_decoding_image_data()?;
            self.mark_subframe_as_consumed_and_flushed();
        }
        Ok(())
    }

    pub fn next_interlaced_row(&mut self) -> Result<Option<InterlacedRow<'_>>, DecodingError> {
        let mut output_buffer = mem::take(&mut self.scratch_buffer);
        let max_line_size = self
            .output_line_size(self.info().width)
            .ok_or(DecodingError::LimitsExceeded)?;
        output_buffer.resize(max_line_size, 0u8);
        let result = self.read_row(&mut output_buffer);
        self.scratch_buffer = output_buffer;
        result.map(move |option| {
            option.map(move |interlace| {
                let output_line_size = self.output_line_size_for_interlace_info(&interlace);
                InterlacedRow {
                    data: &self.scratch_buffer[..output_line_size],
                    interlace,
                }
            })
        })
    }
    pub fn read_row(
        &mut self,
        output_buffer: &mut [u8],
    ) -> Result<Option<InterlaceInfo>, DecodingError> {
        let interlace = match self.subframe.current_interlace_info.as_ref() {
            None => {
                self.finish_decoding()?;
                return Ok(None);
            }
            Some(interlace) => *interlace,
        };
        if interlace.line_number() == 0 {
            self.unfiltering_buffer.reset_prev_row();
        }
        let rowlen = match interlace {
            InterlaceInfo::Null(_) => self.subframe.rowlen,
            InterlaceInfo::Adam7(Adam7Info { samples: width, .. }) => {
                self.info().raw_row_length_from_width(width)
            }
        };
        let output_line_size = self.output_line_size_for_interlace_info(&interlace);
        let output_buffer = &mut output_buffer[..output_line_size];
        self.next_interlaced_row_impl(rowlen, output_buffer)?;
        Ok(Some(interlace))
    }
    fn output_line_size_for_interlace_info(&self, interlace: &InterlaceInfo) -> usize {
        let width = match interlace {
            InterlaceInfo::Adam7(Adam7Info { samples: width, .. }) => *width,
            InterlaceInfo::Null(_) => self.subframe.width,
        };
        self.unguarded_output_line_size(width)
    }
    pub fn finish(&mut self) -> Result<(), DecodingError> {
        if self.finished {
            return Err(DecodingError::Parameter(
                ParameterErrorKind::PolledAfterEndOfImage.into(),
            ));
        }
        self.remaining_frames = 0;
        self.decoder.read_until_end_of_input()?;
        self.finished = true;
        Ok(())
    }
    fn next_interlaced_row_impl(
        &mut self,
        rowlen: usize,
        output_buffer: &mut [u8],
    ) -> Result<(), DecodingError> {
        self.next_raw_interlaced_row(rowlen)?;
        let row = self.unfiltering_buffer.prev_row();
        assert_eq!(row.len(), rowlen - 1);
        let transform_fn = {
            if self.transform_fn.is_none() {
                self.transform_fn = Some(create_transform_fn(self.info(), self.transform)?);
            }
            self.transform_fn.as_deref().unwrap()
        };
        transform_fn(row, output_buffer, self.info());
        self.subframe.current_interlace_info = self.subframe.interlace_info_iter.next();
        Ok(())
    }
    pub fn output_color_type(&self) -> (ColorType, BitDepth) {
        use crate::image_codecs::png::common::ColorType::*;
        let t = self.transform;
        let info = self.info();
        if t == Transformations::IDENTITY {
            (info.color_type, info.bit_depth)
        } else {
            let bits = match info.bit_depth as u8 {
                16 if t.intersects(Transformations::STRIP_16) => 8,
                n if n < 8
                    && (t.contains(Transformations::EXPAND)
                        || t.contains(Transformations::ALPHA)) =>
                {
                    8
                }
                n => n,
            };
            let color_type =
                if t.contains(Transformations::EXPAND) || t.contains(Transformations::ALPHA) {
                    let has_trns = info.trns.is_some() || t.contains(Transformations::ALPHA);
                    match info.color_type {
                        Grayscale if has_trns => GrayscaleAlpha,
                        Rgb if has_trns => Rgba,
                        Indexed if has_trns => Rgba,
                        Indexed => Rgb,
                        ct => ct,
                    }
                } else {
                    info.color_type
                };
            (color_type, BitDepth::from_u8(bits).unwrap())
        }
    }
    pub fn output_buffer_size(&self) -> Option<usize> {
        let (width, height) = self.info().size();
        let (color, depth) = self.output_color_type();
        let linelen = color.checked_raw_row_length(depth, width)?.checked_sub(1)?;
        let height = usize::try_from(height).ok()?;
        let imglen = linelen.checked_mul(height)?;
        (imglen <= isize::MAX as usize).then_some(imglen)
    }
    pub(crate) fn unguarded_output_line_size(&self, width: u32) -> usize {
        let (color, depth) = self.output_color_type();
        color.raw_row_length_from_width(depth, width) - 1
    }
    pub fn output_line_size(&self, width: u32) -> Option<usize> {
        let (color, depth) = self.output_color_type();
        let length = color.checked_raw_row_length(depth, width)?.checked_sub(1)?;
        (length <= isize::MAX as usize).then_some(length)
    }
    fn next_raw_interlaced_row(&mut self, rowlen: usize) -> Result<(), DecodingError> {
        while self.unfiltering_buffer.mutable_len_of_curr_row() < rowlen {
            if self.subframe.consumed_and_flushed {
                return Err(DecodingError::Format(
                    FormatErrorInner::NoMoreImageData.into(),
                ));
            }
            assert!(self.unfiltering_buffer.remaining_bytes() > 0);
            let completion_status = self
                .unfiltering_buffer
                .with_unfilled_buffer(|buffer| self.decoder.decode_image_data(Some(buffer)));
            match completion_status {
                Ok(ImageDataCompletionStatus::ExpectingMoreData) => (),
                Ok(ImageDataCompletionStatus::Done) => self.mark_subframe_as_consumed_and_flushed(),
                Err(DecodingError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof
                        && self.unfiltering_buffer.readable_len_of_curr_row() >= rowlen =>
                {
                    return self
                        .unfiltering_buffer
                        .unfilter_curr_row_using_scratch_buffer(rowlen, self.bpp);
                }
                Err(other_error) => return Err(other_error),
            }
        }
        self.unfiltering_buffer
            .unfilter_curr_row_in_place(rowlen, self.bpp)
    }
}
impl SubframeInfo {
    fn not_yet_init() -> Self {
        SubframeInfo {
            width: 0,
            height: 0,
            rowlen: 0,
            current_interlace_info: None,
            interlace_info_iter: InterlaceInfoIter::empty(),
            consumed_and_flushed: false,
        }
    }
    fn new(info: &Info) -> Self {
        let (width, height) = if let Some(fc) = info.frame_control {
            (fc.width, fc.height)
        } else {
            (info.width, info.height)
        };
        let mut interlace_info_iter = InterlaceInfoIter::new(width, height, info.interlaced);
        let current_interlace_info = interlace_info_iter.next();
        SubframeInfo {
            width,
            height,
            rowlen: info.raw_row_length_from_width(width),
            current_interlace_info,
            interlace_info_iter,
            consumed_and_flushed: false,
        }
    }
}
