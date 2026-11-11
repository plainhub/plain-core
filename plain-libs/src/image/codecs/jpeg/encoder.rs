use super::entropy::build_huff_lut_const;
use super::transform;
use crate::image::error::{
    EncodingError, ImageError, ImageResult, UnsupportedError, UnsupportedErrorKind,
};
use crate::image::traits::PixelWithColorType;
use crate::image::utils::clamp;
use crate::image::{
    ColorType, DynamicImage, ExtendedColorType, GenericImageView, ImageBuffer, ImageEncoder,
    ImageFormat, Luma, Pixel, Rgb,
};
use num_traits::ToPrimitive;
use std::borrow::Cow;
use std::io::{self, Write};
use std::{error, fmt};
static SOF0: u8 = 0xC0;
static DHT: u8 = 0xC4;
static SOI: u8 = 0xD8;
static EOI: u8 = 0xD9;
static SOS: u8 = 0xDA;
static DQT: u8 = 0xDB;
static APP0: u8 = 0xE0;
static APP1: u8 = 0xE1;
static APP2: u8 = 0xE2;
# [rustfmt :: skip] static STD_LUMA_QTABLE : [u8 ; 64] = [16 , 11 , 10 , 16 , 24 , 40 , 51 , 61 , 12 , 12 , 14 , 19 , 26 , 58 , 60 , 55 , 14 , 13 , 16 , 24 , 40 , 57 , 69 , 56 , 14 , 17 , 22 , 29 , 51 , 87 , 80 , 62 , 18 , 22 , 37 , 56 , 68 , 109 , 103 , 77 , 24 , 35 , 55 , 64 , 81 , 104 , 113 , 92 , 49 , 64 , 78 , 87 , 103 , 121 , 120 , 101 , 72 , 92 , 95 , 98 , 112 , 100 , 103 , 99 ,] ;
# [rustfmt :: skip] static STD_CHROMA_QTABLE : [u8 ; 64] = [17 , 18 , 24 , 47 , 99 , 99 , 99 , 99 , 18 , 21 , 26 , 66 , 99 , 99 , 99 , 99 , 24 , 26 , 56 , 99 , 99 , 99 , 99 , 99 , 47 , 66 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 , 99 ,] ;
static STD_LUMA_DC_CODE_LENGTHS: [u8; 16] = [
    0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
static STD_LUMA_DC_VALUES: [u8; 12] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
];
static STD_LUMA_DC_HUFF_LUT: [(u8, u16); 256] =
    build_huff_lut_const(&STD_LUMA_DC_CODE_LENGTHS, &STD_LUMA_DC_VALUES);
static STD_CHROMA_DC_CODE_LENGTHS: [u8; 16] = [
    0x00, 0x03, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
];
static STD_CHROMA_DC_VALUES: [u8; 12] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
];
static STD_CHROMA_DC_HUFF_LUT: [(u8, u16); 256] =
    build_huff_lut_const(&STD_CHROMA_DC_CODE_LENGTHS, &STD_CHROMA_DC_VALUES);
static STD_LUMA_AC_CODE_LENGTHS: [u8; 16] = [
    0x00, 0x02, 0x01, 0x03, 0x03, 0x02, 0x04, 0x03, 0x05, 0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7D,
];
static STD_LUMA_AC_VALUES: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08, 0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
    0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5,
    0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
    0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
    0xF9, 0xFA,
];
static STD_LUMA_AC_HUFF_LUT: [(u8, u16); 256] =
    build_huff_lut_const(&STD_LUMA_AC_CODE_LENGTHS, &STD_LUMA_AC_VALUES);
