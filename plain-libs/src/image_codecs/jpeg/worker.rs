use crate::image_codecs::jpeg::color_convert::ycbcr_to_grayscale;
use crate::image_codecs::jpeg::components::{Components, SampleRatios};
use crate::image_codecs::jpeg::decoder::{ColorConvert16Ptr, MAX_COMPONENTS};
use crate::image_codecs::jpeg::errors::DecodeErrors;
use crate::image_codecs::zune::colorspace::ColorSpace;
use core::cmp::min;
use core::convert::TryInto;
use std::format;
#[inline]
fn blinn_8x8(in_val: u8, y: u8) -> u8 {
    let t = i32::from(in_val) * i32::from(y) + 128;
    return ((t + (t >> 8)) >> 8) as u8;
}
pub(crate) fn color_convert(
    unprocessed: &[&[i16]; MAX_COMPONENTS],
    color_convert_16: ColorConvert16Ptr,
    input_colorspace: ColorSpace,
    output_colorspace: ColorSpace,
    output: &mut [u8],
    width: usize,
    padded_width: usize,
) -> Result<(), DecodeErrors> {
    if input_colorspace.num_components() == 3 && input_colorspace == output_colorspace {
        copy_removing_padding(unprocessed, width, padded_width, output);
        return Ok(());
    }
    if input_colorspace.num_components() == 4 && input_colorspace == output_colorspace {
        copy_removing_padding_4x(unprocessed, width, padded_width, output);
        return Ok(());
    }
    match (input_colorspace, output_colorspace) {
        (ColorSpace::YCbCr | ColorSpace::Luma, ColorSpace::Luma) => {
            ycbcr_to_grayscale(unprocessed[0], width, padded_width, output);
        }
        (
            ColorSpace::YCbCr,
            ColorSpace::RGB | ColorSpace::RGBA | ColorSpace::BGR | ColorSpace::BGRA,
        ) => {
            color_convert_ycbcr(
                unprocessed,
                width,
                padded_width,
                output_colorspace,
                color_convert_16,
                output,
            );
        }
        (ColorSpace::YCCK, ColorSpace::RGB) => {
            color_convert_ycck_to_rgb::<3>(
                unprocessed,
                width,
                padded_width,
                output_colorspace,
                color_convert_16,
                output,
            );
        }
        (ColorSpace::YCCK, ColorSpace::RGBA) => {
            color_convert_ycck_to_rgb::<4>(
                unprocessed,
                width,
                padded_width,
                output_colorspace,
                color_convert_16,
                output,
            );
        }
        (ColorSpace::CMYK, ColorSpace::RGB) => {
            color_convert_cymk_to_rgb::<3>(unprocessed, width, padded_width, output);
        }
        (ColorSpace::CMYK, ColorSpace::RGBA) => {
            color_convert_cymk_to_rgb::<4>(unprocessed, width, padded_width, output);
        }
        (ColorSpace::MultiBand(n), _) => {
            if n.get() != 2 {
                return Err(DecodeErrors::Format(format!(
                    "Unknown multiband sample ({n}), please share sample"
                )));
            }
            copy_removing_padding_generic(
                unprocessed,
                width,
                padded_width,
                output,
                n.get() as usize,
            );
        }
        (ColorSpace::Luma, ColorSpace::RGB) => {
            convert_luma_to_rgb(unprocessed, width, padded_width, output)
        }
        (ColorSpace::Luma, ColorSpace::RGBA) => {
            convert_luma_to_rgba(unprocessed, width, padded_width, output)
        }
        _ => {
            let msg = format!(
                "Unimplemented colorspace mapping from {input_colorspace:?} to {output_colorspace:?}"
            );
            return Err(DecodeErrors::Format(msg));
        }
    }
    Ok(())
}
fn convert_luma_to_rgb(
    mcu_block: &[&[i16]; MAX_COMPONENTS],
    width: usize,
    padded_width: usize,
    output: &mut [u8],
) {
    for (pix_w, y_w) in output
        .chunks_exact_mut(width * 3)
        .zip(mcu_block[0].chunks_exact(padded_width))
    {
        for (pix, c) in pix_w.chunks_exact_mut(3).zip(y_w) {
            pix[0] = *c as u8;
            pix[1] = *c as u8;
            pix[2] = *c as u8;
        }
    }
}
fn convert_luma_to_rgba(
    mcu_block: &[&[i16]; MAX_COMPONENTS],
    width: usize,
    padded_width: usize,
    output: &mut [u8],
) {
    for (pix_w, y_w) in output
        .chunks_exact_mut(width * 4)
        .zip(mcu_block[0].chunks_exact(padded_width))
    {
        for (pix, c) in pix_w.chunks_exact_mut(4).zip(y_w) {
            pix[0] = *c as u8;
            pix[1] = *c as u8;
            pix[2] = *c as u8;
            pix[3] = 255;
        }
    }
}
fn copy_removing_padding(
    mcu_block: &[&[i16]; MAX_COMPONENTS],
    width: usize,
    padded_width: usize,
    output: &mut [u8],
) {
    for (((pix_w, c_w), m_w), y_w) in output
        .chunks_exact_mut(width * 3)
        .zip(mcu_block[0].chunks_exact(padded_width))
        .zip(mcu_block[1].chunks_exact(padded_width))
        .zip(mcu_block[2].chunks_exact(padded_width))
    {
        for (((pix, c), y), m) in pix_w.chunks_exact_mut(3).zip(c_w).zip(m_w).zip(y_w) {
            pix[0] = *c as u8;
            pix[1] = *y as u8;
            pix[2] = *m as u8;
        }
    }
}
fn copy_removing_padding_4x(
    mcu_block: &[&[i16]; MAX_COMPONENTS],
    width: usize,
    padded_width: usize,
    output: &mut [u8],
) {
    for ((((pix_w, c_w), m_w), y_w), k_w) in output
        .chunks_exact_mut(width * 4)
        .zip(mcu_block[0].chunks_exact(padded_width))
        .zip(mcu_block[1].chunks_exact(padded_width))
        .zip(mcu_block[2].chunks_exact(padded_width))
        .zip(mcu_block[3].chunks_exact(padded_width))
    {
        for ((((pix, c), y), m), k) in pix_w
            .chunks_exact_mut(4)
            .zip(c_w)
            .zip(m_w)
            .zip(y_w)
            .zip(k_w)
        {
            pix[0] = *c as u8;
            pix[1] = *y as u8;
            pix[2] = *m as u8;
            pix[3] = *k as u8;
        }
    }
}
fn copy_removing_padding_generic(
    mcu_block: &[&[i16]; MAX_COMPONENTS],
    width: usize,
    padded_width: usize,
    output: &mut [u8],
    channels: usize,
) {
    match channels {
        2 => {
            for ((pix_w, y_w), k_w) in output
                .chunks_exact_mut(width * channels)
                .zip(mcu_block[0].chunks_exact(padded_width))
                .zip(mcu_block[1].chunks_exact(padded_width))
            {
                for ((pix, c), k) in pix_w.chunks_exact_mut(2).zip(y_w).zip(k_w) {
                    pix[0] = *c as u8;
                    pix[1] = *k as u8;
                }
            }
        }
        _ => unreachable!(),
    }
}
fn color_convert_ycck_to_rgb<const NUM_COMPONENTS: usize>(
    mcu_block: &[&[i16]; MAX_COMPONENTS],
    width: usize,
    padded_width: usize,
    output_colorspace: ColorSpace,
    color_convert_16: ColorConvert16Ptr,
    output: &mut [u8],
) {
    color_convert_ycbcr(
        mcu_block,
        width,
        padded_width,
        output_colorspace,
        color_convert_16,
        output,
    );
    for (pix_w, m_w) in output
        .chunks_exact_mut(width * 3)
        .zip(mcu_block[3].chunks_exact(padded_width))
    {
        for (pix, m) in pix_w.chunks_exact_mut(NUM_COMPONENTS).zip(m_w) {
            let m = (*m) as u8;
            pix[0] = blinn_8x8(255 - pix[0], m);
            pix[1] = blinn_8x8(255 - pix[1], m);
            pix[2] = blinn_8x8(255 - pix[2], m);
        }
    }
}
fn color_convert_cymk_to_rgb<const NUM_COMPONENTS: usize>(
    mcu_block: &[&[i16]; MAX_COMPONENTS],
    width: usize,
    padded_width: usize,
    output: &mut [u8],
) {
    for ((((pix_w, c_w), m_w), y_w), k_w) in output
        .chunks_exact_mut(width * NUM_COMPONENTS)
        .zip(mcu_block[0].chunks_exact(padded_width))
        .zip(mcu_block[1].chunks_exact(padded_width))
        .zip(mcu_block[2].chunks_exact(padded_width))
        .zip(mcu_block[3].chunks_exact(padded_width))
    {
        for ((((pix, c), m), y), k) in pix_w
            .chunks_exact_mut(3)
            .zip(c_w)
            .zip(m_w)
            .zip(y_w)
            .zip(k_w)
        {
            let c = *c as u8;
            let m = *m as u8;
            let y = *y as u8;
            let k = *k as u8;
            pix[0] = blinn_8x8(c, k);
            pix[1] = blinn_8x8(m, k);
            pix[2] = blinn_8x8(y, k);
        }
    }
}
fn color_convert_ycbcr(
    mcu_block: &[&[i16]; MAX_COMPONENTS],
    width: usize,
    padded_width: usize,
    output_colorspace: ColorSpace,
    color_convert_16: ColorConvert16Ptr,
    output: &mut [u8],
) {
    let num_components = output_colorspace.num_components();
    let stride = width * num_components;
    let mut temp = [0; 64];
    for (((y_width, cb_width), cr_width), out) in mcu_block[0]
        .chunks_exact(padded_width)
        .zip(mcu_block[1].chunks_exact(padded_width))
        .zip(mcu_block[2].chunks_exact(padded_width))
        .zip(output.chunks_exact_mut(stride))
    {
        if width < 16 {
            let mut y_out = [0; 16];
            let mut cb_out = [0; 16];
            let mut cr_out = [0; 16];
            y_out[0..min(y_width.len(), 16)].copy_from_slice(&y_width[0..min(y_width.len(), 16)]);
            cb_out[0..min(cb_width.len(), 16)]
                .copy_from_slice(&cb_width[0..min(cb_width.len(), 16)]);
            cr_out[0..min(cr_width.len(), 16)]
                .copy_from_slice(&cr_width[0..min(cr_width.len(), 16)]);
            (color_convert_16)(&y_out, &cb_out, &cr_out, &mut temp, &mut 0);
            out[0..width * num_components].copy_from_slice(&temp[0..width * num_components]);
            continue;
        }
        for (((y, cb), cr), out_c) in y_width
            .chunks_exact(16)
            .zip(cb_width.chunks_exact(16))
            .zip(cr_width.chunks_exact(16))
            .zip(out.chunks_exact_mut(16 * num_components))
        {
            (color_convert_16)(
                y.try_into().unwrap(),
                cb.try_into().unwrap(),
                cr.try_into().unwrap(),
                out_c,
                &mut 0,
            );
        }
        for ((y, cb), cr) in y_width[width - 16..]
            .chunks_exact(16)
            .zip(cb_width[width - 16..].chunks_exact(16))
            .zip(cr_width[width - 16..].chunks_exact(16))
            .take(1)
        {
            (color_convert_16)(
                y.try_into().unwrap(),
                cb.try_into().unwrap(),
                cr.try_into().unwrap(),
                &mut temp,
                &mut 0,
            );
        }
        let rem = out[(width - 16) * num_components..]
            .chunks_exact_mut(16 * num_components)
            .next()
            .unwrap();
        rem.copy_from_slice(&temp[0..rem.len()]);
    }
}
pub(crate) fn upsample(
    component: &mut Components,
    mcu_height: usize,
    i: usize,
    upsampler_scratch_space: &mut [i16],
    has_vertical_sample: bool,
) -> Result<(), DecodeErrors> {
    match component.sample_ratio {
        SampleRatios::V | SampleRatios::HV => {
            let mut dest_start = 0;
            let stride_bytes_written = component.width_stride * component.sample_ratio.sample();
            if i > 0 {
                let stride = component.width_stride;
                let dest = &mut component.first_row_upsample_dest[0..stride_bytes_written];
                let row = &component.row[..];
                let row_up = &component.row_up[..];
                let row_down = &component.raw_coeff[0..stride];
                (component.up_sampler)(row, row_up, row_down, upsampler_scratch_space, dest);
            }
            let mut upsample = true;
            let stride = component.width_stride * component.vertical_sample;
            let stop_offset = component.raw_coeff.len() / component.width_stride;
            if component.raw_coeff.len() != stop_offset * stride {
                return Err(DecodeErrors::FormatStatic(
                    "Invalid component dimensions, would panic",
                ));
            }
            for (pos, curr_row) in component
                .raw_coeff
                .chunks_exact(component.width_stride)
                .enumerate()
            {
                let mut dest: &mut [i16] = &mut [];
                let mut row_up: &[i16] = &[];
                let mut row_down: &[i16] = &[];
                if i == 0 && pos == 0 {
                    row_up = &component.raw_coeff[pos * stride..(pos + 1) * stride];
                    row_down = &component.raw_coeff[(pos + 1) * stride..(pos + 2) * stride];
                } else if i > 0 && pos == 0 {
                    row_up = &component.row[..];
                    row_down = &component.raw_coeff[(pos + 1) * stride..(pos + 2) * stride];
                } else if i == mcu_height.saturating_sub(1) && pos == stop_offset - 1 {
                    row_up = &component.raw_coeff[(pos - 1) * stride..pos * stride];
                    row_down = &component.raw_coeff[pos * stride..(pos + 1) * stride];
                } else if pos > 0 && pos < stop_offset - 1 {
                    row_up = &component.raw_coeff[(pos - 1) * stride..pos * stride];
                    row_down = &component.raw_coeff[(pos + 1) * stride..(pos + 2) * stride];
                } else if pos == stop_offset - 1 {
                    let prev_row = &component.raw_coeff[(pos - 1) * stride..pos * stride];
                    component.row_up.copy_from_slice(prev_row);
                    component.row.copy_from_slice(curr_row);
                    upsample = false;
                } else {
                    unreachable!("Uh oh!");
                }
                if upsample {
                    dest =
                        &mut component.upsample_dest[dest_start..dest_start + stride_bytes_written];
                    dest_start += stride_bytes_written;
                }
                if upsample {
                    (component.up_sampler)(
                        curr_row,
                        row_up,
                        row_down,
                        upsampler_scratch_space,
                        dest,
                    );
                }
            }
        }
        SampleRatios::H => {
            component
                .upsample_dest
                .resize(component.raw_coeff.len() * 2, 0);
            let raw_coeff = &component.raw_coeff;
            let dest_coeff = &mut component.upsample_dest;
            if has_vertical_sample {
                let length = component.first_row_upsample_dest.len();
                component
                    .first_row_upsample_dest
                    .copy_from_slice(&dest_coeff.rchunks_exact(length).next().unwrap());
            }
            for (single_row, output_stride) in raw_coeff
                .chunks_exact(component.width_stride)
                .zip(dest_coeff.chunks_exact_mut(component.width_stride * 2))
            {
                (component.up_sampler)(single_row, &[], &[], &mut [], output_stride);
            }
        }
        SampleRatios::Generic(h, v) => {
            let raw_coeff = &component.raw_coeff;
            let dest_coeff = &mut component.upsample_dest;
            for (single_row, output_stride) in raw_coeff
                .chunks_exact(component.width_stride)
                .zip(dest_coeff.chunks_exact_mut(component.width_stride * h * v))
            {
                for row in output_stride.chunks_exact_mut(component.width_stride * h) {
                    (component.up_sampler)(single_row, &[], &[], &mut [], row);
                }
            }
        }
        SampleRatios::None => {}
    };
    Ok(())
}
