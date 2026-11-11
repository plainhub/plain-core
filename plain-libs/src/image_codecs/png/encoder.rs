use crate::image_codecs::png::DeflateCompression;
use crate::image_codecs::png::chunk::{self, ChunkType};
use crate::image_codecs::png::common::{
    AnimationControl, BitDepth, BlendOp, BytesPerPixel, ColorType, Compression, DisposeOp,
    FrameControl, Info, ParameterError, ParameterErrorKind, PixelDimensions, ScaledFloat, Unit,
};
use crate::image_codecs::png::filter::{Filter, filter};
use crate::image_codecs::png::text_metadata::{
    EncodableTextChunk, ITXtChunk, TEXtChunk, TextEncodingError, ZTXtChunk, encode_iso_8859_1,
};
use crate::image_codecs::png::traits::WriteBytesExt;
use borrow::Cow;
use crc32fast::Hasher as Crc32;
use flate2::write::ZlibEncoder;
use io::{Read, Write};
use ops::{Deref, DerefMut};
use std::{borrow, error, fmt, io, mem, ops, result};
pub type Result<T> = result::Result<T, EncodingError>;
#[derive(Debug)]
pub enum EncodingError {
    IoError(io::Error),
    Format(FormatError),
    Parameter(ParameterError),
    LimitsExceeded,
}
#[derive(Debug)]
pub struct FormatError {
    inner: FormatErrorKind,
}
#[derive(Debug)]
enum FormatErrorKind {
    ZeroWidth,
    ZeroHeight,
    InvalidColorCombination(BitDepth, ColorType),
    NoPalette,
    WrittenTooMuch(usize),
    NotAnimated,
    OutOfBounds,
    EndReached,
    ZeroFrames,
    MissingFrames,
    MissingData(usize),
    Unrecoverable,
    BadTextEncoding(TextEncodingError),
}
impl error::Error for EncodingError {
    fn cause(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            EncodingError::IoError(err) => Some(err),
            _ => None,
        }
    }
}
impl fmt::Display for EncodingError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        use self::EncodingError::*;
        match self {
            IoError(err) => write!(fmt, "{}", err),
            Format(desc) => write!(fmt, "{}", desc),
            Parameter(desc) => write!(fmt, "{}", desc),
            LimitsExceeded => write!(fmt, "Limits are exceeded."),
        }
    }
}
impl fmt::Display for FormatError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        use FormatErrorKind::*;
        match self.inner {
            ZeroWidth => write!(fmt, "Zero width not allowed"),
            ZeroHeight => write!(fmt, "Zero height not allowed"),
            ZeroFrames => write!(fmt, "Zero frames not allowed"),
            InvalidColorCombination(depth, color) => write!(
                fmt,
                "Invalid combination of bit-depth '{:?}' and color-type '{:?}'",
                depth, color
            ),
            NoPalette => write!(fmt, "can't write indexed image without palette"),
            WrittenTooMuch(index) => write!(fmt, "wrong data size, got {} bytes too many", index),
            NotAnimated => write!(fmt, "not an animation"),
            OutOfBounds => write!(
                fmt,
                "the dimension and position go over the frame boundaries"
            ),
            EndReached => write!(fmt, "all the frames have been already written"),
            MissingFrames => write!(fmt, "there are still frames to be written"),
            MissingData(n) => write!(fmt, "there are still {} bytes to be written", n),
            Unrecoverable => write!(
                fmt,
                "a previous error put the writer into an unrecoverable state"
            ),
            BadTextEncoding(tee) => match tee {
                TextEncodingError::Unrepresentable => write!(
                    fmt,
                    "The text metadata cannot be encoded into valid ISO 8859-1"
                ),
                TextEncodingError::InvalidKeywordSize => write!(fmt, "Invalid keyword size"),
                TextEncodingError::CompressionError => {
                    write!(fmt, "Unable to compress text metadata")
                }
            },
        }
    }
}
impl From<io::Error> for EncodingError {
    fn from(err: io::Error) -> EncodingError {
        EncodingError::IoError(err)
    }
}
impl From<EncodingError> for io::Error {
    fn from(err: EncodingError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, err.to_string())
    }
}
impl From<FormatErrorKind> for FormatError {
    fn from(kind: FormatErrorKind) -> Self {
        FormatError { inner: kind }
    }
}
impl From<TextEncodingError> for EncodingError {
    fn from(tee: TextEncodingError) -> Self {
        EncodingError::Format(FormatError {
            inner: FormatErrorKind::BadTextEncoding(tee),
        })
    }
}
pub struct Encoder<'a, W: Write> {
    w: W,
    info: Info<'a>,
    options: Options,
}
#[derive(Default)]
struct Options {
    filter: Filter,
    sep_def_img: bool,
    validate_sequence: bool,
    compression: DeflateCompression,
}
impl<'a, W: Write> Encoder<'a, W> {
    pub fn new(w: W, width: u32, height: u32) -> Encoder<'static, W> {
        Encoder {
            w,
            info: Info::with_size(width, height),
            options: Options::default(),
        }
    }
    pub fn with_info(w: W, info: Info<'a>) -> Result<Encoder<'a, W>> {
        if info.animation_control.is_some() != info.frame_control.is_some() {
            return Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()));
        }
        if let Some(actl) = info.animation_control {
            if actl.num_frames == 0 {
                return Err(EncodingError::Format(FormatErrorKind::ZeroFrames.into()));
            }
        }
        Ok(Encoder {
            w,
            info,
            options: Options::default(),
        })
    }

    pub fn set_source_srgb(&mut self, rendering_intent: super::SrgbRenderingIntent) {
        self.info.set_source_srgb(rendering_intent);
    }
    pub fn write_header(self) -> Result<Writer<W>> {
        Writer::new(self.w, PartialInfo::new(&self.info), self.options).init(&self.info)
    }
    pub fn set_color(&mut self, color: ColorType) {
        self.info.color_type = color;
    }
    pub fn set_depth(&mut self, depth: BitDepth) {
        self.info.bit_depth = depth;
    }
    pub fn set_compression(&mut self, compression: Compression) {
        self.set_deflate_compression(DeflateCompression::from_simple(compression));
        self.set_filter(Filter::from_simple(compression));
    }
    pub fn set_deflate_compression(&mut self, compression: DeflateCompression) {
        self.options.compression = compression;
    }
    pub fn set_filter(&mut self, filter: Filter) {
        self.options.filter = filter;
    }
    pub fn set_frame_delay(&mut self, numerator: u16, denominator: u16) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.delay_den = denominator;
            fctl.delay_num = numerator;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn set_blend_op(&mut self, op: BlendOp) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.blend_op = op;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn set_dispose_op(&mut self, op: DisposeOp) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.dispose_op = op;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    pub fn validate_sequence(&mut self, validate: bool) {
        self.options.validate_sequence = validate;
    }
}
pub struct Writer<W: Write> {
    w: W,
    info: PartialInfo,
    options: Options,
    images_written: u64,
    animation_written: u32,
    iend_written: bool,
}
struct PartialInfo {
    width: u32,
    height: u32,
    bit_depth: BitDepth,
    color_type: ColorType,
    frame_control: Option<FrameControl>,
    animation_control: Option<AnimationControl>,
    has_palette: bool,
}
impl PartialInfo {
    fn new(info: &Info) -> Self {
        PartialInfo {
            width: info.width,
            height: info.height,
            bit_depth: info.bit_depth,
            color_type: info.color_type,
            frame_control: info.frame_control,
            animation_control: info.animation_control,
            has_palette: info.palette.is_some(),
        }
    }
    fn bpp_in_prediction(&self) -> BytesPerPixel {
        BytesPerPixel::from_usize(self.bytes_per_pixel())
    }
    fn bytes_per_pixel(&self) -> usize {
        self.color_type.bytes_per_pixel(self.bit_depth)
    }
    fn raw_row_length(&self) -> usize {
        self.raw_row_length_from_width(self.width)
    }
    fn raw_row_length_from_width(&self, width: u32) -> usize {
        self.color_type
            .raw_row_length_from_width(self.bit_depth, width)
    }
}
const DEFAULT_BUFFER_LENGTH: usize = 4 * 1024;
pub(crate) fn write_chunk<W: Write>(mut w: W, name: chunk::ChunkType, data: &[u8]) -> Result<()> {
    w.write_be(data.len() as u32)?;
    w.write_all(&name.0)?;
    w.write_all(data)?;
    let mut crc = Crc32::new();
    crc.update(&name.0);
    crc.update(data);
    w.write_be(crc.finalize())?;
    Ok(())
}
impl<W: Write> Writer<W> {
    fn new(w: W, info: PartialInfo, options: Options) -> Writer<W> {
        Writer {
            w,
            info,
            options,
            images_written: 0,
            animation_written: 0,
            iend_written: false,
        }
    }
    fn init(mut self, info: &Info<'_>) -> Result<Self> {
        if self.info.width == 0 {
            return Err(EncodingError::Format(FormatErrorKind::ZeroWidth.into()));
        }
        if self.info.height == 0 {
            return Err(EncodingError::Format(FormatErrorKind::ZeroHeight.into()));
        }
        if self
            .info
            .color_type
            .is_combination_invalid(self.info.bit_depth)
        {
            return Err(EncodingError::Format(
                FormatErrorKind::InvalidColorCombination(self.info.bit_depth, self.info.color_type)
                    .into(),
            ));
        }
        self.encode_header(info)?;
        Ok(self)
    }
    fn encode_header(&mut self, info: &Info<'_>) -> Result<()> {
        self.w.write_all(&[137, 80, 78, 71, 13, 10, 26, 10])?;
        let mut data = [0; 13];
        data[..4].copy_from_slice(&info.width.to_be_bytes());
        data[4..8].copy_from_slice(&info.height.to_be_bytes());
        data[8] = info.bit_depth as u8;
        data[9] = info.color_type as u8;
        data[12] = info.interlaced as u8;
        self.write_chunk(chunk::IHDR, &data)?;
        if let Some(pd) = info.pixel_dims {
            let mut phys_data = [0; 9];
            phys_data[0..4].copy_from_slice(&pd.xppu.to_be_bytes());
            phys_data[4..8].copy_from_slice(&pd.yppu.to_be_bytes());
            match pd.unit {
                Unit::Meter => phys_data[8] = 1,
                Unit::Unspecified => phys_data[8] = 0,
            }
            self.write_chunk(chunk::pHYs, &phys_data)?;
        }
        if let Some(srgb) = &info.srgb {
            srgb.encode(&mut self.w)?;
            let srgb_gamma = crate::image_codecs::png::srgb::substitute_gamma();
            if Some(srgb_gamma) == info.source_gamma {
                srgb_gamma.encode_gama(&mut self.w)?
            }
            let srgb_chromaticities = crate::image_codecs::png::srgb::substitute_chromaticities();
            if Some(srgb_chromaticities) == info.source_chromaticities {
                srgb_chromaticities.encode(&mut self.w)?;
            }
        } else {
            if let Some(gma) = info.source_gamma {
                gma.encode_gama(&mut self.w)?
            }
            if let Some(chrms) = info.source_chromaticities {
                chrms.encode(&mut self.w)?;
            }
            if let Some(iccp) = &info.icc_profile {
                self.write_iccp_chunk("_", iccp)?
            }
        }
        if let Some(exif) = &info.exif_metadata {
            self.write_chunk(chunk::eXIf, exif)?;
        }
        if let Some(actl) = info.animation_control {
            actl.encode(&mut self.w)?;
        }
        if let Some(p) = &info.palette {
            self.write_chunk(chunk::PLTE, p)?;
        };
        if let Some(t) = &info.trns {
            self.write_chunk(chunk::tRNS, t)?;
        }
        for text_chunk in &info.uncompressed_latin1_text {
            self.write_text_chunk(text_chunk)?;
        }
        for text_chunk in &info.compressed_latin1_text {
            self.write_text_chunk(text_chunk)?;
        }
        for text_chunk in &info.utf8_text {
            self.write_text_chunk(text_chunk)?;
        }
        Ok(())
    }
    pub fn write_chunk(&mut self, name: ChunkType, data: &[u8]) -> Result<()> {
        use std::convert::TryFrom;
        if u32::try_from(data.len()).map_or(true, |length| length > i32::MAX as u32) {
            let kind = FormatErrorKind::WrittenTooMuch(data.len() - i32::MAX as usize);
            return Err(EncodingError::Format(kind.into()));
        }
        write_chunk(&mut self.w, name, data)
    }
    pub fn write_text_chunk<T: EncodableTextChunk>(&mut self, text_chunk: &T) -> Result<()> {
        text_chunk.encode(&mut self.w)
    }
    fn write_iccp_chunk(&mut self, profile_name: &str, icc_profile: &[u8]) -> Result<()> {
        let profile_name = encode_iso_8859_1(profile_name)?;
        if profile_name.is_empty() || profile_name.len() > 79 {
            return Err(TextEncodingError::InvalidKeywordSize.into());
        }
        let estimated_compressed_size = icc_profile.len() * 3 / 4;
        let chunk_size = profile_name
            .len()
            .checked_add(2)
            .and_then(|s| s.checked_add(estimated_compressed_size))
            .ok_or(EncodingError::LimitsExceeded)?;
        let mut data = Vec::new();
        data.try_reserve_exact(chunk_size)
            .map_err(|_| EncodingError::LimitsExceeded)?;
        data.extend(profile_name.into_iter().chain([0, 0]));
        let mut encoder = ZlibEncoder::new(data, flate2::Compression::default());
        encoder.write_all(icc_profile)?;
        self.write_chunk(chunk::iCCP, &encoder.finish()?)
    }
    fn validate_new_image(&self) -> Result<()> {
        if !self.options.validate_sequence {
            return Ok(());
        }
        match self.info.animation_control {
            None => {
                if self.images_written == 0 {
                    Ok(())
                } else {
                    Err(EncodingError::Format(FormatErrorKind::EndReached.into()))
                }
            }
            Some(_) => {
                if self.info.frame_control.is_some() {
                    Ok(())
                } else {
                    Err(EncodingError::Format(FormatErrorKind::EndReached.into()))
                }
            }
        }
    }
    fn validate_sequence_done(&self) -> Result<()> {
        if !self.options.validate_sequence {
            return Ok(());
        }
        if (self.info.animation_control.is_some() && self.info.frame_control.is_some())
            || self.images_written == 0
        {
            Err(EncodingError::Format(FormatErrorKind::MissingFrames.into()))
        } else {
            Ok(())
        }
    }
    const MAX_IDAT_CHUNK_LEN: u32 = u32::MAX >> 1;
    const MAX_fdAT_CHUNK_LEN: u32 = (u32::MAX >> 1) - 4;
    pub fn write_image_data(&mut self, data: &[u8]) -> Result<()> {
        if self.info.color_type == ColorType::Indexed && !self.info.has_palette {
            return Err(EncodingError::Format(FormatErrorKind::NoPalette.into()));
        }
        self.validate_new_image()?;
        let width: usize;
        let height: usize;
        if let Some(ref mut fctl) = self.info.frame_control {
            width = fctl.width as usize;
            height = fctl.height as usize;
        } else {
            width = self.info.width as usize;
            height = self.info.height as usize;
        }
        let in_len = self.info.raw_row_length_from_width(width as u32) - 1;
        let data_size = in_len * height;
        if data_size != data.len() {
            return Err(EncodingError::Parameter(
                ParameterErrorKind::ImageBufferSize {
                    expected: data_size,
                    actual: data.len(),
                }
                .into(),
            ));
        }
        let prev = vec![0; in_len];
        let mut prev = prev.as_slice();
        let bpp = self.info.bpp_in_prediction();
        let filter_method = self.options.filter;
        let zlib_encoded = match self.options.compression {
            DeflateCompression::NoCompression => {
                let mut compressor = crate::image_codecs::deflate::StoredOnlyCompressor::new(
                    std::io::Cursor::new(Vec::new()),
                )?;
                for line in data.chunks(in_len) {
                    compressor.write_data(&[0])?;
                    compressor.write_data(line)?;
                }
                compressor.finish()?.into_inner()
            }
            DeflateCompression::FdeflateUltraFast => {
                let mut compressor = crate::image_codecs::deflate::Compressor::new(
                    std::io::Cursor::new(Vec::new()),
                )?;
                let mut current = vec![0; in_len + 1];
                for line in data.chunks(in_len) {
                    let filter_type = filter(filter_method, bpp, prev, line, &mut current[1..]);
                    current[0] = filter_type as u8;
                    compressor.write_data(&current)?;
                    prev = line;
                }
                let compressed = compressor.finish()?.into_inner();
                if compressed.len()
                    > crate::image_codecs::deflate::StoredOnlyCompressor::<()>::compressed_size(
                        (in_len + 1) * height,
                    )
                {
                    let mut compressor = crate::image_codecs::deflate::StoredOnlyCompressor::new(
                        std::io::Cursor::new(Vec::new()),
                    )?;
                    for line in data.chunks(in_len) {
                        compressor.write_data(&[0])?;
                        compressor.write_data(line)?;
                    }
                    compressor.finish()?.into_inner()
                } else {
                    compressed
                }
            }
            DeflateCompression::Level(level) => {
                let mut current = vec![0; in_len];
                let mut zlib =
                    ZlibEncoder::new(Vec::new(), flate2::Compression::new(u32::from(level)));
                for line in data.chunks(in_len) {
                    let filter_type = filter(filter_method, bpp, prev, line, &mut current);
                    zlib.write_all(&[filter_type as u8])?;
                    zlib.write_all(&current)?;
                    prev = line;
                }
                zlib.finish()?
            }
        };
        match self.info.frame_control {
            None => {
                self.write_zlib_encoded_idat(&zlib_encoded)?;
            }
            Some(_) if self.should_skip_frame_control_on_default_image() => {
                self.write_zlib_encoded_idat(&zlib_encoded)?;
            }
            Some(ref mut fctl) => {
                fctl.encode(&mut self.w)?;
                fctl.sequence_number = fctl.sequence_number.wrapping_add(1);
                self.animation_written += 1;
                if self.images_written == 0 {
                    self.write_zlib_encoded_idat(&zlib_encoded)?;
                } else {
                    let buff_size = zlib_encoded.len().min(Self::MAX_fdAT_CHUNK_LEN as usize);
                    let mut alldata = vec![0u8; 4 + buff_size];
                    for chunk in zlib_encoded.chunks(Self::MAX_fdAT_CHUNK_LEN as usize) {
                        alldata[..4].copy_from_slice(&fctl.sequence_number.to_be_bytes());
                        alldata[4..][..chunk.len()].copy_from_slice(chunk);
                        write_chunk(&mut self.w, chunk::fdAT, &alldata[..4 + chunk.len()])?;
                        fctl.sequence_number = fctl.sequence_number.wrapping_add(1);
                    }
                }
            }
        }
        self.increment_images_written();
        Ok(())
    }
    fn increment_images_written(&mut self) {
        self.images_written = self.images_written.saturating_add(1);
        if let Some(actl) = self.info.animation_control {
            if actl.num_frames <= self.animation_written {
                self.info.frame_control = None;
            }
        }
    }
    fn write_iend(&mut self) -> Result<()> {
        self.iend_written = true;
        self.write_chunk(chunk::IEND, &[])
    }
    fn should_skip_frame_control_on_default_image(&self) -> bool {
        self.options.sep_def_img && self.images_written == 0
    }
    fn write_zlib_encoded_idat(&mut self, zlib_encoded: &[u8]) -> Result<()> {
        for chunk in zlib_encoded.chunks(Self::MAX_IDAT_CHUNK_LEN as usize) {
            self.write_chunk(chunk::IDAT, chunk)?;
        }
        Ok(())
    }
    pub fn set_filter(&mut self, filter: Filter) {
        self.options.filter = filter;
    }
    pub fn set_frame_delay(&mut self, numerator: u16, denominator: u16) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.delay_den = denominator;
            fctl.delay_num = numerator;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn set_frame_dimension(&mut self, width: u32, height: u32) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            if Some(width) > self.info.width.checked_sub(fctl.x_offset)
                || Some(height) > self.info.height.checked_sub(fctl.y_offset)
            {
                return Err(EncodingError::Format(FormatErrorKind::OutOfBounds.into()));
            } else if width == 0 {
                return Err(EncodingError::Format(FormatErrorKind::ZeroWidth.into()));
            } else if height == 0 {
                return Err(EncodingError::Format(FormatErrorKind::ZeroHeight.into()));
            }
            fctl.width = width;
            fctl.height = height;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn set_frame_position(&mut self, x: u32, y: u32) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            if Some(x) > self.info.width.checked_sub(fctl.width)
                || Some(y) > self.info.height.checked_sub(fctl.height)
            {
                return Err(EncodingError::Format(FormatErrorKind::OutOfBounds.into()));
            }
            fctl.x_offset = x;
            fctl.y_offset = y;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn reset_frame_dimension(&mut self) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.width = self.info.width - fctl.x_offset;
            fctl.height = self.info.height - fctl.y_offset;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn reset_frame_position(&mut self) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.x_offset = 0;
            fctl.y_offset = 0;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn set_blend_op(&mut self, op: BlendOp) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.blend_op = op;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn set_dispose_op(&mut self, op: DisposeOp) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.dispose_op = op;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    pub fn stream_writer_with_size(&mut self, size: usize) -> Result<StreamWriter<'_, W>> {
        StreamWriter::new(ChunkOutput::Borrowed(self), size)
    }

    pub fn into_stream_writer_with_size(self, size: usize) -> Result<StreamWriter<'static, W>> {
        StreamWriter::new(ChunkOutput::Owned(self), size)
    }
    pub fn finish(mut self) -> Result<()> {
        self.validate_sequence_done()?;
        self.write_iend()?;
        self.w.flush()?;
        drop(self);
        Ok(())
    }
}
impl<W: Write> Drop for Writer<W> {
    fn drop(&mut self) {
        if !self.iend_written {
            let _ = self.write_iend();
        }
    }
}
enum ChunkOutput<'a, W: Write> {
    Borrowed(&'a mut Writer<W>),
    Owned(Writer<W>),
}
impl<'a, W: Write> Deref for ChunkOutput<'a, W> {
    type Target = Writer<W>;
    fn deref(&self) -> &Self::Target {
        match self {
            ChunkOutput::Borrowed(writer) => writer,
            ChunkOutput::Owned(writer) => writer,
        }
    }
}
impl<'a, W: Write> DerefMut for ChunkOutput<'a, W> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            ChunkOutput::Borrowed(writer) => writer,
            ChunkOutput::Owned(writer) => writer,
        }
    }
}
struct ChunkWriter<'a, W: Write> {
    writer: ChunkOutput<'a, W>,
    buffer: Vec<u8>,
    index: usize,
    curr_chunk: ChunkType,
}
impl<'a, W: Write> ChunkWriter<'a, W> {
    fn new(writer: ChunkOutput<'a, W>, buf_len: usize) -> ChunkWriter<'a, W> {
        const CAP: usize = u32::MAX as usize >> 1;
        let curr_chunk = if writer.images_written == 0 {
            chunk::IDAT
        } else {
            chunk::fdAT
        };
        ChunkWriter {
            writer,
            buffer: vec![0; CAP.min(buf_len)],
            index: 0,
            curr_chunk,
        }
    }
    fn next_frame_info(&self) -> (usize, usize) {
        let wrt = self.writer.deref();
        let width: usize;
        let height: usize;
        if let Some(fctl) = wrt.info.frame_control {
            width = fctl.width as usize;
            height = fctl.height as usize;
        } else {
            width = wrt.info.width as usize;
            height = wrt.info.height as usize;
        }
        let in_len = wrt.info.raw_row_length_from_width(width as u32) - 1;
        let data_size = in_len * height;
        (in_len, data_size)
    }
    fn write_header(&mut self) -> Result<()> {
        assert_eq!(self.index, 0, "Called when not flushed");
        let wrt = self.writer.deref_mut();
        self.curr_chunk = if wrt.images_written == 0 {
            chunk::IDAT
        } else {
            chunk::fdAT
        };
        match wrt.info.frame_control {
            Some(_) if wrt.should_skip_frame_control_on_default_image() => {}
            Some(ref mut fctl) => {
                fctl.encode(&mut wrt.w)?;
                fctl.sequence_number += 1;
            }
            _ => {}
        }
        Ok(())
    }
    fn set_fctl(&mut self, f: FrameControl) {
        if let Some(ref mut fctl) = self.writer.info.frame_control {
            *fctl = FrameControl {
                sequence_number: fctl.sequence_number,
                ..f
            };
        } else {
            panic!("This function must be called on an animated PNG")
        }
    }
    fn flush_inner(&mut self) -> io::Result<()> {
        if self.index > 0 {
            write_chunk(
                &mut self.writer.w,
                self.curr_chunk,
                &self.buffer[..self.index],
            )?;
            self.index = 0;
        }
        Ok(())
    }
}
impl<'a, W: Write> Write for ChunkWriter<'a, W> {
    fn write(&mut self, mut data: &[u8]) -> io::Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }
        if self.index == 0 {
            let wrt = self.writer.deref_mut();
            let no_fctl = wrt.should_skip_frame_control_on_default_image();
            if wrt.info.frame_control.is_some() && !no_fctl {
                let fctl = wrt.info.frame_control.as_mut().unwrap();
                self.buffer[0..4].copy_from_slice(&fctl.sequence_number.to_be_bytes());
                fctl.sequence_number += 1;
                self.index = 4;
            }
        }
        let written = data.len().min(self.buffer.len() - self.index);
        data = &data[..written];
        self.buffer[self.index..][..written].copy_from_slice(data);
        self.index += written;
        if self.index == self.buffer.len() {
            self.flush_inner()?;
        }
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flush_inner()
    }
}
impl<W: Write> Drop for ChunkWriter<'_, W> {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}
enum Wrapper<'a, W: Write> {
    Chunk(ChunkWriter<'a, W>),
    Flate2(ZlibEncoder<ChunkWriter<'a, W>>),
    FDeflate(crate::image_codecs::deflate::Compressor<ChunkWriter<'a, W>>),
    Unrecoverable,
    None,
}
impl<'a, W: Write> Wrapper<'a, W> {
    fn from_level(writer: ChunkWriter<'a, W>, compression: DeflateCompression) -> io::Result<Self> {
        Ok(match compression {
            DeflateCompression::NoCompression => {
                Wrapper::Flate2(ZlibEncoder::new(writer, flate2::Compression::none()))
            }
            DeflateCompression::FdeflateUltraFast => {
                Wrapper::FDeflate(crate::image_codecs::deflate::Compressor::new(writer)?)
            }
            DeflateCompression::Level(level) => Wrapper::Flate2(ZlibEncoder::new(
                writer,
                flate2::Compression::new(u32::from(level)),
            )),
        })
    }
    fn take(&mut self) -> Wrapper<'a, W> {
        let mut swap = Wrapper::None;
        mem::swap(self, &mut swap);
        swap
    }
}
pub struct StreamWriter<'a, W: Write> {
    writer: Wrapper<'a, W>,
    prev_buf: Vec<u8>,
    curr_buf: Vec<u8>,
    filtered_buf: Vec<u8>,
    index: usize,
    line_len: usize,
    to_write: usize,
    width: u32,
    height: u32,
    bpp: BytesPerPixel,
    filter: Filter,
    fctl: Option<FrameControl>,
    compression: DeflateCompression,
}
impl<'a, W: Write> StreamWriter<'a, W> {
    fn new(writer: ChunkOutput<'a, W>, buf_len: usize) -> Result<StreamWriter<'a, W>> {
        let PartialInfo {
            width,
            height,
            frame_control: fctl,
            ..
        } = writer.info;
        let bpp = writer.info.bpp_in_prediction();
        let in_len = writer.info.raw_row_length() - 1;
        let filter = writer.options.filter;
        let compression = writer.options.compression;
        let prev_buf = vec![0; in_len];
        let curr_buf = vec![0; in_len];
        let filtered_buf = vec![0; in_len];
        let mut chunk_writer = ChunkWriter::new(writer, buf_len);
        let (line_len, to_write) = chunk_writer.next_frame_info();
        chunk_writer.write_header()?;
        Ok(StreamWriter {
            writer: Wrapper::from_level(chunk_writer, compression)?,
            index: 0,
            prev_buf,
            curr_buf,
            filtered_buf,
            bpp,
            filter,
            width,
            height,
            line_len,
            to_write,
            fctl,
            compression,
        })
    }
    pub fn set_filter(&mut self, filter: Filter) {
        self.filter = filter;
    }
    pub fn set_frame_delay(&mut self, numerator: u16, denominator: u16) -> Result<()> {
        if let Some(ref mut fctl) = self.fctl {
            fctl.delay_den = denominator;
            fctl.delay_num = numerator;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn set_frame_dimension(&mut self, width: u32, height: u32) -> Result<()> {
        if let Some(ref mut fctl) = self.fctl {
            if Some(width) > self.width.checked_sub(fctl.x_offset)
                || Some(height) > self.height.checked_sub(fctl.y_offset)
            {
                return Err(EncodingError::Format(FormatErrorKind::OutOfBounds.into()));
            } else if width == 0 {
                return Err(EncodingError::Format(FormatErrorKind::ZeroWidth.into()));
            } else if height == 0 {
                return Err(EncodingError::Format(FormatErrorKind::ZeroHeight.into()));
            }
            fctl.width = width;
            fctl.height = height;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn set_frame_position(&mut self, x: u32, y: u32) -> Result<()> {
        if let Some(ref mut fctl) = self.fctl {
            if Some(x) > self.width.checked_sub(fctl.width)
                || Some(y) > self.height.checked_sub(fctl.height)
            {
                return Err(EncodingError::Format(FormatErrorKind::OutOfBounds.into()));
            }
            fctl.x_offset = x;
            fctl.y_offset = y;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn reset_frame_dimension(&mut self) -> Result<()> {
        if let Some(ref mut fctl) = self.fctl {
            fctl.width = self.width - fctl.x_offset;
            fctl.height = self.height - fctl.y_offset;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn reset_frame_position(&mut self) -> Result<()> {
        if let Some(ref mut fctl) = self.fctl {
            fctl.x_offset = 0;
            fctl.y_offset = 0;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn set_blend_op(&mut self, op: BlendOp) -> Result<()> {
        if let Some(ref mut fctl) = self.fctl {
            fctl.blend_op = op;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn set_dispose_op(&mut self, op: DisposeOp) -> Result<()> {
        if let Some(ref mut fctl) = self.fctl {
            fctl.dispose_op = op;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn finish(mut self) -> Result<()> {
        self.finish_mut()
    }
    fn finish_mut(&mut self) -> Result<()> {
        if self.to_write > 0 {
            let err = FormatErrorKind::MissingData(self.to_write).into();
            return Err(EncodingError::Format(err));
        }
        self.flush()?;
        match self.writer.take() {
            Wrapper::Chunk(wrt) => {
                wrt.writer.validate_sequence_done()?;
            }
            Wrapper::FDeflate(wrt) => {
                wrt.finish()?;
            }
            Wrapper::Flate2(wrt) => {
                wrt.finish()?;
            }
            Wrapper::None => unreachable!(),
            Wrapper::Unrecoverable => {
                let err = FormatErrorKind::Unrecoverable.into();
                return Err(EncodingError::Format(err));
            }
        }
        Ok(())
    }
    fn new_frame(&mut self) -> Result<()> {
        let wrt = match &mut self.writer {
            Wrapper::Chunk(wrt) => wrt,
            Wrapper::Unrecoverable => {
                let err = FormatErrorKind::Unrecoverable.into();
                return Err(EncodingError::Format(err));
            }
            Wrapper::Flate2(_) | Wrapper::FDeflate(_) => {
                unreachable!("never called on a half-finished frame")
            }
            Wrapper::None => unreachable!(),
        };
        wrt.flush()?;
        wrt.writer.validate_new_image()?;
        if let Some(fctl) = self.fctl {
            wrt.set_fctl(fctl);
        }
        let (scansize, size) = wrt.next_frame_info();
        self.line_len = scansize;
        self.to_write = size;
        wrt.write_header()?;
        wrt.writer.increment_images_written();
        match self.writer.take() {
            Wrapper::Chunk(wrt) => match Wrapper::from_level(wrt, self.compression) {
                Ok(writer) => self.writer = writer,
                Err(err) => {
                    self.writer = Wrapper::Unrecoverable;
                    return Err(err.into());
                }
            },
            _ => unreachable!(),
        };
        Ok(())
    }
}
impl<'a, W: Write> Write for StreamWriter<'a, W> {
    fn write(&mut self, mut data: &[u8]) -> io::Result<usize> {
        if let Wrapper::Unrecoverable = self.writer {
            let err = FormatErrorKind::Unrecoverable.into();
            return Err(EncodingError::Format(err).into());
        }
        if data.is_empty() {
            return Ok(0);
        }
        if self.to_write == 0 {
            match self.writer.take() {
                Wrapper::Flate2(wrt) => match wrt.finish() {
                    Ok(chunk) => self.writer = Wrapper::Chunk(chunk),
                    Err(err) => {
                        self.writer = Wrapper::Unrecoverable;
                        return Err(err);
                    }
                },
                Wrapper::FDeflate(wrt) => match wrt.finish() {
                    Ok(chunk) => self.writer = Wrapper::Chunk(chunk),
                    Err(err) => {
                        self.writer = Wrapper::Unrecoverable;
                        return Err(err);
                    }
                },
                chunk @ Wrapper::Chunk(_) => self.writer = chunk,
                Wrapper::Unrecoverable => unreachable!(),
                Wrapper::None => unreachable!(),
            };
            self.new_frame()?;
        }
        let written = data.read(&mut self.curr_buf[..self.line_len][self.index..])?;
        self.index += written;
        self.to_write -= written;
        if self.index == self.line_len {
            let filter_type = filter(
                self.filter,
                self.bpp,
                &self.prev_buf,
                &self.curr_buf,
                &mut self.filtered_buf,
            );
            match &mut self.writer {
                Wrapper::Flate2(wrt) => {
                    wrt.write_all(&[filter_type as u8])?;
                    wrt.write_all(&self.filtered_buf)?;
                }
                Wrapper::FDeflate(wrt) => {
                    wrt.write_data(&[filter_type as u8])?;
                    wrt.write_data(&self.filtered_buf)?;
                }
                _ => unreachable!(),
            };
            mem::swap(&mut self.prev_buf, &mut self.curr_buf);
            self.index = 0;
        }
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        match &mut self.writer {
            Wrapper::Flate2(wrt) => wrt.flush()?,
            Wrapper::Chunk(wrt) => wrt.flush()?,
            Wrapper::FDeflate(_) => (),
            Wrapper::Unrecoverable | Wrapper::None => {
                let err = FormatErrorKind::Unrecoverable.into();
                return Err(EncodingError::Format(err).into());
            }
        }
        if self.index > 0 {
            let err = FormatErrorKind::WrittenTooMuch(self.index).into();
            return Err(EncodingError::Format(err).into());
        }
        Ok(())
    }
}
impl<W: Write> Drop for StreamWriter<'_, W> {
    fn drop(&mut self) {
        let _ = self.finish_mut();
    }
}