static STD_CHROMA_AC_CODE_LENGTHS: [u8; 16] = [
    0x00, 0x02, 0x01, 0x02, 0x04, 0x04, 0x03, 0x04, 0x07, 0x05, 0x04, 0x04, 0x00, 0x01, 0x02, 0x77,
];
static STD_CHROMA_AC_VALUES: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xA1, 0xB1, 0xC1, 0x09, 0x23, 0x33, 0x52, 0xF0,
    0x15, 0x62, 0x72, 0xD1, 0x0A, 0x16, 0x24, 0x34, 0xE1, 0x25, 0xF1, 0x17, 0x18, 0x19, 0x1A, 0x26,
    0x27, 0x28, 0x29, 0x2A, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5,
    0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3,
    0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA,
    0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
    0xF9, 0xFA,
];
static STD_CHROMA_AC_HUFF_LUT: [(u8, u16); 256] =
    build_huff_lut_const(&STD_CHROMA_AC_CODE_LENGTHS, &STD_CHROMA_AC_VALUES);
static DCCLASS: u8 = 0;
static ACCLASS: u8 = 1;
static LUMADESTINATION: u8 = 0;
static CHROMADESTINATION: u8 = 1;
static LUMAID: u8 = 1;
static CHROMABLUEID: u8 = 2;
static CHROMAREDID: u8 = 3;
# [rustfmt :: skip] static UNZIGZAG : [u8 ; 64] = [0 , 1 , 8 , 16 , 9 , 2 , 3 , 10 , 17 , 24 , 32 , 25 , 18 , 11 , 4 , 5 , 12 , 19 , 26 , 33 , 40 , 48 , 41 , 34 , 27 , 20 , 13 , 6 , 7 , 14 , 21 , 28 , 35 , 42 , 49 , 56 , 57 , 50 , 43 , 36 , 29 , 22 , 15 , 23 , 30 , 37 , 44 , 51 , 58 , 59 , 52 , 45 , 38 , 31 , 39 , 46 , 53 , 60 , 61 , 54 , 47 , 55 , 62 , 63 ,] ;
static EXIF_HEADER: [u8; 6] = [0x45, 0x78, 0x69, 0x66, 0x00, 0x00];
#[derive(Copy, Clone)]
struct Component {
    id: u8,
    h: u8,
    v: u8,
    tq: u8,
    dc_table: u8,
    ac_table: u8,
    _dc_pred: i32,
}
pub(crate) struct BitWriter<W> {
    w: W,
    accumulator: u32,
    nbits: u8,
}
impl<W: Write> BitWriter<W> {
    fn new(w: W) -> Self {
        BitWriter {
            w,
            accumulator: 0,
            nbits: 0,
        }
    }
    fn write_bits(&mut self, bits: u16, size: u8) -> io::Result<()> {
        if size == 0 {
            return Ok(());
        }
        self.nbits += size;
        self.accumulator |= u32::from(bits) << (32 - self.nbits) as usize;
        while self.nbits >= 8 {
            let byte = self.accumulator >> 24;
            self.w.write_all(&[byte as u8])?;
            if byte == 0xFF {
                self.w.write_all(&[0x00])?;
            }
            self.nbits -= 8;
            self.accumulator <<= 8;
        }
        Ok(())
    }
    fn pad_byte(&mut self) -> io::Result<()> {
        self.write_bits(0x7F, 7)
    }
    fn huffman_encode(&mut self, val: u8, table: &[(u8, u16); 256]) -> io::Result<()> {
        let (size, code) = table[val as usize];
        assert!(size <= 16, "bad huffman value");
        self.write_bits(code, size)
    }
    fn write_block(
        &mut self,
        block: &[i32; 64],
        prevdc: i32,
        dctable: &[(u8, u16); 256],
        actable: &[(u8, u16); 256],
    ) -> io::Result<i32> {
        let dcval = block[0];
        let diff = dcval - prevdc;
        let (size, value) = encode_coefficient(diff);
        self.huffman_encode(size, dctable)?;
        self.write_bits(value, size)?;
        let mut zero_run = 0;
        for &k in &UNZIGZAG[1..] {
            if block[k as usize] == 0 {
                zero_run += 1;
            } else {
                while zero_run > 15 {
                    self.huffman_encode(0xF0, actable)?;
                    zero_run -= 16;
                }
                let (size, value) = encode_coefficient(block[k as usize]);
                let symbol = (zero_run << 4) | size;
                self.huffman_encode(symbol, actable)?;
                self.write_bits(value, size)?;
                zero_run = 0;
            }
        }
        if block[UNZIGZAG[63] as usize] == 0 {
            self.huffman_encode(0x00, actable)?;
        }
        Ok(dcval)
    }
    fn write_marker(&mut self, marker: u8) -> io::Result<()> {
        self.w.write_all(&[0xFF, marker])
    }
    fn write_segment(&mut self, marker: u8, data: &[u8]) -> io::Result<()> {
        self.w.write_all(&[0xFF, marker])?;
        self.w.write_all(&(data.len() as u16 + 2).to_be_bytes())?;
        self.w.write_all(data)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PixelDensityUnit {
    PixelAspectRatio,
    Inches,
    Centimeters,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PixelDensity {
    pub density: (u16, u16),
    pub unit: PixelDensityUnit,
}
impl PixelDensity {
    #[must_use]
    pub fn dpi(density: u16) -> Self {
        PixelDensity {
            density: (density, density),
            unit: PixelDensityUnit::Inches,
        }
    }
}
impl Default for PixelDensity {
    fn default() -> Self {
        PixelDensity {
            density: (1, 1),
            unit: PixelDensityUnit::PixelAspectRatio,
        }
    }
}
#[derive(Debug, Copy, Clone)]
enum EncoderError {
    InvalidSize(u32, u32),
}
impl fmt::Display for EncoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncoderError::InvalidSize(w, h) => f.write_fmt(format_args!(
                "Invalid image size ({w} x {h}) to encode as JPEG: \
                 width and height must be >= 1 and <= 65535"
            )),
        }
    }
}
impl From<EncoderError> for ImageError {
    fn from(e: EncoderError) -> ImageError {
        ImageError::Encoding(EncodingError::new(ImageFormat::Jpeg.into(), e))
    }
}
impl error::Error for EncoderError {}
pub struct JpegEncoder<W> {
    writer: BitWriter<W>,
    components: Vec<Component>,
    tables: Vec<[u8; 64]>,
    luma_dctable: Cow<'static, [(u8, u16); 256]>,
    luma_actable: Cow<'static, [(u8, u16); 256]>,
    chroma_dctable: Cow<'static, [(u8, u16); 256]>,
    chroma_actable: Cow<'static, [(u8, u16); 256]>,
    pixel_density: PixelDensity,
    icc_profile: Vec<u8>,
    exif: Vec<u8>,
}
impl<W: Write> JpegEncoder<W> {
    pub fn new(w: W) -> JpegEncoder<W> {
        JpegEncoder::new_with_quality(w, 75)
    }
    pub fn new_with_quality(w: W, quality: u8) -> JpegEncoder<W> {
        let components = vec![
            Component {
                id: LUMAID,
                h: 1,
                v: 1,
                tq: LUMADESTINATION,
                dc_table: LUMADESTINATION,
                ac_table: LUMADESTINATION,
                _dc_pred: 0,
            },
            Component {
                id: CHROMABLUEID,
                h: 1,
                v: 1,
                tq: CHROMADESTINATION,
                dc_table: CHROMADESTINATION,
                ac_table: CHROMADESTINATION,
                _dc_pred: 0,
            },
            Component {
                id: CHROMAREDID,
                h: 1,
                v: 1,
                tq: CHROMADESTINATION,
                dc_table: CHROMADESTINATION,
                ac_table: CHROMADESTINATION,
                _dc_pred: 0,
            },
        ];
        let scale = u32::from(clamp(quality, 1, 100));
        let scale = if scale < 50 {
            5000 / scale
        } else {
            200 - scale * 2
        };
        let mut tables = vec![STD_LUMA_QTABLE, STD_CHROMA_QTABLE];
        for t in tables.iter_mut() {
            for v in t.iter_mut() {
                *v = clamp((u32::from(*v) * scale + 50) / 100, 1, u32::from(u8::MAX)) as u8;
            }
        }
        JpegEncoder {
            writer: BitWriter::new(w),
            components,
            tables,
            luma_dctable: Cow::Borrowed(&STD_LUMA_DC_HUFF_LUT),
            luma_actable: Cow::Borrowed(&STD_LUMA_AC_HUFF_LUT),
            chroma_dctable: Cow::Borrowed(&STD_CHROMA_DC_HUFF_LUT),
            chroma_actable: Cow::Borrowed(&STD_CHROMA_AC_HUFF_LUT),
            pixel_density: PixelDensity::default(),
            icc_profile: Vec::new(),
            exif: Vec::new(),
        }
    }

