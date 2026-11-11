// Adapted from image 0.25.10: https://github.com/image-rs/image/blob/v0.25.10/src/codecs/bmp/decoder.rs; see LICENSE-MIT.
use super::read::{ReadBytes, vec_try_with_capacity};
mod bitfields;
mod errors;
mod rows;
use bitfields::{Bitfield, Bitfields};
use errors::{ChannelWidthError, DecoderError};
use std::cmp::{self, Ordering};
use std::io::{self, BufRead, Seek, SeekFrom};
use std::iter::{Rev, repeat};
use std::slice::ChunksExactMut;

use crate::image::ColorType;
use crate::image::error::{ImageError, ImageResult, UnsupportedError, UnsupportedErrorKind};

use crate::image::{ImageDecoder, ImageFormat};

const BITMAPCOREHEADER_SIZE: u32 = 12;
const BITMAPINFOHEADER_SIZE: u32 = 40;
const BITMAPV2HEADER_SIZE: u32 = 52;
const BITMAPV3HEADER_SIZE: u32 = 56;
const BITMAPV4HEADER_SIZE: u32 = 108;
const BITMAPV5HEADER_SIZE: u32 = 124;

static R5_G5_B5_COLOR_MASK: Bitfields = Bitfields {
    r: Bitfield { len: 5, shift: 10 },
    g: Bitfield { len: 5, shift: 5 },
    b: Bitfield { len: 5, shift: 0 },
    a: Bitfield { len: 0, shift: 0 },
};
const R8_G8_B8_COLOR_MASK: Bitfields = Bitfields {
    r: Bitfield { len: 8, shift: 24 },
    g: Bitfield { len: 8, shift: 16 },
    b: Bitfield { len: 8, shift: 8 },
    a: Bitfield { len: 0, shift: 0 },
};
const R8_G8_B8_A8_COLOR_MASK: Bitfields = Bitfields {
    r: Bitfield { len: 8, shift: 16 },
    g: Bitfield { len: 8, shift: 8 },
    b: Bitfield { len: 8, shift: 0 },
    a: Bitfield { len: 8, shift: 24 },
};

const RLE_ESCAPE: u8 = 0;
const RLE_ESCAPE_EOL: u8 = 0;
const RLE_ESCAPE_EOF: u8 = 1;
const RLE_ESCAPE_DELTA: u8 = 2;

const MAX_WIDTH_HEIGHT: i32 = 0xFFFF;

#[derive(PartialEq, Copy, Clone)]
enum ImageType {
    Palette,
    RGB16,
    RGB24,
    RGB32,
    RLE8,
    RLE4,
    Bitfields16,
    Bitfields32,
}

#[derive(PartialEq)]
enum BMPHeaderType {
    Core,
    Info,
    V2,
    V3,
    V4,
    V5,
}

#[derive(PartialEq)]
enum FormatFullBytes {
    RGB24,
    RGB32,
    RGBA32,
    Format888,
}

enum Chunker<'a> {
    FromTop(ChunksExactMut<'a, u8>),
    FromBottom(Rev<ChunksExactMut<'a, u8>>),
}

struct RowIterator<'a> {
    chunks: Chunker<'a>,
}

impl<'a> Iterator for RowIterator<'a> {
    type Item = &'a mut [u8];

    #[inline(always)]
    fn next(&mut self) -> Option<&'a mut [u8]> {
        match self.chunks {
            Chunker::FromTop(ref mut chunks) => chunks.next(),
            Chunker::FromBottom(ref mut chunks) => chunks.next(),
        }
    }
}

fn check_for_overflow(width: i32, length: i32, channels: usize) -> ImageResult<()> {
    num_bytes(width, length, channels)
        .map(|_| ())
        .ok_or_else(|| {
            ImageError::Unsupported(UnsupportedError::from_format_and_kind(
                ImageFormat::Bmp.into(),
                UnsupportedErrorKind::GenericFeature(format!(
                    "Image dimensions ({width}x{length} w/{channels} channels) are too large"
                )),
            ))
        })
}

