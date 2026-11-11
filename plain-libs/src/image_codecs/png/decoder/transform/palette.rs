use super::{TransformFn, unpack_bits};
use crate::image_codecs::png::{BitDepth, Info};
pub fn create_expansion_into_rgb8(info: &Info) -> TransformFn {
    let rgba_palette = create_rgba_palette(info);
    if info.bit_depth == BitDepth::Eight {
        Box::new(move |input, output, _info| expand_8bit_into_rgb8(input, output, &rgba_palette))
    } else {
        Box::new(move |input, output, info| expand_into_rgb8(input, output, info, &rgba_palette))
    }
}
pub fn create_expansion_into_rgba8(info: &Info) -> TransformFn {
    let rgba_palette = create_rgba_palette(info);
    Box::new(move |input, output, info| {
        expand_paletted_into_rgba8(input, output, info, &rgba_palette)
    })
}
fn create_rgba_palette(info: &Info) -> [[u8; 4]; 256] {
    let palette = info.palette.as_deref().expect("Caller should verify");
    let trns = info.trns.as_deref().unwrap_or(&[]);
    let trns = if trns.len() <= palette.len() / 3 {
        trns
    } else {
        &[]
    };
    let mut rgba_palette = [[0, 0, 0, 0xFF]; 256];
    {
        let mut palette_iter = palette;
        let mut rgba_iter = &mut rgba_palette[..];
        while palette_iter.len() >= 4 {
            rgba_iter[0].copy_from_slice(&palette_iter[0..4]);
            palette_iter = &palette_iter[3..];
            rgba_iter = &mut rgba_iter[1..];
        }
        if !palette_iter.is_empty() {
            rgba_iter[0][0..3].copy_from_slice(&palette_iter[0..3]);
        }
    }
    for (alpha, rgba) in trns.iter().copied().zip(rgba_palette.iter_mut()) {
        rgba[3] = alpha;
    }
    for rgba in rgba_palette[trns.len()..(palette.len() / 3)].iter_mut() {
        rgba[3] = 0xFF;
    }
    rgba_palette
}
fn expand_8bit_into_rgb8(mut input: &[u8], mut output: &mut [u8], rgba_palette: &[[u8; 4]; 256]) {
    while output.len() >= 4 {
        let rgba = &rgba_palette[input[0] as usize];
        output[0..4].copy_from_slice(rgba);
        input = &input[1..];
        output = &mut output[3..];
    }
    if !output.is_empty() {
        let rgba = &rgba_palette[input[0] as usize];
        output[0..3].copy_from_slice(&rgba[0..3]);
    }
}
fn expand_into_rgb8(row: &[u8], buffer: &mut [u8], info: &Info, rgba_palette: &[[u8; 4]; 256]) {
    unpack_bits(row, buffer, 3, info.bit_depth as u8, |i, chunk| {
        let rgba = &rgba_palette[i as usize];
        chunk[0] = rgba[0];
        chunk[1] = rgba[1];
        chunk[2] = rgba[2];
    })
}
fn expand_paletted_into_rgba8(
    row: &[u8],
    buffer: &mut [u8],
    info: &Info,
    rgba_palette: &[[u8; 4]; 256],
) {
    unpack_bits(row, buffer, 4, info.bit_depth as u8, |i, chunk| {
        chunk.copy_from_slice(&rgba_palette[i as usize]);
    });
}
