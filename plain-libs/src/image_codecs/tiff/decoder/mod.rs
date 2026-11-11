use std::alloc::{Layout, LayoutError};
use std::collections::BTreeMap;
use std::io::{self, Read, Seek};
use std::num::NonZeroUsize;

use crate::image_codecs::tiff::tags::{
    CompressionMethod, IfdPointer, PhotometricInterpretation, PlanarConfiguration, Predictor,
    SampleFormat, Tag, Type, ValueBuffer,
};
use crate::image_codecs::tiff::{
    ColorType, Directory, TiffError, TiffFormatError, TiffResult, TiffUnsupportedError, UsageError,
};

use self::image::Image;
use self::stream::{ByteOrder, EndianReader};

mod cycles;
pub mod ifd;
mod image;
mod stream;
mod tag_reader;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TiffCodingUnit(pub u32);
#[derive(Debug)]
pub enum DecodingResult {
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
    U64(Vec<u64>),
    F16(Vec<u16>),
    F32(Vec<f32>),
    F64(Vec<f64>),
    I8(Vec<i8>),
    I16(Vec<i16>),
    I32(Vec<i32>),
    I64(Vec<i64>),
}

impl DecodingResult {
    pub fn resize_to(
        &mut self,
        buffer: &BufferLayoutPreference,
        limits: &Limits,
    ) -> Result<(), TiffError> {
        let sample_type = buffer.sample_type.ok_or(TiffError::UnsupportedError(
            TiffUnsupportedError::UnknownInterpretation,
        ))?;

        let extent = sample_type.extent_for_bytes(buffer.complete_len);
        self.resize_to_extent(extent, limits)
    }

    fn resize_to_extent(
        &mut self,
        extent: DecodingExtent,
        limits: &Limits,
    ) -> Result<(), TiffError> {
        // FIXME: we *can* reuse the allocation sometimes.
        *self = extent.to_result_buffer(limits)?;
        Ok(())
    }

    fn new<T: Default + Copy>(
        size: usize,
        limits: &Limits,
        from_fn: fn(Vec<T>) -> Self,
    ) -> TiffResult<DecodingResult> {
        if size > limits.decoding_buffer_size / core::mem::size_of::<T>() {
            Err(TiffError::LimitsExceeded)
        } else {
            Ok(from_fn(vec![T::default(); size]))
        }
    }

    fn new_u8(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::U8)
    }

    fn new_u16(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::U16)
    }

    fn new_u32(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::U32)
    }

    fn new_u64(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::U64)
    }

    fn new_f32(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::F32)
    }

    fn new_f64(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::F64)
    }

    fn new_f16(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::F16)
    }

    fn new_i8(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::I8)
    }

    fn new_i16(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::I16)
    }

    fn new_i32(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::I32)
    }

    fn new_i64(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::I64)
    }
    pub fn as_buffer(&mut self, start: usize) -> DecodingBuffer<'_> {
        match *self {
            DecodingResult::U8(ref mut buf) => DecodingBuffer::U8(&mut buf[start..]),
            DecodingResult::U16(ref mut buf) => DecodingBuffer::U16(&mut buf[start..]),
            DecodingResult::U32(ref mut buf) => DecodingBuffer::U32(&mut buf[start..]),
            DecodingResult::U64(ref mut buf) => DecodingBuffer::U64(&mut buf[start..]),
            DecodingResult::F16(ref mut buf) => DecodingBuffer::F16(&mut buf[start..]),
            DecodingResult::F32(ref mut buf) => DecodingBuffer::F32(&mut buf[start..]),
            DecodingResult::F64(ref mut buf) => DecodingBuffer::F64(&mut buf[start..]),
            DecodingResult::I8(ref mut buf) => DecodingBuffer::I8(&mut buf[start..]),
            DecodingResult::I16(ref mut buf) => DecodingBuffer::I16(&mut buf[start..]),
            DecodingResult::I32(ref mut buf) => DecodingBuffer::I32(&mut buf[start..]),
            DecodingResult::I64(ref mut buf) => DecodingBuffer::I64(&mut buf[start..]),
        }
    }
}