    #[track_caller]
    pub fn encode(
        &mut self,
        image: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        let expected_buffer_len = color_type.buffer_size(width, height);
        assert_eq!(
            expected_buffer_len,
            image.len() as u64,
            "Invalid buffer length: expected {expected_buffer_len} got {} for {width}x{height} image",
            image.len(),
        );
        match color_type {
            ExtendedColorType::L8 => {
                let image: ImageBuffer<Luma<_>, _> =
                    ImageBuffer::from_raw(width, height, image).unwrap();
                self.encode_image(&image)
            }
            ExtendedColorType::Rgb8 => {
                let image: ImageBuffer<Rgb<_>, _> =
                    ImageBuffer::from_raw(width, height, image).unwrap();
                self.encode_image(&image)
            }
            _ => Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Jpeg.into(),
                    UnsupportedErrorKind::Color(color_type),
                ),
            )),
        }
    }
    fn write_exif(&mut self) -> ImageResult<()> {
        if !self.exif.is_empty() {
            let mut formatted = EXIF_HEADER.to_vec();
            formatted.extend_from_slice(&self.exif);
            self.writer.write_segment(APP1, &formatted)?;
        }
        Ok(())
    }
    pub fn encode_image<I: GenericImageView>(&mut self, image: &I) -> ImageResult<()>
    where
        I::Pixel: PixelWithColorType,
    {
        let n = I::Pixel::CHANNEL_COUNT;
        let color_type = I::Pixel::COLOR_TYPE;
        let num_components = if n == 1 || n == 2 { 1 } else { 3 };
        let (width, height) = match (u16::try_from(image.width()), u16::try_from(image.height())) {
            (Ok(w @ 1..), Ok(h @ 1..)) => (w, h),
            _ => return Err(EncoderError::InvalidSize(image.width(), image.height()).into()),
        };
        self.writer.write_marker(SOI)?;
        let mut buf = Vec::new();
        build_jfif_header(&mut buf, self.pixel_density);
        self.writer.write_segment(APP0, &buf)?;
        self.write_exif()?;
        self.write_icc_profile_chunks()?;
        build_frame_header(
            &mut buf,
            8,
            width,
            height,
            &self.components[..num_components],
        );
        self.writer.write_segment(SOF0, &buf)?;
        assert_eq!(self.tables.len(), 2);
        let numtables = if num_components == 1 { 1 } else { 2 };
        for (i, table) in self.tables[..numtables].iter().enumerate() {
            build_quantization_segment(&mut buf, 8, i as u8, table);
            self.writer.write_segment(DQT, &buf)?;
        }
        build_huffman_segment(
            &mut buf,
            DCCLASS,
            LUMADESTINATION,
            &STD_LUMA_DC_CODE_LENGTHS,
            &STD_LUMA_DC_VALUES,
        );
        self.writer.write_segment(DHT, &buf)?;
        build_huffman_segment(
            &mut buf,
            ACCLASS,
            LUMADESTINATION,
            &STD_LUMA_AC_CODE_LENGTHS,
            &STD_LUMA_AC_VALUES,
        );
        self.writer.write_segment(DHT, &buf)?;
        if num_components == 3 {
            build_huffman_segment(
                &mut buf,
                DCCLASS,
                CHROMADESTINATION,
                &STD_CHROMA_DC_CODE_LENGTHS,
                &STD_CHROMA_DC_VALUES,
            );
            self.writer.write_segment(DHT, &buf)?;
            build_huffman_segment(
                &mut buf,
                ACCLASS,
                CHROMADESTINATION,
                &STD_CHROMA_AC_CODE_LENGTHS,
                &STD_CHROMA_AC_VALUES,
            );
            self.writer.write_segment(DHT, &buf)?;
        }
        build_scan_header(&mut buf, &self.components[..num_components]);
        self.writer.write_segment(SOS, &buf)?;
        if ExtendedColorType::Rgb8 == color_type || ExtendedColorType::Rgba8 == color_type {
            self.encode_rgb(image)
        } else {
            self.encode_gray(image)
        }?;
        self.writer.pad_byte()?;
        self.writer.write_marker(EOI)?;
        Ok(())
    }
    fn encode_gray<I: GenericImageView>(&mut self, image: &I) -> io::Result<()> {
        let mut yblock = [0u8; 64];
        let mut y_dcprev = 0;
        let mut dct_yblock = [0i32; 64];
        for y in (0..image.height()).step_by(8) {
            for x in (0..image.width()).step_by(8) {
                copy_blocks_gray(image, x, y, &mut yblock);
                transform::fdct(&yblock, &mut dct_yblock);
                for (i, dct) in dct_yblock.iter_mut().enumerate() {
                    *dct = ((*dct / 8) as f32 / f32::from(self.tables[0][i])).round() as i32;
                }
                let la = &*self.luma_actable;
                let ld = &*self.luma_dctable;
                y_dcprev = self.writer.write_block(&dct_yblock, y_dcprev, ld, la)?;
            }
        }
        Ok(())
    }
    fn encode_rgb<I: GenericImageView>(&mut self, image: &I) -> io::Result<()> {
        let mut y_dcprev = 0;
        let mut cb_dcprev = 0;
        let mut cr_dcprev = 0;
        let mut dct_yblock = [0i32; 64];
        let mut dct_cb_block = [0i32; 64];
        let mut dct_cr_block = [0i32; 64];
        let mut yblock = [0u8; 64];
        let mut cb_block = [0u8; 64];
        let mut cr_block = [0u8; 64];
        for y in (0..image.height()).step_by(8) {
            for x in (0..image.width()).step_by(8) {
                copy_blocks_ycbcr(image, x, y, &mut yblock, &mut cb_block, &mut cr_block);
                transform::fdct(&yblock, &mut dct_yblock);
                transform::fdct(&cb_block, &mut dct_cb_block);
                transform::fdct(&cr_block, &mut dct_cr_block);
                for i in 0usize..64 {
                    dct_yblock[i] =
                        ((dct_yblock[i] / 8) as f32 / f32::from(self.tables[0][i])).round() as i32;
                    dct_cb_block[i] = ((dct_cb_block[i] / 8) as f32 / f32::from(self.tables[1][i]))
                        .round() as i32;
                    dct_cr_block[i] = ((dct_cr_block[i] / 8) as f32 / f32::from(self.tables[1][i]))
                        .round() as i32;
                }
                let la = &*self.luma_actable;
                let ld = &*self.luma_dctable;
                let cd = &*self.chroma_dctable;
                let ca = &*self.chroma_actable;
                y_dcprev = self.writer.write_block(&dct_yblock, y_dcprev, ld, la)?;
                cb_dcprev = self.writer.write_block(&dct_cb_block, cb_dcprev, cd, ca)?;
                cr_dcprev = self.writer.write_block(&dct_cr_block, cr_dcprev, cd, ca)?;
            }
        }
        Ok(())
    }
    fn write_icc_profile_chunks(&mut self) -> io::Result<()> {
        if self.icc_profile.is_empty() {
            return Ok(());
        }
        const MAX_CHUNK_SIZE: usize = 65533 - 14;
        const MAX_CHUNK_COUNT: usize = 255;
        const MAX_ICC_PROFILE_SIZE: usize = MAX_CHUNK_SIZE * MAX_CHUNK_COUNT;
        if self.icc_profile.len() > MAX_ICC_PROFILE_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ICC profile too large",
            ));
        }
        let chunk_iter = self.icc_profile.chunks(MAX_CHUNK_SIZE);
        let num_chunks = chunk_iter.len() as u8;
        let mut segment = Vec::new();
        for (i, chunk) in chunk_iter.enumerate() {
            let chunk_number = (i + 1) as u8;
            let length = 14 + chunk.len();
            segment.clear();
            segment.reserve(length);
            segment.extend_from_slice(b"ICC_PROFILE\0");
            segment.push(chunk_number);
            segment.push(num_chunks);
            segment.extend_from_slice(chunk);
            self.writer.write_segment(APP2, &segment)?;
        }
        Ok(())
    }
}
impl<W: Write> ImageEncoder for JpegEncoder<W> {
    #[track_caller]
    fn write_image(
        mut self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        self.encode(buf, width, height, color_type)
    }
    fn set_icc_profile(&mut self, icc_profile: Vec<u8>) -> Result<(), UnsupportedError> {
        self.icc_profile = icc_profile;
        Ok(())
    }
    fn set_exif_metadata(&mut self, exif: Vec<u8>) -> Result<(), UnsupportedError> {
        self.exif = exif;
        Ok(())
    }
    fn make_compatible_img(
        &self,
        _: crate::image::io::encoder::MethodSealedToImage,
        img: &DynamicImage,
    ) -> Option<DynamicImage> {
        use ColorType::*;
        match img.color() {
            L8 | Rgb8 => None,
            La8 | L16 | La16 => Some(img.to_luma8().into()),
            Rgba8 | Rgb16 | Rgb32F | Rgba16 | Rgba32F => Some(img.to_rgb8().into()),
        }
    }
}
fn build_jfif_header(m: &mut Vec<u8>, density: PixelDensity) {
    m.clear();
    m.extend_from_slice(b"JFIF");
    m.extend_from_slice(&[
        0,
        0x01,
        0x02,
        match density.unit {
            PixelDensityUnit::PixelAspectRatio => 0x00,
            PixelDensityUnit::Inches => 0x01,
            PixelDensityUnit::Centimeters => 0x02,
        },
    ]);
    m.extend_from_slice(&density.density.0.to_be_bytes());
    m.extend_from_slice(&density.density.1.to_be_bytes());
    m.extend_from_slice(&[0, 0]);
}
fn build_frame_header(
    m: &mut Vec<u8>,
    precision: u8,
    width: u16,
    height: u16,
    components: &[Component],
) {
    m.clear();
    m.push(precision);
    m.extend_from_slice(&height.to_be_bytes());
    m.extend_from_slice(&width.to_be_bytes());
    m.push(components.len() as u8);
    for &comp in components {
        let hv = (comp.h << 4) | comp.v;
        m.extend_from_slice(&[comp.id, hv, comp.tq]);
    }
}
fn build_scan_header(m: &mut Vec<u8>, components: &[Component]) {
    m.clear();
    m.push(components.len() as u8);
    for &comp in components {
        let tables = (comp.dc_table << 4) | comp.ac_table;
        m.extend_from_slice(&[comp.id, tables]);
    }
    m.extend_from_slice(&[0, 63, 0]);
}
fn build_huffman_segment(
    m: &mut Vec<u8>,
    class: u8,
    destination: u8,
    numcodes: &[u8; 16],
    values: &[u8],
) {
    m.clear();
    let tcth = (class << 4) | destination;
    m.push(tcth);
    m.extend_from_slice(numcodes);
    let sum: usize = numcodes.iter().map(|&x| x as usize).sum();
    assert_eq!(sum, values.len());
    m.extend_from_slice(values);
}
fn build_quantization_segment(m: &mut Vec<u8>, precision: u8, identifier: u8, qtable: &[u8; 64]) {
    m.clear();
    let p = if precision == 8 { 0 } else { 1 };
    let pqtq = (p << 4) | identifier;
    m.push(pqtq);
    for &i in &UNZIGZAG[..] {
        m.push(qtable[i as usize]);
    }
}
fn encode_coefficient(coefficient: i32) -> (u8, u16) {
    let mut magnitude = coefficient.unsigned_abs() as u16;
    let mut num_bits = 0u8;
    while magnitude > 0 {
        magnitude >>= 1;
        num_bits += 1;
    }
    let mask = (1 << num_bits as usize) - 1;
    let val = if coefficient < 0 {
        (coefficient - 1) as u16 & mask
    } else {
        coefficient as u16 & mask
    };
    (num_bits, val)
}
#[inline]
fn rgb_to_ycbcr<P: Pixel>(pixel: P) -> (u8, u8, u8) {
    let [r, g, b] = pixel.to_rgb().0;
    let r: i32 = i32::from(r.to_u8().unwrap());
    let g: i32 = i32::from(g.to_u8().unwrap());
    let b: i32 = i32::from(b.to_u8().unwrap());
    const C_YR: i32 = 19595;
    const C_YG: i32 = 38469;
    const C_YB: i32 = 7471;
    const Y_ROUNDING: i32 = (1 << 15) - 1;
    const C_UR: i32 = 11059;
    const C_UG: i32 = 21709;
    const C_UB: i32 = 32768;
    const UV_BIAS_ROUNDING: i32 = (128 * (1 << 16)) + ((1 << 15) - 1);
    const C_VR: i32 = C_UB;
    const C_VG: i32 = 27439;
    const C_VB: i32 = 5329;
    let y = (C_YR * r + C_YG * g + C_YB * b + Y_ROUNDING) >> 16;
    let cb = (-C_UR * r - C_UG * g + C_UB * b + UV_BIAS_ROUNDING) >> 16;
    let cr = (C_VR * r - C_VG * g - C_VB * b + UV_BIAS_ROUNDING) >> 16;
    (y as u8, cb as u8, cr as u8)
}
#[inline]
fn pixel_at_or_near<I: GenericImageView>(source: &I, x: u32, y: u32) -> I::Pixel {
    if source.in_bounds(x, y) {
        source.get_pixel(x, y)
    } else {
        source.get_pixel(x.min(source.width() - 1), y.min(source.height() - 1))
    }
}
fn copy_blocks_ycbcr<I: GenericImageView>(
    source: &I,
    x0: u32,
    y0: u32,
    yb: &mut [u8; 64],
    cbb: &mut [u8; 64],
    crb: &mut [u8; 64],
) {
    for y in 0..8 {
        for x in 0..8 {
            let pixel = pixel_at_or_near(source, x + x0, y + y0);
            let (yc, cb, cr) = rgb_to_ycbcr(pixel);
            yb[(y * 8 + x) as usize] = yc;
            cbb[(y * 8 + x) as usize] = cb;
            crb[(y * 8 + x) as usize] = cr;
        }
    }
}
fn copy_blocks_gray<I: GenericImageView>(source: &I, x0: u32, y0: u32, gb: &mut [u8; 64]) {
    use num_traits::cast::ToPrimitive;
    for y in 0..8 {
        for x in 0..8 {
            let pixel = pixel_at_or_near(source, x0 + x, y0 + y);
            let [luma] = pixel.to_luma().0;
            gb[(y * 8 + x) as usize] = luma.to_u8().unwrap();
        }
    }
}
