#[cfg(doc)]
use crate::image_codecs::png::decoder::Reader;
use core::ops::RangeTo;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Adam7Info {
    pub(crate) pass: u8,
    pub(crate) line: u32,
    pub(crate) width: u32,
    pub(crate) samples: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BitPostion {
    byte: usize,
    bit: u8,
}
impl Adam7Info {
    pub fn new(pass: u8, line: u32, width: u32) -> Self {
        assert!((1..=7).contains(&pass));
        assert!(width > 0);
        let info = PassConstants::PASSES[pass as usize - 1];
        let samples = info.count_samples(width);
        Self {
            pass,
            line,
            width,
            samples,
        }
    }
    fn pass_constants(&self) -> PassConstants {
        PassConstants::PASSES[self.pass as usize - 1]
    }
    fn splat_pixel_repeat(self, idx: usize) -> u8 {
        let pass = self.pass_constants();
        let x_pixel = idx as u32 * u32::from(pass.x_sampling) + u32::from(pass.x_offset);
        (self.width - x_pixel).min(pass.splat_x_repeat().into()) as u8
    }
    fn splat_line_repeat(self, height: u32) -> u8 {
        let pass = self.pass_constants();
        let y_line = self.line * u32::from(pass.y_sampling) + u32::from(pass.y_offset);
        (height - y_line).min(pass.splat_y_repeat().into()) as u8
    }
}
#[derive(Clone, Copy)]
pub(crate) struct PassConstants {
    x_sampling: u8,
    x_offset: u8,
    y_sampling: u8,
    y_offset: u8,
}
impl PassConstants {
    const fn splat_x_repeat(self) -> u8 {
        self.x_sampling - self.x_offset
    }
    const fn splat_y_repeat(self) -> u8 {
        self.y_sampling - self.y_offset
    }
    pub fn count_samples(self, width: u32) -> u32 {
        width
            .saturating_sub(u32::from(self.x_offset))
            .div_ceil(u32::from(self.x_sampling))
    }
    pub fn count_lines(self, height: u32) -> u32 {
        height
            .saturating_sub(u32::from(self.y_offset))
            .div_ceil(u32::from(self.y_sampling))
    }
    pub const PASSES: [Self; 7] = {
        const fn new(x_sampling: u8, x_offset: u8, y_sampling: u8, y_offset: u8) -> PassConstants {
            PassConstants {
                x_sampling,
                x_offset,
                y_sampling,
                y_offset,
            }
        }
        [
            new(8, 0, 8, 0),
            new(8, 4, 8, 0),
            new(4, 0, 8, 4),
            new(4, 2, 4, 0),
            new(2, 0, 4, 2),
            new(2, 1, 2, 0),
            new(1, 0, 2, 1),
        ]
    };
}
#[derive(Clone)]
pub(crate) struct Adam7Iterator {
    line: u32,
    lines: u32,
    line_width: u32,
    current_pass: u8,
    width: u32,
    height: u32,
}
impl Adam7Iterator {
    pub fn new(width: u32, height: u32) -> Adam7Iterator {
        let mut this = Adam7Iterator {
            line: 0,
            lines: 0,
            line_width: 0,
            current_pass: 1,
            width,
            height,
        };
        this.init_pass();
        this
    }
    fn init_pass(&mut self) {
        let info = PassConstants::PASSES[self.current_pass as usize - 1];
        self.line_width = info.count_samples(self.width);
        self.lines = info.count_lines(self.height);
        self.line = 0;
    }
}
impl Iterator for Adam7Iterator {
    type Item = Adam7Info;
    fn next(&mut self) -> Option<Self::Item> {
        if self.line < self.lines && self.line_width > 0 {
            let this_line = self.line;
            self.line += 1;
            Some(Adam7Info {
                pass: self.current_pass,
                line: this_line,
                width: self.width,
                samples: self.line_width,
            })
        } else if self.current_pass < 7 {
            self.current_pass += 1;
            self.init_pass();
            self.next()
        } else {
            None
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Adam7Variant {
    #[default]
    Sparse,
    Splat,
}
fn subbyte_values<const N: usize>(
    scanline: &[u8],
    bit_pos: [u8; N],
    mask: u8,
) -> impl Iterator<Item = u8> + '_ {
    (scanline.iter().copied()).flat_map(move |value| bit_pos.map(|n| (value >> n) & mask))
}
fn expand_adam7_bits(
    row_stride_in_bytes: usize,
    info: &Adam7Info,
    bits_pp: u8,
) -> impl Iterator<Item = BitPostion> {
    debug_assert!(bits_pp == 1 || bits_pp == 2 || bits_pp == 4);
    let (line_mul, line_off, samp_mul, samp_off) = {
        let constants = info.pass_constants();
        (
            usize::from(constants.y_sampling),
            usize::from(constants.y_offset),
            u64::from(constants.x_sampling),
            u64::from(constants.x_offset),
        )
    };
    let prog_line = line_mul * info.line as usize + line_off;
    let byte_start = prog_line * row_stride_in_bytes;
    (0..u64::from(info.samples))
        .map(move |i| (i * samp_mul + samp_off) * u64::from(bits_pp))
        .map(move |i| BitPostion {
            byte: byte_start + (i / 8) as usize,
            bit: i as u8 % 8,
        })
}
fn expand_adam7_bytes(
    row_stride_in_bytes: usize,
    info: &Adam7Info,
    bytes_pp: u8,
) -> impl Iterator<Item = usize> {
    let (line_mul, line_off, samp_mul, samp_off) = {
        let constants = info.pass_constants();
        (
            usize::from(constants.y_sampling),
            usize::from(constants.y_offset),
            u64::from(constants.x_sampling),
            u64::from(constants.x_offset),
        )
    };
    let prog_line = line_mul * info.line as usize + line_off;
    let byte_start = prog_line * row_stride_in_bytes;
    (0..u64::from(info.samples))
        .map(move |i| (i * samp_mul + samp_off) * u64::from(bytes_pp))
        .map(move |i| i as usize + byte_start)
}
pub fn expand_pass(
    img: &mut [u8],
    img_row_stride: usize,
    interlaced_row: &[u8],
    interlace_info: &Adam7Info,
    bits_per_pixel: u8,
) {
    match bits_per_pixel {
        1 => {
            const BIT_POS_1: [u8; 8] = [7, 6, 5, 4, 3, 2, 1, 0];
            let bit_indices = expand_adam7_bits(img_row_stride, interlace_info, 1);
            for (pos, px) in bit_indices.zip(subbyte_values(interlaced_row, BIT_POS_1, 0b1)) {
                let shift = 8 - bits_per_pixel - pos.bit;
                img[pos.byte] |= px << shift;
            }
        }
        2 => {
            const BIT_POS_2: [u8; 4] = [6, 4, 2, 0];
            let bit_indices = expand_adam7_bits(img_row_stride, interlace_info, 2);
            for (pos, px) in bit_indices.zip(subbyte_values(interlaced_row, BIT_POS_2, 0b11)) {
                let shift = 8 - bits_per_pixel - pos.bit;
                img[pos.byte] |= px << shift;
            }
        }
        4 => {
            const BIT_POS_4: [u8; 2] = [4, 0];
            let bit_indices = expand_adam7_bits(img_row_stride, interlace_info, 4);
            for (pos, px) in bit_indices.zip(subbyte_values(interlaced_row, BIT_POS_4, 0b1111)) {
                let shift = 8 - bits_per_pixel - pos.bit;
                img[pos.byte] |= px << shift;
            }
        }
        8 => {
            let byte_indices = expand_adam7_bytes(img_row_stride, interlace_info, 1);
            for (bytepos, &px) in byte_indices.zip(interlaced_row) {
                img[bytepos] = px;
            }
        }
        16 => {
            let byte_indices = expand_adam7_bytes(img_row_stride, interlace_info, 2);
            for (bytepos, px) in byte_indices.zip(interlaced_row.chunks(2)) {
                img[bytepos..][..2].copy_from_slice(px);
            }
        }
        _ => {
            debug_assert!(bits_per_pixel % 8 == 0);
            let bytes_pp = bits_per_pixel / 8;
            let byte_indices = expand_adam7_bytes(img_row_stride, interlace_info, bytes_pp);
            for (bytepos, px) in byte_indices.zip(interlaced_row.chunks(bytes_pp.into())) {
                img[bytepos..][..px.len()].copy_from_slice(px);
            }
        }
    }
}
pub fn expand_pass_splat(
    img: &mut [u8],
    img_row_stride: usize,
    interlaced_row: &[u8],
    interlace_info: &Adam7Info,
    bits_per_pixel: u8,
) {
    fn expand_bits_to_img(
        img: &mut [u8],
        px: u8,
        mut pos: BitPostion,
        repeat: RangeTo<u8>,
        bpp: u8,
    ) {
        let (mut into, mut tail) = img[pos.byte..].split_first_mut().unwrap();
        let mask = (1u8 << bpp) - 1;
        for _ in 0..repeat.end {
            if pos.bit >= 8 {
                pos.byte += 1;
                pos.bit -= 8;
                (into, tail) = tail.split_first_mut().unwrap();
            }
            let shift = 8 - bpp - pos.bit;
            let pre = (*into >> shift) & mask;
            *into ^= (pre ^ px) << shift;
            pos.bit += bpp;
        }
    }
    let height = (img.len() / img_row_stride) as u32;
    let y_repeat = interlace_info.splat_line_repeat(height);
    debug_assert!(y_repeat > 0);
    match bits_per_pixel {
        1 => {
            const BIT_POS_1: [u8; 8] = [7, 6, 5, 4, 3, 2, 1, 0];
            for offset in 0..y_repeat {
                let bit_indices = expand_adam7_bits(img_row_stride, interlace_info, 1);
                let line_offset = usize::from(offset) * img_row_stride;
                for (idx, (mut pos, px)) in bit_indices
                    .zip(subbyte_values(interlaced_row, BIT_POS_1, 0b1))
                    .enumerate()
                {
                    let x_repeat = interlace_info.splat_pixel_repeat(idx);
                    debug_assert!(x_repeat > 0);
                    pos.byte += line_offset;
                    expand_bits_to_img(img, px, pos, ..x_repeat, bits_per_pixel);
                }
            }
        }
        2 => {
            const BIT_POS_2: [u8; 4] = [6, 4, 2, 0];
            for offset in 0..y_repeat {
                let bit_indices = expand_adam7_bits(img_row_stride, interlace_info, 2);
                let line_offset = usize::from(offset) * img_row_stride;
                for (idx, (mut pos, px)) in bit_indices
                    .zip(subbyte_values(interlaced_row, BIT_POS_2, 0b11))
                    .enumerate()
                {
                    let x_repeat = interlace_info.splat_pixel_repeat(idx);
                    pos.byte += line_offset;
                    expand_bits_to_img(img, px, pos, ..x_repeat, bits_per_pixel);
                }
            }
        }
        4 => {
            const BIT_POS_4: [u8; 2] = [4, 0];
            for offset in 0..y_repeat {
                let bit_indices = expand_adam7_bits(img_row_stride, interlace_info, 4);
                let line_offset = usize::from(offset) * img_row_stride;
                for (idx, (mut pos, px)) in bit_indices
                    .zip(subbyte_values(interlaced_row, BIT_POS_4, 0b1111))
                    .enumerate()
                {
                    let x_repeat = interlace_info.splat_pixel_repeat(idx);
                    pos.byte += line_offset;
                    expand_bits_to_img(img, px, pos, ..x_repeat, bits_per_pixel);
                }
            }
        }
        8 => {
            for offset in 0..y_repeat {
                let byte_indices = expand_adam7_bytes(img_row_stride, interlace_info, 1);
                let line_offset = usize::from(offset) * img_row_stride;
                for (idx, (bytepos, px)) in byte_indices.zip(interlaced_row).enumerate() {
                    let x_repeat = usize::from(interlace_info.splat_pixel_repeat(idx));
                    debug_assert!(x_repeat > 0);
                    img[line_offset + bytepos..][..x_repeat].fill(*px);
                }
            }
        }
        _ => {
            debug_assert!(bits_per_pixel % 8 == 0);
            let bytes = bits_per_pixel / 8;
            let chunk = usize::from(bytes);
            for offset in 0..y_repeat {
                let byte_indices = expand_adam7_bytes(img_row_stride, interlace_info, bytes);
                let line_offset = usize::from(offset) * img_row_stride;
                for (idx, (bytepos, px)) in byte_indices
                    .zip(interlaced_row.chunks_exact(chunk))
                    .enumerate()
                {
                    let x_repeat = usize::from(interlace_info.splat_pixel_repeat(idx));
                    let target = &mut img[line_offset + bytepos..][..chunk * x_repeat];
                    for target in target.chunks_exact_mut(chunk) {
                        target.copy_from_slice(px);
                    }
                }
            }
        }
    }
}