// A buffer for image decoding
pub enum DecodingBuffer<'a> {
    U8(&'a mut [u8]),
    U16(&'a mut [u16]),
    U32(&'a mut [u32]),
    U64(&'a mut [u64]),
    F16(&'a mut [u16]),
    F32(&'a mut [f32]),
    F64(&'a mut [f64]),
    I8(&'a mut [i8]),
    I16(&'a mut [i16]),
    I32(&'a mut [i32]),
    I64(&'a mut [i64]),
}

impl<'a> DecodingBuffer<'a> {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            DecodingBuffer::U8(buf) => buf,
            DecodingBuffer::I8(buf) => crate::byte_cast::cast_slice(buf),
            DecodingBuffer::U16(buf) => crate::byte_cast::cast_slice(buf),
            DecodingBuffer::I16(buf) => crate::byte_cast::cast_slice(buf),
            DecodingBuffer::U32(buf) => crate::byte_cast::cast_slice(buf),
            DecodingBuffer::I32(buf) => crate::byte_cast::cast_slice(buf),
            DecodingBuffer::U64(buf) => crate::byte_cast::cast_slice(buf),
            DecodingBuffer::I64(buf) => crate::byte_cast::cast_slice(buf),
            DecodingBuffer::F16(buf) => crate::byte_cast::cast_slice(buf),
            DecodingBuffer::F32(buf) => crate::byte_cast::cast_slice(buf),
            DecodingBuffer::F64(buf) => crate::byte_cast::cast_slice(buf),
        }
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        match self {
            DecodingBuffer::U8(buf) => buf,
            DecodingBuffer::I8(buf) => crate::byte_cast::cast_slice_mut(buf),
            DecodingBuffer::U16(buf) => crate::byte_cast::cast_slice_mut(buf),
            DecodingBuffer::I16(buf) => crate::byte_cast::cast_slice_mut(buf),
            DecodingBuffer::U32(buf) => crate::byte_cast::cast_slice_mut(buf),
            DecodingBuffer::I32(buf) => crate::byte_cast::cast_slice_mut(buf),
            DecodingBuffer::U64(buf) => crate::byte_cast::cast_slice_mut(buf),
            DecodingBuffer::I64(buf) => crate::byte_cast::cast_slice_mut(buf),
            DecodingBuffer::F16(buf) => crate::byte_cast::cast_slice_mut(buf),
            DecodingBuffer::F32(buf) => crate::byte_cast::cast_slice_mut(buf),
            DecodingBuffer::F64(buf) => crate::byte_cast::cast_slice_mut(buf),
        }
    }

    pub fn byte_len(&self) -> usize {
        self.as_bytes().len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecodingSampleType {
    U8,
    U16,
    U32,
    U64,
    F16,
    F32,
    F64,
    I8,
    I16,
    I32,
    I64,
}

impl DecodingSampleType {
    fn extent_for_bytes(self, bytes: usize) -> DecodingExtent {
        match self {
            DecodingSampleType::U8 => DecodingExtent::U8(bytes),
            DecodingSampleType::U16 => DecodingExtent::U16(bytes.div_ceil(2)),
            DecodingSampleType::U32 => DecodingExtent::U32(bytes.div_ceil(4)),
            DecodingSampleType::U64 => DecodingExtent::U64(bytes.div_ceil(8)),
            DecodingSampleType::I8 => DecodingExtent::I8(bytes),
            DecodingSampleType::I16 => DecodingExtent::I16(bytes.div_ceil(2)),
            DecodingSampleType::I32 => DecodingExtent::I32(bytes.div_ceil(4)),
            DecodingSampleType::I64 => DecodingExtent::I64(bytes.div_ceil(8)),
            DecodingSampleType::F16 => DecodingExtent::F16(bytes.div_ceil(2)),
            DecodingSampleType::F32 => DecodingExtent::F32(bytes.div_ceil(4)),
            DecodingSampleType::F64 => DecodingExtent::F64(bytes.div_ceil(8)),
        }
    }
}
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct BufferLayoutPreference {
    pub len: usize,
    pub sample_format: SampleFormat,
    pub sample_type: Option<DecodingSampleType>,
    pub row_stride: Option<NonZeroUsize>,
    pub planes: usize,
    pub plane_stride: Option<NonZeroUsize>,
    pub complete_len: usize,
}

impl BufferLayoutPreference {
    fn from_planes(layout: &image::PlaneLayout) -> Self {
        BufferLayoutPreference {
            len: layout.readout.plane_stride,
            row_stride: core::num::NonZeroUsize::new(layout.readout.row_stride),
            planes: layout.plane_offsets.len(),
            plane_stride: core::num::NonZeroUsize::new(layout.readout.plane_stride),
            complete_len: layout.total_bytes,
            sample_format: layout.readout.sample_format,
            sample_type: Self::sample_type(layout.readout.sample_format, layout.readout.color),
        }
    }

    fn sample_type(sample_format: SampleFormat, color: ColorType) -> Option<DecodingSampleType> {
        Some(match sample_format {
            SampleFormat::Uint => match color.bit_depth() {
                n if n <= 8 => DecodingSampleType::U8,
                n if n <= 16 => DecodingSampleType::U16,
                n if n <= 32 => DecodingSampleType::U32,
                n if n <= 64 => DecodingSampleType::U64,
                _ => return None,
            },
            SampleFormat::IEEEFP => match color.bit_depth() {
                16 => DecodingSampleType::F16,
                32 => DecodingSampleType::F32,
                64 => DecodingSampleType::F64,
                _ => return None,
            },
            SampleFormat::Int => match color.bit_depth() {
                n if n <= 8 => DecodingSampleType::I8,
                n if n <= 16 => DecodingSampleType::I16,
                n if n <= 32 => DecodingSampleType::I32,
                n if n <= 64 => DecodingSampleType::I64,
                _ => return None,
            },
            _other => {
                return None;
            }
        })
    }
}

impl image::ReadoutLayout {
    // FIXME: when planes are not homogenous (i.e. subsampled or differing depths) then the
    // `readout_for_size` or `to_plane_layout` needs a parameter to determine the planes being
    // read instead of assuming a constant repeated size for them.
    fn result_extent_for_planes(
        self: &image::ReadoutLayout,
        planes: core::ops::Range<u16>,
    ) -> TiffResult<DecodingExtent> {
        let buffer = self.to_plane_layout()?;

        // The layout is for all planes. So restrict ourselves to the planes that were requested.
        let offset = match buffer.plane_offsets.get(usize::from(planes.start)) {
            Some(n) => *n,
            None => {
                return Err(TiffError::UsageError(UsageError::InvalidPlaneIndex(
                    planes.start,
                )));
            }
        };

        let end = match buffer.plane_offsets.get(usize::from(planes.end)) {
            Some(n) => *n,
            None => buffer.total_bytes,
        };

        let buffer_bytes = end - offset;
        let bits_per_sample = self.color.bit_depth();

        let Some(sample_type) = BufferLayoutPreference::sample_type(self.sample_format, self.color)
        else {
            if matches!(
                self.sample_format,
                SampleFormat::Uint | SampleFormat::Int | SampleFormat::IEEEFP
            ) {
                return Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::UnsupportedSampleDepth(bits_per_sample),
                ));
            } else {
                return Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::UnsupportedSampleFormat(vec![self.sample_format]),
                ));
            }
        };

        Ok(sample_type.extent_for_bytes(buffer_bytes))
    }

    #[inline(always)]
    fn assert_min_layout<T>(&self, buffer: &[T]) -> TiffResult<()> {
        if core::mem::size_of_val(buffer) < self.plane_stride {
            Err(TiffError::UsageError(
                UsageError::InsufficientOutputBufferSize {
                    needed: self.plane_stride,
                    provided: buffer.len(),
                },
            ))
        } else {
            Ok(())
        }
    }
}
#[derive(Clone)]
enum DecodingExtent {
    U8(usize),
    U16(usize),
    U32(usize),
    U64(usize),
    F16(usize),
    F32(usize),
    F64(usize),
    I8(usize),
    I16(usize),
    I32(usize),
    I64(usize),
}

impl DecodingExtent {
    fn to_result_buffer(&self, limits: &Limits) -> TiffResult<DecodingResult> {
        match *self {
            DecodingExtent::U8(count) => DecodingResult::new_u8(count, limits),
            DecodingExtent::U16(count) => DecodingResult::new_u16(count, limits),
            DecodingExtent::U32(count) => DecodingResult::new_u32(count, limits),
            DecodingExtent::U64(count) => DecodingResult::new_u64(count, limits),
            DecodingExtent::F16(count) => DecodingResult::new_f16(count, limits),
            DecodingExtent::F32(count) => DecodingResult::new_f32(count, limits),
            DecodingExtent::F64(count) => DecodingResult::new_f64(count, limits),
            DecodingExtent::I8(count) => DecodingResult::new_i8(count, limits),
            DecodingExtent::I16(count) => DecodingResult::new_i16(count, limits),
            DecodingExtent::I32(count) => DecodingResult::new_i32(count, limits),
            DecodingExtent::I64(count) => DecodingResult::new_i64(count, limits),
        }
    }

    fn preferred_layout(self) -> TiffResult<Layout> {
        fn overflow(_: LayoutError) -> TiffError {
            TiffError::LimitsExceeded
        }

        match self {
            DecodingExtent::U8(count) => Layout::array::<u8>(count),
            DecodingExtent::U16(count) => Layout::array::<u16>(count),
            DecodingExtent::U32(count) => Layout::array::<u32>(count),
            DecodingExtent::U64(count) => Layout::array::<u64>(count),
            DecodingExtent::F16(count) => Layout::array::<u16>(count),
            DecodingExtent::F32(count) => Layout::array::<f32>(count),
            DecodingExtent::F64(count) => Layout::array::<f64>(count),
            DecodingExtent::I8(count) => Layout::array::<i8>(count),
            DecodingExtent::I16(count) => Layout::array::<i16>(count),
            DecodingExtent::I32(count) => Layout::array::<i32>(count),
            DecodingExtent::I64(count) => Layout::array::<i64>(count),
        }
        .map_err(overflow)
    }

    fn sample_type(&self) -> DecodingSampleType {
        match *self {
            DecodingExtent::U8(_) => DecodingSampleType::U8,
            DecodingExtent::U16(_) => DecodingSampleType::U16,
            DecodingExtent::U32(_) => DecodingSampleType::U32,
            DecodingExtent::U64(_) => DecodingSampleType::U64,
            DecodingExtent::F16(_) => DecodingSampleType::F16,
            DecodingExtent::F32(_) => DecodingSampleType::F32,
            DecodingExtent::F64(_) => DecodingSampleType::F64,
            DecodingExtent::I8(_) => DecodingSampleType::I8,
            DecodingExtent::I16(_) => DecodingSampleType::I16,
            DecodingExtent::I32(_) => DecodingSampleType::I32,
            DecodingExtent::I64(_) => DecodingSampleType::I64,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ChunkType {
    Strip,
    Tile,
}
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Limits {
    pub decoding_buffer_size: usize,
    pub ifd_value_size: usize,
    pub intermediate_buffer_size: usize,
}

impl Limits {
    pub fn unlimited() -> Limits {
        Limits {
            decoding_buffer_size: usize::MAX,
            ifd_value_size: usize::MAX,
            intermediate_buffer_size: usize::MAX,
        }
    }
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            decoding_buffer_size: 256 * 1024 * 1024,
            intermediate_buffer_size: 128 * 1024 * 1024,
            ifd_value_size: 1024 * 1024,
        }
    }
}
#[derive(Debug)]
pub struct Decoder<R>
where
    R: Read + Seek,
{
    value_reader: ValueReader<R>,
    current_ifd: Option<IfdPointer>,
    next_ifd: Option<IfdPointer>,
    ifd_offsets: Vec<IfdPointer>,
    seen_ifds: cycles::IfdCycles,
    image: Image,
}
#[derive(Debug)]
struct ValueReader<R> {
    reader: EndianReader<R>,
    bigtiff: bool,
    limits: Limits,
}
pub struct IfdDecoder<'lt> {
    inner: tag_reader::TagReader<'lt, dyn tag_reader::EntryDecoder + 'lt>,
}

fn rev_hpredict_nsamp(buf: &mut [u8], bit_depth: u8, samples: u16) {
    fn one_byte_predict<const N: usize>(buf: &mut [u8]) {
        for i in N..buf.len() {
            buf[i] = buf[i].wrapping_add(buf[i - N]);
        }
    }

    fn two_bytes_predict<const N: usize>(buf: &mut [u8]) {
        for i in (2 * N..buf.len()).step_by(2) {
            let v = u16::from_ne_bytes(buf[i..][..2].try_into().unwrap());
            let p = u16::from_ne_bytes(buf[i - 2 * N..][..2].try_into().unwrap());
            buf[i..][..2].copy_from_slice(&(v.wrapping_add(p)).to_ne_bytes());
        }
    }

    fn four_bytes_predict<const N: usize>(buf: &mut [u8]) {
        for i in (N * 4..buf.len()).step_by(4) {
            let v = u32::from_ne_bytes(buf[i..][..4].try_into().unwrap());
            let p = u32::from_ne_bytes(buf[i - 4 * N..][..4].try_into().unwrap());
            buf[i..][..4].copy_from_slice(&(v.wrapping_add(p)).to_ne_bytes());
        }
    }

    let samples = usize::from(samples);

    match (bit_depth, samples) {
        // Note we can't use `windows` or so due to the overlap between each iteration. We split
        // the cases by the samples / lookback constant so that each is optimized individually.
        // This is more code generated but each loop can then have a different vectorization
        // strategy.
        (0..=8, 1) => one_byte_predict::<1>(buf),
        (0..=8, 2) => one_byte_predict::<2>(buf),
        (0..=8, 3) => one_byte_predict::<3>(buf),
        (0..=8, 4) => one_byte_predict::<4>(buf),
        // The generic, sub-optimal case for the above.
        (0..=8, _) => {
            for i in samples..buf.len() {
                buf[i] = buf[i].wrapping_add(buf[i - samples]);
            }
        }
        (9..=16, 1) => {
            two_bytes_predict::<1>(buf);
        }
        (9..=16, 2) => {
            two_bytes_predict::<2>(buf);
        }
        (9..=16, 3) => {
            two_bytes_predict::<3>(buf);
        }
        (9..=16, 4) => {
            two_bytes_predict::<4>(buf);
        }
        (9..=16, _) => {
            for i in (samples * 2..buf.len()).step_by(2) {
                let v = u16::from_ne_bytes(buf[i..][..2].try_into().unwrap());
                let p = u16::from_ne_bytes(buf[i - 2 * samples..][..2].try_into().unwrap());
                buf[i..][..2].copy_from_slice(&(v.wrapping_add(p)).to_ne_bytes());
            }
        }
        (17..=32, 1) => {
            four_bytes_predict::<1>(buf);
        }
        (17..=32, 2) => {
            four_bytes_predict::<2>(buf);
        }
        (17..=32, 3) => {
            four_bytes_predict::<3>(buf);
        }
        (17..=32, 4) => {
            four_bytes_predict::<4>(buf);
        }
        (17..=32, _) => {
            for i in (samples * 4..buf.len()).step_by(4) {
                let v = u32::from_ne_bytes(buf[i..][..4].try_into().unwrap());
                let p = u32::from_ne_bytes(buf[i - 4 * samples..][..4].try_into().unwrap());
                buf[i..][..4].copy_from_slice(&(v.wrapping_add(p)).to_ne_bytes());
            }
        }
        (33..=64, _) => {
            for i in (samples * 8..buf.len()).step_by(8) {
                let v = u64::from_ne_bytes(buf[i..][..8].try_into().unwrap());
                let p = u64::from_ne_bytes(buf[i - 8 * samples..][..8].try_into().unwrap());
                buf[i..][..8].copy_from_slice(&(v.wrapping_add(p)).to_ne_bytes());
            }
        }
        _ => {
            unreachable!("Caller should have validated arguments. Please file a bug.")
        }
    }
}

fn predict_f32(input: &mut [u8], output: &mut [u8], samples: u16) {
    let samples = usize::from(samples);

    for i in samples..input.len() {
        input[i] = input[i].wrapping_add(input[i - samples]);
    }

    for (i, chunk) in output.chunks_mut(4).enumerate() {
        chunk.copy_from_slice(&u32::to_ne_bytes(u32::from_be_bytes([
            input[i],
            input[input.len() / 4 + i],
            input[input.len() / 4 * 2 + i],
            input[input.len() / 4 * 3 + i],
        ])));
    }
}

fn predict_f16(input: &mut [u8], output: &mut [u8], samples: u16) {
    let samples = usize::from(samples);

    for i in samples..input.len() {
        input[i] = input[i].wrapping_add(input[i - samples]);
    }

    for (i, chunk) in output.chunks_mut(2).enumerate() {
        chunk.copy_from_slice(&u16::to_ne_bytes(u16::from_be_bytes([
            input[i],
            input[input.len() / 2 + i],
        ])));
    }
}

fn predict_f64(input: &mut [u8], output: &mut [u8], samples: u16) {
    let samples = usize::from(samples);

    for i in samples..input.len() {
        input[i] = input[i].wrapping_add(input[i - samples]);
    }

    for (i, chunk) in output.chunks_mut(8).enumerate() {
        chunk.copy_from_slice(&u64::to_ne_bytes(u64::from_be_bytes([
            input[i],
            input[input.len() / 8 + i],
            input[input.len() / 8 * 2 + i],
            input[input.len() / 8 * 3 + i],
            input[input.len() / 8 * 4 + i],
            input[input.len() / 8 * 5 + i],
            input[input.len() / 8 * 6 + i],
            input[input.len() / 8 * 7 + i],
        ])));
    }
}

fn fix_endianness_and_predict(
    buf: &mut [u8],
    bit_depth: u8,
    samples: u16,
    byte_order: ByteOrder,
    predictor: Predictor,
) {
    match predictor {
        Predictor::None => {
            fix_endianness(buf, byte_order, bit_depth);
        }
        Predictor::Horizontal => {
            fix_endianness(buf, byte_order, bit_depth);
            rev_hpredict_nsamp(buf, bit_depth, samples);
        }
        Predictor::FloatingPoint => {
            let mut buffer_copy = buf.to_vec();
            match bit_depth {
                16 => predict_f16(&mut buffer_copy, buf, samples),
                32 => predict_f32(&mut buffer_copy, buf, samples),
                64 => predict_f64(&mut buffer_copy, buf, samples),
                _ => unreachable!("Caller should have validated arguments. Please file a bug."),
            }
        }
    }
}

fn invert_colors(
    buf: &mut [u8],
    color_type: ColorType,
    sample_format: SampleFormat,
) -> TiffResult<()> {
    match (color_type, sample_format) {
        // Where pixels do not cross a byte boundary
        (ColorType::Gray(1 | 2 | 4 | 8), SampleFormat::Uint) => {
            for x in buf {
                // Equivalent to both of the following:
                //
                // *x = 0xff - *x
                // *x = !*x
                //
                // since -x = !x+1
                *x = !*x;
            }
        }
        (ColorType::Gray(16), SampleFormat::Uint) => {
            for x in buf.chunks_mut(2) {
                let v = u16::from_ne_bytes(x.try_into().unwrap());
                x.copy_from_slice(&(0xffff - v).to_ne_bytes());
            }
        }
        (ColorType::Gray(32), SampleFormat::Uint) => {
            for x in buf.chunks_mut(4) {
                let v = u32::from_ne_bytes(x.try_into().unwrap());
                x.copy_from_slice(&(0xffff_ffff - v).to_ne_bytes());
            }
        }
        (ColorType::Gray(64), SampleFormat::Uint) => {
            for x in buf.chunks_mut(8) {
                let v = u64::from_ne_bytes(x.try_into().unwrap());
                x.copy_from_slice(&(0xffff_ffff_ffff_ffff - v).to_ne_bytes());
            }
        }
        (ColorType::Gray(32), SampleFormat::IEEEFP) => {
            for x in buf.chunks_mut(4) {
                let v = f32::from_ne_bytes(x.try_into().unwrap());
                x.copy_from_slice(&(1.0 - v).to_ne_bytes());
            }
        }
        (ColorType::Gray(64), SampleFormat::IEEEFP) => {
            for x in buf.chunks_mut(8) {
                let v = f64::from_ne_bytes(x.try_into().unwrap());
                x.copy_from_slice(&(1.0 - v).to_ne_bytes());
            }
        }
        _ => {
            return Err(TiffError::UnsupportedError(
                TiffUnsupportedError::UnknownInterpretation,
            ));
        }
    }

    Ok(())
}
fn fix_endianness(buf: &mut [u8], byte_order: ByteOrder, bit_depth: u8) {
    let host = ByteOrder::native();

    let class = match bit_depth {
        0..=8 => crate::image_codecs::tiff::tags::EndianBytes::One,
        9..=16 => crate::image_codecs::tiff::tags::EndianBytes::Two,
        17..=32 => crate::image_codecs::tiff::tags::EndianBytes::Four,
        _ => crate::image_codecs::tiff::tags::EndianBytes::Eight,
    };

    host.convert_endian_bytes(class, buf, byte_order);
}

impl<R: Read + Seek> Decoder<R> {
    pub fn new(mut r: R) -> TiffResult<Decoder<R>> {
        let mut endianess = Vec::with_capacity(2);
        (&mut r).take(2).read_to_end(&mut endianess)?;
        let byte_order = match &*endianess {
            b"II" => ByteOrder::LittleEndian,
            b"MM" => ByteOrder::BigEndian,
            _ => {
                return Err(TiffError::FormatError(
                    TiffFormatError::TiffSignatureNotFound,
                ));
            }
        };
        let mut reader = EndianReader::new(r, byte_order);

        let bigtiff = match reader.read_u16()? {
            42 => false,
            43 => {
                // Read bytesize of offsets (in bigtiff it's alway 8 but provide a way to move to 16 some day)
                if reader.read_u16()? != 8 {
                    return Err(TiffError::FormatError(
                        TiffFormatError::TiffSignatureNotFound,
                    ));
                }
                // This constant should always be 0
                if reader.read_u16()? != 0 {
                    return Err(TiffError::FormatError(
                        TiffFormatError::TiffSignatureNotFound,
                    ));
                }
                true
            }
            _ => {
                return Err(TiffError::FormatError(
                    TiffFormatError::TiffSignatureInvalid,
                ));
            }
        };

        let next_ifd = if bigtiff {
            Some(reader.read_u64()?)
        } else {
            Some(u64::from(reader.read_u32()?))
        }
        .map(IfdPointer);

        let current_ifd = *next_ifd.as_ref().unwrap();
        let ifd_offsets = vec![current_ifd];

        let mut decoder = Decoder {
            value_reader: ValueReader {
                reader,
                bigtiff,
                limits: Default::default(),
            },
            next_ifd,
            ifd_offsets,
            current_ifd: None,
            seen_ifds: cycles::IfdCycles::new(),
            image: Image {
                ifd: None,
                width: 0,
                height: 0,
                bits_per_sample: 1,
                samples: 1,
                extra_samples: vec![],
                photometric_samples: 1,
                sample_format: SampleFormat::Uint,
                photometric_interpretation: PhotometricInterpretation::BlackIsZero,
                compression_method: CompressionMethod::None,
                jpeg_tables: None,
                predictor: Predictor::None,
                chunk_type: ChunkType::Strip,
                planar_config: PlanarConfiguration::Chunky,
                strip_decoder: None,
                tile_attributes: None,
                chunk_offsets: Vec::new(),
                chunk_bytes: Vec::new(),
                chroma_subsampling: (2, 2),
            },
        };
        decoder.next_image()?;
        Ok(decoder)
    }

    fn validate_chunk_extents(&mut self) -> TiffResult<()> {
        use std::io::SeekFrom;
        let reader = self.value_reader.reader.inner();
        let position = reader.stream_position()?;
        let length = reader.seek(SeekFrom::End(0))?;
        reader.seek(SeekFrom::Start(position))?;
        if self
            .image
            .chunk_offsets
            .iter()
            .zip(&self.image.chunk_bytes)
            .any(|(offset, size)| offset.checked_add(*size).is_none_or(|end| end > length))
        {
            return Err(TiffError::IoError(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "TIFF chunk extends beyond source",
            )));
        }
        Ok(())
    }

    pub fn with_limits(mut self, limits: Limits) -> Decoder<R> {
        self.value_reader.limits = limits;
        self
    }

    pub fn dimensions(&mut self) -> TiffResult<(u32, u32)> {
        Ok((self.image().width, self.image().height))
    }

    pub fn colortype(&mut self) -> TiffResult<ColorType> {
        self.image().colortype()
    }

    fn image(&self) -> &Image {
        &self.image
    }

    fn next_ifd(&mut self) -> TiffResult<Directory> {
        let Some(next_ifd) = self.next_ifd.take() else {
            return Err(TiffError::FormatError(
                TiffFormatError::ImageFileDirectoryNotFound,
            ));
        };

        let ifd = self.value_reader.read_directory(next_ifd)?;

        // Ensure this walk does not get us into a cycle.
        self.seen_ifds.insert_next(next_ifd, ifd.next())?;

        // Extend the list of known IFD offsets in this chain, if needed.
        if self.ifd_offsets.last().copied() == self.current_ifd {
            self.ifd_offsets.push(next_ifd);
        }

        self.current_ifd = Some(next_ifd);
        self.next_ifd = ifd.next();

        Ok(ifd)
    }
    pub fn next_image(&mut self) -> TiffResult<()> {
        let ifd = self.next_ifd()?;
        self.image = Image::from_reader(&mut self.value_reader, ifd)?;
        self.validate_chunk_extents()?;
        Ok(())
    }

    pub fn byte_order(&self) -> ByteOrder {
        self.value_reader.reader.byte_order
    }

    pub fn inner(&mut self) -> &mut R {
        self.value_reader.reader.inner()
    }
    #[inline]
    pub fn read_byte(&mut self) -> Result<u8, io::Error> {
        let mut buf = [0; 1];
        self.value_reader.reader.inner().read_exact(&mut buf)?;
        Ok(buf[0])
    }

    #[inline]
    pub fn read_long(&mut self) -> Result<u32, io::Error> {
        self.value_reader.reader.read_u32()
    }

    #[inline]
    pub fn read_long8(&mut self) -> Result<u64, io::Error> {
        self.value_reader.reader.read_u64()
    }

    #[inline]
    pub fn read_string(&mut self, length: usize) -> TiffResult<String> {
        let mut out = vec![0; length];
        self.value_reader.reader.inner().read_exact(&mut out)?;
        // Strings may be null-terminated, so we trim anything downstream of the null byte
        if let Some(first) = out.iter().position(|&b| b == 0) {
            out.truncate(first);
        }
        Ok(String::from_utf8(out)?)
    }

    #[inline]
    pub fn goto_offset(&mut self, offset: u32) -> io::Result<()> {
        self.goto_offset_u64(offset.into())
    }

    #[inline]
    pub fn goto_offset_u64(&mut self, offset: u64) -> io::Result<()> {
        self.value_reader.reader.goto_offset(offset)
    }
    pub fn read_directory(&mut self, ptr: IfdPointer) -> TiffResult<Directory> {
        self.value_reader.read_directory(ptr)
    }

    fn check_chunk_type(&self, expected: ChunkType) -> TiffResult<()> {
        if expected != self.image().chunk_type {
            return Err(TiffError::UsageError(UsageError::InvalidChunkType(
                expected,
                self.image().chunk_type,
            )));
        }

        Ok(())
    }
    pub fn get_chunk_type(&self) -> ChunkType {
        self.image().chunk_type
    }

    pub fn tile_count(&mut self) -> TiffResult<u32> {
        self.check_chunk_type(ChunkType::Tile)?;
        Ok(u32::try_from(self.image().chunk_offsets.len())?)
    }

    fn read_chunk_to_bytes(
        &mut self,
        buffer: &mut [u8],
        chunk_index: u32,
        layout: &image::ReadoutLayout,
    ) -> TiffResult<()> {
        let offset = self.image.chunk_file_range(chunk_index)?.0;
        self.goto_offset_u64(offset)?;

        self.image
            .expand_chunk(&mut self.value_reader, buffer, layout, chunk_index)?;

        Ok(())
    }
    pub fn image_chunk_buffer_layout(
        &mut self,
        chunk_index: u32,
    ) -> TiffResult<BufferLayoutPreference> {
        let data_dims = self.image().chunk_data_dimensions(chunk_index)?;
        let readout = self.image().readout_for_size(data_dims.0, data_dims.1)?;

        let extent = readout.result_extent_for_planes(0..1)?;
        let sample_type = extent.sample_type();
        let layout = extent.preferred_layout()?;

        let row_stride = core::num::NonZeroUsize::new(readout.minimum_row_stride);
        let plane_stride = core::num::NonZeroUsize::new(readout.plane_stride);

        Ok(BufferLayoutPreference {
            len: layout.size(),
            row_stride,
            planes: 1,
            plane_stride,
            complete_len: layout.size(),
            sample_format: self.image().sample_format,
            sample_type: Some(sample_type),
        })
    }

    pub fn read_chunk(&mut self, chunk_index: u32) -> TiffResult<DecodingResult> {
        let (width, height) = self.image().chunk_data_dimensions(chunk_index)?;

        let readout = self.image().readout_for_size(width, height)?;

        let mut result = readout
            .result_extent_for_planes(0..1)?
            .to_result_buffer(&self.value_reader.limits)?;

        self.read_chunk_to_bytes(result.as_buffer(0).as_bytes_mut(), chunk_index, &readout)?;

        Ok(result)
    }
    pub fn read_chunk_bytes(&mut self, chunk_index: u32, buffer: &mut [u8]) -> TiffResult<()> {
        let (width, height) = self.image().chunk_data_dimensions(chunk_index)?;

        let layout = self.image().readout_for_size(width, height)?;
        layout.assert_min_layout(buffer)?;

        self.read_chunk_to_bytes(buffer, chunk_index, &layout)?;

        Ok(())
    }

    pub fn chunk_dimensions(&self) -> (u32, u32) {
        self.image().chunk_dimensions().unwrap()
    }
    pub fn chunk_data_dimensions(&self, chunk_index: u32) -> (u32, u32) {
        self.image()
            .chunk_data_dimensions(chunk_index)
            .expect("invalid chunk_index")
    }

    pub fn read_image(&mut self) -> TiffResult<DecodingResult> {
        let readout = self.image().readout_for_image()?;

        let mut result = readout
            .result_extent_for_planes(0..1)?
            .to_result_buffer(&self.value_reader.limits)?;

        self.read_image_bytes(result.as_buffer(0).as_bytes_mut())?;

        Ok(result)
    }
    pub fn read_image_to_buffer(
        &mut self,
        result: &mut DecodingResult,
    ) -> TiffResult<BufferLayoutPreference> {
        let readout = self.image().readout_for_image()?;
        let planes = readout.to_plane_layout()?;

        let num_planes = if planes.total_bytes <= self.value_reader.limits.decoding_buffer_size {
            planes.plane_offsets.len() as u16
        } else {
            1
        };

        let layout = BufferLayoutPreference::from_planes(&planes);
        let extent = readout.result_extent_for_planes(0..num_planes)?;
        // Compatibility: if this extent is too large our configured limits we will fall back to
        // reading only the first plane.
        result.resize_to_extent(extent, &self.value_reader.limits)?;

        self.read_image_bytes(result.as_buffer(0).as_bytes_mut())?;

        Ok(layout)
    }
    pub fn read_image_bytes(&mut self, buffer: &mut [u8]) -> TiffResult<()> {
        let readout = self.image().readout_for_image()?;

        let ref layout @ image::PlaneLayout {
            ref plane_offsets,
            // We assume that is correct, so really it can be ignored.
            total_bytes: _,
            ref readout,
        } = readout.to_plane_layout()?;

        let used_plane_offsets = usize::from(layout.used_planes(buffer)?);
        debug_assert!(used_plane_offsets >= 1, "Should have errored");

        // For multi-band images, only the first band is read.
        // Possible improvements:
        // * pass requested band as parameter
        // * collect bands to a RGB encoding result in case of RGB bands
        for chunk in 0..readout.chunks_per_plane {
            let x = (chunk % readout.chunks_across) as usize;
            let y = (chunk / readout.chunks_across) as usize;

            let buffer_offset = y * readout.chunk_col_stride + x * readout.chunk_row_stride;

            for (idx, &plane_offset) in plane_offsets[..used_plane_offsets].iter().enumerate() {
                let chunk = chunk + idx as u32 * readout.chunks_per_plane;
                self.goto_offset_u64(self.image().chunk_offsets[chunk as usize])?;

                self.image.expand_chunk(
                    &mut self.value_reader,
                    &mut buffer[plane_offset..][buffer_offset..],
                    readout,
                    chunk,
                )?;
            }
        }

        Ok(())
    }
    pub fn image_ifd(&mut self) -> IfdDecoder<'_> {
        IfdDecoder {
            inner: tag_reader::TagReader {
                decoder: &mut self.value_reader,
                ifd: self.image.ifd.as_ref().unwrap(),
            },
        }
    }

    pub fn find_tag(&mut self, tag: Tag) -> TiffResult<Option<ifd::Value>> {
        self.image_ifd().find_tag(tag)
    }
    pub fn find_tag_unsigned<T: TryFrom<u64>>(&mut self, tag: Tag) -> TiffResult<Option<T>> {
        self.image_ifd().find_tag_unsigned(tag)
    }
    pub fn find_tag_unsigned_vec<T: TryFrom<u64>>(
        &mut self,
        tag: Tag,
    ) -> TiffResult<Option<Vec<T>>> {
        self.image_ifd().find_tag_unsigned_vec(tag)
    }
    pub fn get_tag_unsigned<T: TryFrom<u64>>(&mut self, tag: Tag) -> TiffResult<T> {
        self.image_ifd().get_tag_unsigned(tag)
    }
    pub fn get_tag(&mut self, tag: Tag) -> TiffResult<ifd::Value> {
        self.image_ifd().get_tag(tag)
    }

    pub fn get_tag_u32(&mut self, tag: Tag) -> TiffResult<u32> {
        self.get_tag(tag)?.into_u32()
    }

    pub fn get_tag_u64(&mut self, tag: Tag) -> TiffResult<u64> {
        self.get_tag(tag)?.into_u64()
    }
    pub fn get_tag_f32(&mut self, tag: Tag) -> TiffResult<f32> {
        self.get_tag(tag)?.into_f32()
    }
    pub fn get_tag_f64(&mut self, tag: Tag) -> TiffResult<f64> {
        self.get_tag(tag)?.into_f64()
    }
    pub fn get_tag_u32_vec(&mut self, tag: Tag) -> TiffResult<Vec<u32>> {
        self.get_tag(tag)?.into_u32_vec()
    }

    pub fn get_tag_u16_vec(&mut self, tag: Tag) -> TiffResult<Vec<u16>> {
        self.get_tag(tag)?.into_u16_vec()
    }

    pub fn get_tag_u64_vec(&mut self, tag: Tag) -> TiffResult<Vec<u64>> {
        self.get_tag(tag)?.into_u64_vec()
    }
    pub fn get_tag_f32_vec(&mut self, tag: Tag) -> TiffResult<Vec<f32>> {
        self.get_tag(tag)?.into_f32_vec()
    }
    pub fn get_tag_f64_vec(&mut self, tag: Tag) -> TiffResult<Vec<f64>> {
        self.get_tag(tag)?.into_f64_vec()
    }
    pub fn get_tag_u8_vec(&mut self, tag: Tag) -> TiffResult<Vec<u8>> {
        self.get_tag(tag)?.into_u8_vec()
    }
    pub fn get_tag_ascii_string(&mut self, tag: Tag) -> TiffResult<String> {
        self.get_tag(tag)?.into_string()
    }

    pub fn tag_iter(&mut self) -> impl Iterator<Item = TiffResult<(Tag, ifd::Value)>> + '_ {
        self.image_ifd().tag_iter()
    }
}

impl<R: Seek + Read> ValueReader<R> {
    pub(crate) fn read_directory(&mut self, ptr: IfdPointer) -> Result<Directory, TiffError> {
        Self::read_ifd(&mut self.reader, self.bigtiff, ptr)
    }
    // An IFD entry has four fields:
    //
    // Tag   2 bytes
    // Type  2 bytes
    // Count 4 bytes
    // Value 4 bytes either a pointer the value itself
    fn read_entry(
        reader: &mut EndianReader<R>,
        bigtiff: bool,
    ) -> TiffResult<Option<(Tag, ifd::Entry)>> {
        let tag = Tag::from_u16_exhaustive(reader.read_u16()?);
        let type_ = match Type::from_u16(reader.read_u16()?) {
            Some(t) => t,
            None => {
                // Unknown type. Skip this entry according to spec.
                reader.read_u32()?;
                reader.read_u32()?;
                return Ok(None);
            }
        };
        let entry = if bigtiff {
            let mut offset = [0; 8];

            let count = reader.read_u64()?;
            reader.inner().read_exact(&mut offset)?;
            ifd::Entry::new_u64(type_, count, offset)
        } else {
            let mut offset = [0; 4];

            let count = reader.read_u32()?;
            reader.inner().read_exact(&mut offset)?;
            ifd::Entry::new(type_, count, offset)
        };
        Ok(Some((tag, entry)))
    }
    fn read_ifd(
        reader: &mut EndianReader<R>,
        bigtiff: bool,
        ifd_location: IfdPointer,
    ) -> TiffResult<Directory> {
        reader.goto_offset(ifd_location.0)?;

        let mut entries: BTreeMap<_, _> = BTreeMap::new();

        let num_tags = if bigtiff {
            reader.read_u64()?
        } else {
            reader.read_u16()?.into()
        };

        for _ in 0..num_tags {
            let (tag, entry) = match Self::read_entry(reader, bigtiff)? {
                Some(val) => val,
                None => {
                    continue;
                } // Unknown data type in tag, skip
            };

            entries.insert(tag.to_u16(), entry);
        }

        let next_ifd = if bigtiff {
            reader.read_u64()?
        } else {
            reader.read_u32()?.into()
        };

        let next_ifd = core::num::NonZeroU64::new(next_ifd);

        Ok(Directory { entries, next_ifd })
    }
}

impl IfdDecoder<'_> {
    pub fn find_entry(&self, tag: Tag) -> Option<ifd::Entry> {
        self.inner.ifd.get(tag).cloned()
    }
    pub fn find_tag(&mut self, tag: Tag) -> TiffResult<Option<ifd::Value>> {
        self.inner.find_tag(tag)
    }
    pub fn find_tag_buf(
        &mut self,
        tag: Tag,
        buf: &mut ValueBuffer,
    ) -> TiffResult<Option<ifd::Entry>> {
        self.inner.find_tag_buf(tag, buf)
    }
    pub fn find_tag_bytes(
        &mut self,
        tag: Tag,
        buf: &mut [u8],
        offset: u64,
    ) -> TiffResult<Option<usize>> {
        self.inner.find_tag_raw(tag, buf, offset)
    }
    pub fn find_tag_unsigned<T: TryFrom<u64>>(&mut self, tag: Tag) -> TiffResult<Option<T>> {
        self.find_tag(tag)?
            .map(|v| v.into_u64())
            .transpose()?
            .map(|value| {
                T::try_from(value).map_err(|_| TiffFormatError::InvalidTagValueType(tag).into())
            })
            .transpose()
    }
    pub fn find_tag_unsigned_vec<T: TryFrom<u64>>(
        &mut self,
        tag: Tag,
    ) -> TiffResult<Option<Vec<T>>> {
        self.find_tag(tag)?
            .map(|v| v.into_u64_vec())
            .transpose()?
            .map(|v| {
                v.into_iter()
                    .map(|u| {
                        T::try_from(u).map_err(|_| TiffFormatError::InvalidTagValueType(tag).into())
                    })
                    .collect()
            })
            .transpose()
    }
    pub fn get_tag_unsigned<T: TryFrom<u64>>(&mut self, tag: Tag) -> TiffResult<T> {
        self.find_tag_unsigned(tag)?
            .ok_or_else(|| TiffFormatError::RequiredTagNotFound(tag).into())
    }
    pub fn get_tag(&mut self, tag: Tag) -> TiffResult<ifd::Value> {
        match self.find_tag(tag)? {
            Some(val) => Ok(val),
            None => Err(TiffError::FormatError(
                TiffFormatError::RequiredTagNotFound(tag),
            )),
        }
    }
    pub fn get_tag_u32(&mut self, tag: Tag) -> TiffResult<u32> {
        self.get_tag(tag)?.into_u32()
    }

    pub fn get_tag_u64(&mut self, tag: Tag) -> TiffResult<u64> {
        self.get_tag(tag)?.into_u64()
    }
    pub fn get_tag_f32(&mut self, tag: Tag) -> TiffResult<f32> {
        self.get_tag(tag)?.into_f32()
    }
    pub fn get_tag_f64(&mut self, tag: Tag) -> TiffResult<f64> {
        self.get_tag(tag)?.into_f64()
    }
    pub fn get_tag_u32_vec(&mut self, tag: Tag) -> TiffResult<Vec<u32>> {
        self.get_tag(tag)?.into_u32_vec()
    }

    pub fn get_tag_u16_vec(&mut self, tag: Tag) -> TiffResult<Vec<u16>> {
        self.get_tag(tag)?.into_u16_vec()
    }

    pub fn get_tag_u64_vec(&mut self, tag: Tag) -> TiffResult<Vec<u64>> {
        self.get_tag(tag)?.into_u64_vec()
    }
    pub fn get_tag_f32_vec(&mut self, tag: Tag) -> TiffResult<Vec<f32>> {
        self.get_tag(tag)?.into_f32_vec()
    }
    pub fn get_tag_f64_vec(&mut self, tag: Tag) -> TiffResult<Vec<f64>> {
        self.get_tag(tag)?.into_f64_vec()
    }
    pub fn get_tag_u8_vec(&mut self, tag: Tag) -> TiffResult<Vec<u8>> {
        self.get_tag(tag)?.into_u8_vec()
    }
    pub fn get_tag_ascii_string(&mut self, tag: Tag) -> TiffResult<String> {
        self.get_tag(tag)?.into_string()
    }
    pub fn directory(&self) -> &Directory {
        self.inner.ifd
    }
}

impl<'l> IfdDecoder<'l> {
    pub fn tag_iter(self) -> impl Iterator<Item = TiffResult<(Tag, ifd::Value)>> + 'l {
        self.inner
            .ifd
            .iter()
            .map(|(tag, entry)| match self.inner.decoder.entry_val(entry) {
                Ok(value) => Ok((tag, value)),
                Err(err) => Err(err),
            })
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/image_codecs/tiff/decoder/mod.rs"]
mod tests;
