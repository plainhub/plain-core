mod palette;
use super::stream::FormatErrorInner;
use crate::image_codecs::png::{BitDepth, ColorType, DecodingError, Info, Transformations};
pub type TransformFn = Box<dyn Fn(&[u8], &mut [u8], &Info) + Send + Sync>;
pub fn create_transform_fn(
    info: &Info,
    transform: Transformations,
) -> Result<TransformFn, DecodingError> {
    let color_type = info.color_type;
    let bit_depth = info.bit_depth as u8;
    let trns = info.trns.is_some() || transform.contains(Transformations::ALPHA);
    let expand =
        transform.contains(Transformations::EXPAND) || transform.contains(Transformations::ALPHA);
    let strip16 = bit_depth == 16 && transform.contains(Transformations::STRIP_16);
    match color_type {
        ColorType::Indexed if expand => {
            if info.palette.is_none() {
                Err(DecodingError::Format(
                    FormatErrorInner::PaletteRequired.into(),
                ))
            } else if let BitDepth::Sixteen = info.bit_depth {
                Err(DecodingError::Format(
                    FormatErrorInner::InvalidColorBitDepth {
                        color_type: ColorType::Indexed,
                        bit_depth: BitDepth::Sixteen,
                    }
                    .into(),
                ))
            } else {
                Ok(if trns {
                    palette::create_expansion_into_rgba8(info)
                } else {
                    palette::create_expansion_into_rgb8(info)
                })
            }
        }
        ColorType::Grayscale | ColorType::GrayscaleAlpha if bit_depth < 8 && expand => {
            Ok(Box::new(if trns {
                expand_gray_u8_with_trns
            } else {
                expand_gray_u8
            }))
        }
        ColorType::Grayscale | ColorType::Rgb if expand && trns => {
            Ok(Box::new(if bit_depth == 8 {
                expand_trns_line
            } else if strip16 {
                expand_trns_and_strip_line16
            } else {
                assert_eq!(bit_depth, 16);
                expand_trns_line16
            }))
        }
        ColorType::Grayscale | ColorType::GrayscaleAlpha | ColorType::Rgb | ColorType::Rgba
            if strip16 =>
        {
            Ok(Box::new(transform_row_strip16))
        }
        _ => Ok(Box::new(copy_row)),
    }
}
fn copy_row(row: &[u8], output_buffer: &mut [u8], _: &Info) {
    output_buffer.copy_from_slice(row);
}
fn transform_row_strip16(row: &[u8], output_buffer: &mut [u8], _: &Info) {
    for i in 0..row.len() / 2 {
        output_buffer[i] = row[2 * i];
    }
}
#[inline(always)]
fn unpack_bits<F>(input: &[u8], output: &mut [u8], channels: usize, bit_depth: u8, func: F)
where
    F: Fn(u8, &mut [u8]),
{
    assert!(matches!(bit_depth, 1 | 2 | 4 | 8));
    assert!((8 / bit_depth as usize * channels).saturating_mul(input.len()) >= output.len());
    let mut buf_chunks = output.chunks_exact_mut(channels);
    let mut iter = input.iter();
    if bit_depth == 8 {
        for (&curr, chunk) in iter.zip(&mut buf_chunks) {
            func(curr, chunk);
        }
    } else {
        let mask = ((1u16 << bit_depth) - 1) as u8;
        let mut shift = -1;
        let mut curr = 0;
        for chunk in buf_chunks {
            if shift < 0 {
                shift = 8 - bit_depth as i32;
                curr = *iter.next().expect("input for unpack bits is not empty");
            }
            let pixel = (curr >> shift) & mask;
            func(pixel, chunk);
            shift -= bit_depth as i32;
        }
    }
}
fn expand_trns_line(input: &[u8], output: &mut [u8], info: &Info) {
    let channels = info.color_type.samples();
    let trns = info.trns.as_deref();
    for (input, output) in input
        .chunks_exact(channels)
        .zip(output.chunks_exact_mut(channels + 1))
    {
        output[..channels].copy_from_slice(input);
        output[channels] = if Some(input) == trns { 0 } else { 0xFF };
    }
}
fn expand_trns_line16(input: &[u8], output: &mut [u8], info: &Info) {
    let channels = info.color_type.samples();
    let trns = info.trns.as_deref();
    for (input, output) in input
        .chunks_exact(channels * 2)
        .zip(output.chunks_exact_mut(channels * 2 + 2))
    {
        output[..channels * 2].copy_from_slice(input);
        if Some(input) == trns {
            output[channels * 2] = 0;
            output[channels * 2 + 1] = 0
        } else {
            output[channels * 2] = 0xFF;
            output[channels * 2 + 1] = 0xFF
        };
    }
}
fn expand_trns_and_strip_line16(input: &[u8], output: &mut [u8], info: &Info) {
    let channels = info.color_type.samples();
    let trns = info.trns.as_deref();
    for (input, output) in input
        .chunks_exact(channels * 2)
        .zip(output.chunks_exact_mut(channels + 1))
    {
        for i in 0..channels {
            output[i] = input[i * 2];
        }
        output[channels] = if Some(input) == trns { 0 } else { 0xFF };
    }
}
fn expand_gray_u8(row: &[u8], buffer: &mut [u8], info: &Info) {
    let scaling_factor = (255) / ((1u16 << info.bit_depth as u8) - 1) as u8;
    unpack_bits(row, buffer, 1, info.bit_depth as u8, |val, chunk| {
        chunk[0] = val * scaling_factor
    });
}
fn expand_gray_u8_with_trns(row: &[u8], buffer: &mut [u8], info: &Info) {
    let scaling_factor = (255) / ((1u16 << info.bit_depth as u8) - 1) as u8;
    let trns = info.trns.as_deref();
    unpack_bits(row, buffer, 2, info.bit_depth as u8, |pixel, chunk| {
        chunk[1] = if let Some(trns) = trns {
            if pixel == trns[0] { 0 } else { 0xFF }
        } else {
            0xFF
        };
        chunk[0] = pixel * scaling_factor
    });
}