fn num_bytes(width: i32, length: i32, channels: usize) -> Option<usize> {
    if width <= 0 || length <= 0 {
        None
    } else {
        match channels.checked_mul(width as usize) {
            Some(n) => n.checked_mul(length as usize),
            None => None,
        }
    }
}

fn with_rows<F>(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    channels: usize,
    top_down: bool,
    mut func: F,
) -> io::Result<()>
where
    F: FnMut(&mut [u8]) -> io::Result<()>,
{
    let row_width = channels.checked_mul(width as usize).unwrap();
    let full_image_size = row_width.checked_mul(height as usize).unwrap();
    assert_eq!(buffer.len(), full_image_size);

    if !top_down {
        for row in buffer.chunks_mut(row_width).rev() {
            func(row)?;
        }
    } else {
        for row in buffer.chunks_mut(row_width) {
            func(row)?;
        }
    }
    Ok(())
}

fn set_8bit_pixel_run<'a, T: Iterator<Item = &'a u8>>(
    pixel_iter: &mut ChunksExactMut<u8>,
    palette: &[[u8; 3]],
    indices: T,
    n_pixels: usize,
) -> bool {
    for idx in indices.take(n_pixels) {
        if let Some(pixel) = pixel_iter.next() {
            let rgb = palette[*idx as usize];
            pixel[0] = rgb[0];
            pixel[1] = rgb[1];
            pixel[2] = rgb[2];
        } else {
            return false;
        }
    }
    true
}

fn set_4bit_pixel_run<'a, T: Iterator<Item = &'a u8>>(
    pixel_iter: &mut ChunksExactMut<u8>,
    palette: &[[u8; 3]],
    indices: T,
    mut n_pixels: usize,
) -> bool {
    for idx in indices {
        macro_rules! set_pixel {
            ($i:expr) => {
                if n_pixels == 0 {
                    break;
                }
                if let Some(pixel) = pixel_iter.next() {
                    let rgb = palette[$i as usize];
                    pixel[0] = rgb[0];
                    pixel[1] = rgb[1];
                    pixel[2] = rgb[2];
                } else {
                    return false;
                }
                n_pixels -= 1;
            };
        }
        set_pixel!(idx >> 4);
        set_pixel!(idx & 0xf);
    }
    true
}

#[rustfmt::skip]
fn set_2bit_pixel_run<'a, T: Iterator<Item = &'a u8>>(
    pixel_iter: &mut ChunksExactMut<u8>,
    palette: &[[u8; 3]],
    indices: T,
    mut n_pixels: usize,
) -> bool {
    for idx in indices {
        macro_rules! set_pixel {
            ($i:expr) => {
                if n_pixels == 0 {
                    break;
                }
                if let Some(pixel) = pixel_iter.next() {
                    let rgb = palette[$i as usize];
                    pixel[0] = rgb[0];
                    pixel[1] = rgb[1];
                    pixel[2] = rgb[2];
                } else {
                    return false;
                }
                n_pixels -= 1;
            };
        }
        set_pixel!((idx >> 6) & 0x3u8);
        set_pixel!((idx >> 4) & 0x3u8);
        set_pixel!((idx >> 2) & 0x3u8);
        set_pixel!( idx       & 0x3u8);
    }
    true
}

fn set_1bit_pixel_run<'a, T: Iterator<Item = &'a u8>>(
    pixel_iter: &mut ChunksExactMut<u8>,
    palette: &[[u8; 3]],
    indices: T,
) {
    for idx in indices {
        let mut bit = 0x80;
        while bit != 0 {
            if let Some(pixel) = pixel_iter.next() {
                let rgb = palette[usize::from((idx & bit) != 0)];
                pixel[0] = rgb[0];
                pixel[1] = rgb[1];
                pixel[2] = rgb[2];
            } else {
                return;
            }

            bit >>= 1;
        }
    }
}

pub(super) struct BmpDecoder<R> {
    reader: R,

    bmp_header_type: BMPHeaderType,

    width: i32,
    height: i32,
    data_offset: u64,
    top_down: bool,
    has_alpha: bool,
    image_type: ImageType,

    bit_count: u16,
    colors_used: u32,
    palette: Option<Vec<[u8; 3]>>,
    bitfields: Option<Bitfields>,
}

enum RLEInsn {
    EndOfFile,
    EndOfRow,
    Delta(u8, u8),
    Absolute(u8, Vec<u8>),
    PixelRun(u8, u8),
}

impl<R: BufRead + Seek> BmpDecoder<R> {
    pub(super) fn new(reader: R) -> ImageResult<BmpDecoder<R>> {
        let mut decoder = Self {
            reader,

            bmp_header_type: BMPHeaderType::Info,

            width: 0,
            height: 0,
            data_offset: 0,
            top_down: false,
            has_alpha: false,
            image_type: ImageType::Palette,

            bit_count: 0,
            colors_used: 0,
            palette: None,
            bitfields: None,
        };
        decoder.read_metadata()?;
        Ok(decoder)
    }

    fn read_file_header(&mut self) -> ImageResult<()> {
        let mut signature = [0; 2];
        self.reader.read_exact(&mut signature)?;

        if signature != b"BM"[..] {
            return Err(DecoderError::BmpSignatureInvalid.into());
        }

        self.reader.read_u32()?;
        self.reader.read_u32()?;

        self.data_offset = u64::from(self.reader.read_u32()?);

        Ok(())
    }

    fn read_bitmap_core_header(&mut self) -> ImageResult<()> {
        self.width = i32::from(self.reader.read_u16()?);
        self.height = i32::from(self.reader.read_u16()?);

        check_for_overflow(self.width, self.height, self.num_channels())?;

        if self.reader.read_u16()? != 1 {
            return Err(DecoderError::MoreThanOnePlane.into());
        }

        self.bit_count = self.reader.read_u16()?;
        self.image_type = match self.bit_count {
            1 | 4 | 8 => ImageType::Palette,
            24 => ImageType::RGB24,
            _ => {
                return Err(DecoderError::InvalidChannelWidth(
                    ChannelWidthError::Rgb,
                    self.bit_count,
                )
                .into());
            }
        };

        Ok(())
    }

    fn read_bitmap_info_header(&mut self) -> ImageResult<()> {
        self.width = self.reader.read_i32()?;
        self.height = self.reader.read_i32()?;

        if self.width < 0 {
            return Err(DecoderError::NegativeWidth(self.width).into());
        }

        if self.height == i32::MIN {
            return Err(DecoderError::InvalidHeight.into());
        }

        if self.height < 0 {
            self.height *= -1;
            self.top_down = true;
        }
        if self.width > MAX_WIDTH_HEIGHT || self.height > MAX_WIDTH_HEIGHT {
            return Err(DecoderError::ImageTooLarge(self.width, self.height).into());
        }

        check_for_overflow(self.width, self.height, self.num_channels())?;

        if self.reader.read_u16()? != 1 {
            return Err(DecoderError::MoreThanOnePlane.into());
        }

        self.bit_count = self.reader.read_u16()?;
        let image_type_u32 = self.reader.read_u32()?;

        if self.top_down && image_type_u32 != 0 && image_type_u32 != 3 {
            return Err(DecoderError::ImageTypeInvalidForTopDown(image_type_u32).into());
        }
        self.image_type = match image_type_u32 {
            0 => match self.bit_count {
                1 | 2 | 4 | 8 => ImageType::Palette,
                16 => ImageType::RGB16,
                24 => ImageType::RGB24,
                32 => ImageType::RGB32,
                _ => {
                    return Err(DecoderError::InvalidChannelWidth(
                        ChannelWidthError::Rgb,
                        self.bit_count,
                    )
                    .into());
                }
            },
            1 => match self.bit_count {
                8 => ImageType::RLE8,
                _ => {
                    return Err(DecoderError::InvalidChannelWidth(
                        ChannelWidthError::Rle8,
                        self.bit_count,
                    )
                    .into());
                }
            },
            2 => match self.bit_count {
                4 => ImageType::RLE4,
                _ => {
                    return Err(DecoderError::InvalidChannelWidth(
                        ChannelWidthError::Rle4,
                        self.bit_count,
                    )
                    .into());
                }
            },
            3 => match self.bit_count {
                16 => ImageType::Bitfields16,
                32 => ImageType::Bitfields32,
                _ => {
                    return Err(DecoderError::InvalidChannelWidth(
                        ChannelWidthError::Bitfields,
                        self.bit_count,
                    )
                    .into());
                }
            },
            4 => {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Bmp.into(),
                        UnsupportedErrorKind::GenericFeature("JPEG compression".to_owned()),
                    ),
                ));
            }
            5 => {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Bmp.into(),
                        UnsupportedErrorKind::GenericFeature("PNG compression".to_owned()),
                    ),
                ));
            }
            11..=13 => {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Bmp.into(),
                        UnsupportedErrorKind::GenericFeature("CMYK format".to_owned()),
                    ),
                ));
            }
            _ => {
                return Err(DecoderError::ImageTypeUnknown(image_type_u32).into());
            }
        };

        self.reader.read_u32()?;
        self.reader.read_u32()?;
        self.reader.read_u32()?;

        self.colors_used = self.reader.read_u32()?;

        self.reader.read_u32()?;

        Ok(())
    }

    fn read_bitmasks(&mut self) -> ImageResult<()> {
        let r_mask = self.reader.read_u32()?;
        let g_mask = self.reader.read_u32()?;
        let b_mask = self.reader.read_u32()?;

        let a_mask = match self.bmp_header_type {
            BMPHeaderType::V3 | BMPHeaderType::V4 | BMPHeaderType::V5 => self.reader.read_u32()?,
            _ => 0,
        };

        self.bitfields = match self.image_type {
            ImageType::Bitfields16 => {
                Some(Bitfields::from_mask(r_mask, g_mask, b_mask, a_mask, 16)?)
            }
            ImageType::Bitfields32 => {
                Some(Bitfields::from_mask(r_mask, g_mask, b_mask, a_mask, 32)?)
            }
            _ => None,
        };

        if self.bitfields.is_some() && a_mask != 0 {
            self.has_alpha = true;
        }

        Ok(())
    }

    fn read_metadata(&mut self) -> ImageResult<()> {
        self.read_file_header()?;
        let bmp_header_offset = self.reader.stream_position()?;
        let bmp_header_size = self.reader.read_u32()?;
        let bmp_header_end = bmp_header_offset + u64::from(bmp_header_size);

        self.bmp_header_type = match bmp_header_size {
            BITMAPCOREHEADER_SIZE => BMPHeaderType::Core,
            BITMAPINFOHEADER_SIZE => BMPHeaderType::Info,
            BITMAPV2HEADER_SIZE => BMPHeaderType::V2,
            BITMAPV3HEADER_SIZE => BMPHeaderType::V3,
            BITMAPV4HEADER_SIZE => BMPHeaderType::V4,
            BITMAPV5HEADER_SIZE => BMPHeaderType::V5,
            _ if bmp_header_size < BITMAPCOREHEADER_SIZE => {
                return Err(DecoderError::HeaderTooSmall(bmp_header_size).into());
            }
            _ => {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Bmp.into(),
                        UnsupportedErrorKind::GenericFeature(format!(
                            "Unknown bitmap header type (size={bmp_header_size})"
                        )),
                    ),
                ));
            }
        };

        match self.bmp_header_type {
            BMPHeaderType::Core => {
                self.read_bitmap_core_header()?;
            }
            BMPHeaderType::Info
            | BMPHeaderType::V2
            | BMPHeaderType::V3
            | BMPHeaderType::V4
            | BMPHeaderType::V5 => {
                self.read_bitmap_info_header()?;
            }
        }

        let mut bitmask_bytes_offset = 0;
        if self.image_type == ImageType::Bitfields16 || self.image_type == ImageType::Bitfields32 {
            self.read_bitmasks()?;

            if matches!(
                self.bmp_header_type,
                BMPHeaderType::Info | BMPHeaderType::V4 | BMPHeaderType::V5
            ) {
                bitmask_bytes_offset = 12;
            }
        };

        self.reader
            .seek(SeekFrom::Start(bmp_header_end + bitmask_bytes_offset))?;

        match self.image_type {
            ImageType::Palette | ImageType::RLE4 | ImageType::RLE8 => self.read_palette()?,
            _ => {}
        }

        Ok(())
    }

    fn get_palette_size(&mut self) -> ImageResult<usize> {
        match self.colors_used {
            0 => Ok(1 << self.bit_count),
            _ => {
                if self.colors_used > 1 << self.bit_count {
                    return Err(DecoderError::PaletteSizeExceeded {
                        colors_used: self.colors_used,
                        bit_count: self.bit_count,
                    }
                    .into());
                }
                Ok(self.colors_used as usize)
            }
        }
    }

    fn bytes_per_color(&self) -> usize {
        match self.bmp_header_type {
            BMPHeaderType::Core => 3,
            _ => 4,
        }
    }

    fn read_palette(&mut self) -> ImageResult<()> {
        const MAX_PALETTE_SIZE: usize = 256;

        let bytes_per_color = self.bytes_per_color();
        let palette_size = self.get_palette_size()?;
        let max_length = MAX_PALETTE_SIZE * bytes_per_color;

        let length = palette_size * bytes_per_color;
        let mut buf = vec_try_with_capacity(max_length)?;

        buf.resize(cmp::min(length, max_length), 0);
        self.reader.by_ref().read_exact(&mut buf)?;

        match length.cmp(&max_length) {
            Ordering::Greater => {
                self.reader
                    .seek(SeekFrom::Current((length - max_length) as i64))?;
            }
            Ordering::Less => buf.resize(max_length, 0),
            Ordering::Equal => (),
        }

        let p: Vec<[u8; 3]> = (0..MAX_PALETTE_SIZE)
            .map(|i| {
                let b = buf[bytes_per_color * i];
                let g = buf[bytes_per_color * i + 1];
                let r = buf[bytes_per_color * i + 2];
                [r, g, b]
            })
            .collect();

        self.palette = Some(p);

        Ok(())
    }

    fn num_channels(&self) -> usize {
        if self.has_alpha { 4 } else { 3 }
    }

    fn rows<'a>(&self, pixel_data: &'a mut [u8]) -> RowIterator<'a> {
        let stride = self.width as usize * self.num_channels();
        if self.top_down {
            RowIterator {
                chunks: Chunker::FromTop(pixel_data.chunks_exact_mut(stride)),
            }
        } else {
            RowIterator {
                chunks: Chunker::FromBottom(pixel_data.chunks_exact_mut(stride).rev()),
            }
        }
    }

    fn read_palettized_pixel_data(&mut self, buf: &mut [u8]) -> ImageResult<()> {
        let num_channels = self.num_channels();
        let row_byte_length = ((i32::from(self.bit_count) * self.width + 31) / 32 * 4) as usize;
        let mut indices = vec![0; row_byte_length];
        let palette = self.palette.as_ref().unwrap();
        let bit_count = self.bit_count;
        let reader = &mut self.reader;
        let width = self.width as usize;

        reader.seek(SeekFrom::Start(self.data_offset))?;

        if num_channels == 4 {
            buf.chunks_exact_mut(4).for_each(|c| c[3] = 0xFF);
        }

        with_rows(
            buf,
            self.width,
            self.height,
            num_channels,
            self.top_down,
            |row| {
                reader.read_exact(&mut indices)?;
                let mut pixel_iter = row.chunks_exact_mut(num_channels);
                match bit_count {
                    1 => {
                        set_1bit_pixel_run(&mut pixel_iter, palette, indices.iter());
                    }
                    2 => {
                        set_2bit_pixel_run(&mut pixel_iter, palette, indices.iter(), width);
                    }
                    4 => {
                        set_4bit_pixel_run(&mut pixel_iter, palette, indices.iter(), width);
                    }
                    8 => {
                        set_8bit_pixel_run(&mut pixel_iter, palette, indices.iter(), width);
                    }
                    _ => panic!(),
                }
                Ok(())
            },
        )?;

        Ok(())
    }

    fn read_16_bit_pixel_data(
        &mut self,
        buf: &mut [u8],
        bitfields: Option<&Bitfields>,
    ) -> ImageResult<()> {
        let num_channels = self.num_channels();
        let row_padding_len = self.width as usize % 2 * 2;
        let row_padding = &mut [0; 2][..row_padding_len];
        let bitfields = match bitfields {
            Some(b) => b,
            None => self.bitfields.as_ref().unwrap(),
        };
        let reader = &mut self.reader;

        reader.seek(SeekFrom::Start(self.data_offset))?;

        with_rows(
            buf,
            self.width,
            self.height,
            num_channels,
            self.top_down,
            |row| {
                for pixel in row.chunks_mut(num_channels) {
                    let data = u32::from(reader.read_u16()?);

                    pixel[0] = bitfields.r.read(data);
                    pixel[1] = bitfields.g.read(data);
                    pixel[2] = bitfields.b.read(data);
                    if num_channels == 4 {
                        if bitfields.a.len != 0 {
                            pixel[3] = bitfields.a.read(data);
                        } else {
                            pixel[3] = 0xFF;
                        }
                    }
                }
                reader.read_exact(row_padding)
            },
        )?;

        Ok(())
    }

    fn read_32_bit_pixel_data(&mut self, buf: &mut [u8]) -> ImageResult<()> {
        let num_channels = self.num_channels();

        let bitfields = self.bitfields.as_ref().unwrap();

        let reader = &mut self.reader;
        reader.seek(SeekFrom::Start(self.data_offset))?;

        with_rows(
            buf,
            self.width,
            self.height,
            num_channels,
            self.top_down,
            |row| {
                for pixel in row.chunks_mut(num_channels) {
                    let data = reader.read_u32()?;

                    pixel[0] = bitfields.r.read(data);
                    pixel[1] = bitfields.g.read(data);
                    pixel[2] = bitfields.b.read(data);
                    if num_channels == 4 {
                        if bitfields.a.len != 0 {
                            pixel[3] = bitfields.a.read(data);
                        } else {
                            pixel[3] = 0xff;
                        }
                    }
                }
                Ok(())
            },
        )?;

        Ok(())
    }

    fn read_rle_data(&mut self, buf: &mut [u8], image_type: ImageType) -> ImageResult<()> {
        self.reader.seek(SeekFrom::Start(self.data_offset))?;

        let num_channels = self.num_channels();
        let p = self.palette.as_ref().unwrap();

        let mut row_iter = self.rows(buf);

        while let Some(row) = row_iter.next() {
            let mut pixel_iter = row.chunks_exact_mut(num_channels);

            let mut x = 0;
            let mut end_of_row = false;
            while !end_of_row {
                let instruction = {
                    let control_byte = self.reader.read_u8()?;
                    match control_byte {
                        RLE_ESCAPE => {
                            let op = self.reader.read_u8()?;

                            match op {
                                RLE_ESCAPE_EOL => RLEInsn::EndOfRow,
                                RLE_ESCAPE_EOF => RLEInsn::EndOfFile,
                                RLE_ESCAPE_DELTA => {
                                    let xdelta = self.reader.read_u8()?;
                                    let ydelta = self.reader.read_u8()?;
                                    RLEInsn::Delta(xdelta, ydelta)
                                }
                                _ => {
                                    let mut length = op as usize;
                                    if self.image_type == ImageType::RLE4 {
                                        length = length.div_ceil(2);
                                    }
                                    length += length & 1;
                                    let mut buffer = Vec::new();
                                    self.reader.read_exact_vec(&mut buffer, length)?;
                                    RLEInsn::Absolute(op, buffer)
                                }
                            }
                        }
                        _ => {
                            let palette_index = self.reader.read_u8()?;
                            RLEInsn::PixelRun(control_byte, palette_index)
                        }
                    }
                };

                match instruction {
                    RLEInsn::EndOfFile => {
                        pixel_iter.for_each(|p| p.fill(0));
                        row_iter.for_each(|r| r.fill(0));
                        return Ok(());
                    }
                    RLEInsn::EndOfRow => {
                        pixel_iter.by_ref().for_each(|p| p.fill(0));
                        end_of_row = true;
                    }
                    RLEInsn::Delta(x_delta, y_delta) => {
                        if y_delta > 0 {
                            pixel_iter.for_each(|p| p.fill(0));

                            for _ in 1..y_delta {
                                let row = row_iter.next().ok_or(DecoderError::CorruptRleData)?;
                                row.fill(0);
                            }

                            pixel_iter = row_iter
                                .next()
                                .ok_or(DecoderError::CorruptRleData)?
                                .chunks_exact_mut(num_channels);

                            for _ in 0..x {
                                pixel_iter
                                    .next()
                                    .ok_or(DecoderError::CorruptRleData)?
                                    .fill(0);
                            }
                        }

                        for _ in 0..x_delta {
                            let pixel = pixel_iter.next().ok_or(DecoderError::CorruptRleData)?;
                            pixel.fill(0);
                        }
                        x += x_delta as usize;
                    }
                    RLEInsn::Absolute(length, indices) => {
                        match image_type {
                            ImageType::RLE8 => {
                                if !set_8bit_pixel_run(
                                    &mut pixel_iter,
                                    p,
                                    indices.iter(),
                                    length as usize,
                                ) {
                                    return Err(DecoderError::CorruptRleData.into());
                                }
                            }
                            ImageType::RLE4 => {
                                if !set_4bit_pixel_run(
                                    &mut pixel_iter,
                                    p,
                                    indices.iter(),
                                    length as usize,
                                ) {
                                    return Err(DecoderError::CorruptRleData.into());
                                }
                            }
                            _ => unreachable!(),
                        }
                        x += length as usize;
                    }
                    RLEInsn::PixelRun(n_pixels, palette_index) => {
                        match image_type {
                            ImageType::RLE8 => {
                                let repeat_pixel: [u8; 3] = p[palette_index as usize];
                                (&mut pixel_iter).take(n_pixels as usize).for_each(|p| {
                                    p[2] = repeat_pixel[2];
                                    p[1] = repeat_pixel[1];
                                    p[0] = repeat_pixel[0];
                                });
                            }
                            ImageType::RLE4 => {
                                if !set_4bit_pixel_run(
                                    &mut pixel_iter,
                                    p,
                                    repeat(&palette_index),
                                    n_pixels as usize,
                                ) {
                                    return Err(DecoderError::CorruptRleData.into());
                                }
                            }
                            _ => unreachable!(),
                        }
                        x += n_pixels as usize;
                    }
                }
            }
        }

        Ok(())
    }

    pub(crate) fn read_image_data(&mut self, buf: &mut [u8]) -> ImageResult<()> {
        match self.image_type {
            ImageType::Palette => self.read_palettized_pixel_data(buf),
            ImageType::RGB16 => self.read_16_bit_pixel_data(buf, Some(&R5_G5_B5_COLOR_MASK)),
            ImageType::RGB24 => self.read_full_byte_pixel_data(buf, &FormatFullBytes::RGB24),
            ImageType::RGB32 => self.read_full_byte_pixel_data(buf, &FormatFullBytes::RGB32),
            ImageType::RLE8 => self.read_rle_data(buf, ImageType::RLE8),
            ImageType::RLE4 => self.read_rle_data(buf, ImageType::RLE4),
            ImageType::Bitfields16 => match self.bitfields {
                Some(_) => self.read_16_bit_pixel_data(buf, None),
                None => Err(DecoderError::BitfieldMasksMissing(16).into()),
            },
            ImageType::Bitfields32 => match self.bitfields {
                Some(R8_G8_B8_COLOR_MASK) => {
                    self.read_full_byte_pixel_data(buf, &FormatFullBytes::Format888)
                }
                Some(R8_G8_B8_A8_COLOR_MASK) => {
                    self.read_full_byte_pixel_data(buf, &FormatFullBytes::RGBA32)
                }
                Some(_) => self.read_32_bit_pixel_data(buf),
                None => Err(DecoderError::BitfieldMasksMissing(32).into()),
            },
        }
    }
}

impl<R: BufRead + Seek> ImageDecoder for BmpDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        (self.width as u32, self.height as u32)
    }

    fn color_type(&self) -> ColorType {
        if self.has_alpha {
            ColorType::Rgba8
        } else {
            ColorType::Rgb8
        }
    }

    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));
        self.read_image_data(buf)
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/media/thumbnails/bmp/decoder.rs"]
mod tests;
