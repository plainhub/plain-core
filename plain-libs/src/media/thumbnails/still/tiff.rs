use super::super::{
    admission,
    scale::Bitmap,
    sniff::{ImageKind, Sniffed},
};
use anyhow::{Result, bail, ensure};
use std::io::{Read, Seek};
use tiff::{
    ColorType,
    decoder::{Decoder, DecodingResult},
    tags::Tag,
};

pub(super) fn probe(reader: impl Read + Seek) -> Result<Sniffed> {
    let mut decoder = Decoder::new(reader)?;
    let (width, height) = decoder.dimensions()?;
    let color = decoder.colortype()?;
    let orientation = decoder
        .find_tag_unsigned::<u8>(Tag::Orientation)?
        .unwrap_or(1);
    let preserve_alpha = matches!(
        color,
        ColorType::GrayA(_) | ColorType::RGBA(_) | ColorType::CMYKA(_)
    );
    let (tile_w, tile_h) = decoder.chunk_dimensions();
    let jpeg = decoder
        .find_tag_unsigned::<u16>(Tag::Compression)?
        .unwrap_or(1)
        == 7;
    let compressed_bytes = if jpeg {
        let count_tag = match decoder.get_chunk_type() {
            tiff::decoder::ChunkType::Strip => Tag::StripByteCounts,
            tiff::decoder::ChunkType::Tile => Tag::TileByteCounts,
        };
        decoder
            .get_tag_u64_vec(count_tag)?
            .into_iter()
            .max()
            .unwrap_or(0)
            .checked_mul(2)
    } else {
        Some(0)
    };
    let work_bytes = if streamable(&mut decoder, color)? {
        let band_pixels = u64::from(width) * u64::from(tile_h.min(height));
        let chunk_pixels = u64::from(tile_w) * u64::from(tile_h);
        let input_channels = u64::from(color.num_samples());
        let output_channels = if preserve_alpha { 4_u64 } else { 3 };
        let row_sums = u64::from(width) * output_channels * 4;
        band_pixels
            .checked_mul(output_channels)
            .and_then(|band| chunk_pixels.checked_mul(input_channels)?.checked_add(band))
            .and_then(|pixels| pixels.checked_add(row_sums))
            .and_then(|pixels| {
                pixels.checked_add(if jpeg {
                    chunk_pixels.checked_mul(8)?
                } else {
                    0
                })
            })
    } else {
        (u64::from(width) * u64::from(height)).checked_mul(16)
    }
    .and_then(|pixels| pixels.checked_add(compressed_bytes?))
    .ok_or_else(|| anyhow::anyhow!("TIFF working memory estimate exceeds integer limits"))?;
    Ok(Sniffed {
        kind: ImageKind::Tiff,
        width,
        height,
        progressive: false,
        preserve_alpha,
        orientation,
        work_bytes,
    })
}

pub(super) fn decode(reader: impl Read + Seek) -> Result<Bitmap> {
    decode_decoder(Decoder::new(reader)?)
}

fn decode_decoder<R: Read + Seek>(mut decoder: Decoder<R>) -> Result<Bitmap> {
    let (width, height) = decoder.dimensions()?;
    ensure!(
        u64::from(width) * u64::from(height) <= admission::MAX_SOURCE_PIXELS,
        "TIFF exceeds pixel limit"
    );
    let color = decoder.colortype()?;
    let depth = color.bit_depth();
    let channels = usize::from(color.num_samples());
    let palette = if matches!(color, ColorType::Palette(_)) {
        Some(decoder.get_tag_u16_vec(Tag::ColorMap)?)
    } else {
        None
    };
    let ycbcr = if matches!(color, ColorType::YCbCr(_)) {
        Some(ycbcr_parameters(&mut decoder)?)
    } else {
        None
    };
    let premultiplied = decoder
        .find_tag_unsigned_vec::<u16>(Tag::ExtraSamples)?
        .is_some_and(|s| s.first() == Some(&1));
    let mut buffer = DecodingResult::U8(Vec::new());
    let layout = decoder.read_image_to_buffer(&mut buffer)?;
    if let Some(map) = palette.as_ref() {
        let n = 1_usize << depth;
        ensure!(map.len() == n * 3, "Invalid TIFF color map length");
        let indices: Vec<usize> = match &buffer {
            DecodingResult::U8(values) if depth < 8 => {
                let stride = (width as usize * depth as usize).div_ceil(8);
                let mask = (1_u8 << depth) - 1;
                let mut indices = Vec::with_capacity(width as usize * height as usize);
                for row in values.chunks_exact(stride).take(height as usize) {
                    for x in 0..width as usize {
                        let bit = x * depth as usize;
                        indices.push(
                            ((row[bit / 8] >> (8 - depth as usize - bit % 8)) & mask) as usize,
                        );
                    }
                }
                indices
            }
            DecodingResult::U8(v) => v.iter().map(|v| *v as usize).collect(),
            DecodingResult::U16(v) => v.iter().map(|v| *v as usize).collect(),
            _ => bail!("Unsupported TIFF palette indices"),
        };
        ensure!(
            indices.len() == width as usize * height as usize,
            "Incomplete TIFF palette pixels"
        );
        let mut output = Vec::with_capacity(indices.len() * 3);
        for index in indices {
            ensure!(index < n, "Invalid TIFF palette index");
            output.extend([
                (map[index] >> 8) as u8,
                (map[n + index] >> 8) as u8,
                (map[n * 2 + index] >> 8) as u8,
            ]);
        }
        return Ok(Bitmap::new_rgb(output, width, height));
    }
    let samples = match buffer {
        DecodingResult::U8(values) if depth < 8 => {
            ensure!(
                channels == 1 && matches!(depth, 1 | 2 | 4),
                "Unsupported packed TIFF layout"
            );
            let stride = (width as usize * depth as usize).div_ceil(8);
            let max = (1_u8 << depth) - 1;
            let mut samples = Vec::with_capacity(width as usize * height as usize);
            for row in values.chunks_exact(stride).take(height as usize) {
                for x in 0..width as usize {
                    let bit = x * depth as usize;
                    let v = (row[bit / 8] >> (8 - depth as usize - bit % 8)) & max;
                    samples.push((u16::from(v) * 255 / u16::from(max)) as u8);
                }
            }
            samples
        }
        DecodingResult::U8(v) => v,
        DecodingResult::U16(v) => v
            .into_iter()
            .map(|v| (u32::from(v) * 255 / ((1_u32 << depth.min(16)) - 1)) as u8)
            .collect(),
        DecodingResult::F16(v) => v.into_iter().map(|v| float(binary16_to_f32(v))).collect(),
        DecodingResult::F32(v) => v.into_iter().map(float).collect(),
        DecodingResult::F64(v) => v.into_iter().map(|v| float(v as f32)).collect(),
        DecodingResult::U32(v) => v.into_iter().map(|v| (v >> 24) as u8).collect(),
        DecodingResult::U64(v) => v.into_iter().map(|v| (v >> 56) as u8).collect(),
        DecodingResult::I16(v) => v
            .into_iter()
            .map(|v| ((v as u16 ^ 0x8000) >> 8) as u8)
            .collect(),
        DecodingResult::I32(v) => v
            .into_iter()
            .map(|v| ((v as u32 ^ 0x8000_0000) >> 24) as u8)
            .collect(),
        DecodingResult::I64(v) => v
            .into_iter()
            .map(|v| ((v as u64 ^ 0x8000_0000_0000_0000) >> 56) as u8)
            .collect(),
        DecodingResult::I8(v) => v.into_iter().map(|v| (i16::from(v) + 128) as u8).collect(),
    };
    let count = width as usize * height as usize;
    ensure!(
        samples.len() >= count * channels,
        "Incomplete TIFF sample buffer"
    );
    let alpha = matches!(
        color,
        ColorType::GrayA(_) | ColorType::RGBA(_) | ColorType::CMYKA(_)
    );
    let out_channels = if alpha { 4 } else { 3 };
    let mut out = Vec::with_capacity(count * out_channels);
    let planar = layout.planes > 1;
    for i in 0..count {
        let sample = |c: usize| {
            samples[if planar {
                c * count + i
            } else {
                i * channels + c
            }]
        };
        let (r, g, b, a) = match color {
            ColorType::RGB(_) => (sample(0), sample(1), sample(2), 255),
            ColorType::RGBA(_) => (sample(0), sample(1), sample(2), sample(3)),
            ColorType::Gray(_) => (sample(0), sample(0), sample(0), 255),
            ColorType::GrayA(_) => (sample(0), sample(0), sample(0), sample(1)),
            ColorType::CMYK(_) | ColorType::CMYKA(_) => {
                let a = if alpha { sample(4) } else { 255 };
                if premultiplied && a == 0 {
                    (0, 0, 0, 0)
                } else {
                    let component = |c| unassociate(sample(c), a, premultiplied);
                    let k = u16::from(255 - component(3));
                    let rgb = |c| ((u16::from(255 - component(c)) * k + 127) / 255) as u8;
                    (rgb(0), rgb(1), rgb(2), a)
                }
            }
            ColorType::YCbCr(_) => {
                let (coefficients, reference) = ycbcr.as_ref().unwrap();
                let [kr, kg, kb] = *coefficients;
                let y =
                    (f64::from(sample(0)) - reference[0]) * 255. / (reference[1] - reference[0]);
                let cb =
                    (f64::from(sample(1)) - reference[2]) * 127. / (reference[3] - reference[2]);
                let cr =
                    (f64::from(sample(2)) - reference[4]) * 127. / (reference[5] - reference[4]);
                let r = y + (2. - 2. * kr) * cr;
                let b = y + (2. - 2. * kb) * cb;
                let g = (y - kr * r - kb * b) / kg;
                let byte = |v: f64| v.round().clamp(0., 255.) as u8;
                (byte(r), byte(g), byte(b), 255)
            }
            _ => bail!("Unsupported TIFF color representation {color:?}"),
        };
        let unassociate = |v| {
            unassociate(
                v,
                a,
                premultiplied && !matches!(color, ColorType::CMYK(_) | ColorType::CMYKA(_)),
            )
        };
        out.extend_from_slice(&[unassociate(r), unassociate(g), unassociate(b)]);
        if alpha {
            out.push(a);
        }
    }
    if alpha {
        Ok(Bitmap::new_rgba(out, width, height))
    } else {
        Ok(Bitmap::new_rgb(out, width, height))
    }
}
fn ycbcr_parameters<R: Read + Seek>(decoder: &mut Decoder<R>) -> Result<([f64; 3], [f64; 6])> {
    fn values(value: tiff::decoder::ifd::Value) -> Result<Vec<f64>> {
        use tiff::decoder::ifd::Value;
        match value {
            Value::List(values_list) => {
                let mut out = Vec::new();
                for value in values_list {
                    out.extend(values(value)?);
                }
                Ok(out)
            }
            Value::Rational(n, d) => {
                ensure!(d != 0, "Invalid TIFF color rational");
                Ok(vec![f64::from(n) / f64::from(d)])
            }
            _ => bail!("Invalid TIFF color tag type"),
        }
    }
    let coefficients = match decoder.find_tag(Tag::Unknown(529))? {
        Some(v) => values(v)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid TIFF YCbCr coefficients"))?,
        None => [0.299, 0.587, 0.114],
    };
    let reference = match decoder.find_tag(Tag::Unknown(532))? {
        Some(v) => values(v)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid TIFF YCbCr reference"))?,
        None => [0., 255., 128., 255., 128., 255.],
    };
    ensure!(
        coefficients.iter().all(|v| v.is_finite() && *v > 0.)
            && (coefficients.iter().sum::<f64>() - 1.).abs() < 0.0001,
        "Invalid TIFF YCbCr coefficients"
    );
    ensure!(
        reference.chunks_exact(2).all(|p| p[1] > p[0]),
        "Invalid TIFF YCbCr reference"
    );
    Ok((coefficients, reference))
}

fn unassociate(value: u8, alpha: u8, premultiplied: bool) -> u8 {
    if !premultiplied {
        value
    } else if alpha == 0 {
        0
    } else {
        ((u16::from(value) * 255 + u16::from(alpha) / 2) / u16::from(alpha)).min(255) as u8
    }
}

fn float(v: f32) -> u8 {
    (v.clamp(0., 1.) * 255. + 0.5) as u8
}

fn streamable<R: Read + Seek>(decoder: &mut Decoder<R>, color: ColorType) -> Result<bool> {
    Ok(matches!(
        color,
        ColorType::RGB(8) | ColorType::RGBA(8) | ColorType::Gray(8) | ColorType::GrayA(8)
    ) && decoder
        .find_tag_unsigned::<u16>(Tag::PlanarConfiguration)?
        .unwrap_or(1)
        == 1)
}

pub(super) fn thumbnail<R: Read + Seek>(reader: R, tw: u32, th: u32) -> Result<Bitmap> {
    let mut decoder = Decoder::new(reader)?;
    let color = decoder.colortype()?;
    if !streamable(&mut decoder, color)? {
        return decode_decoder(decoder);
    }
    let (width, height) = decoder.dimensions()?;
    let alpha = matches!(color, ColorType::RGBA(_) | ColorType::GrayA(_));
    if alpha {
        stream::<_, 4>(&mut decoder, color, width, height, tw, th)
    } else {
        stream::<_, 3>(&mut decoder, color, width, height, tw, th)
    }
}

fn stream<R: Read + Seek, const C: usize>(
    decoder: &mut Decoder<R>,
    color: ColorType,
    width: u32,
    height: u32,
    tw: u32,
    th: u32,
) -> Result<Bitmap> {
    ensure!(
        u64::from(width) * u64::from(height) <= admission::MAX_SOURCE_PIXELS,
        "TIFF exceeds pixel limit"
    );
    let (tile_w, tile_h) = decoder.chunk_dimensions();
    let tile_w = tile_w.min(width);
    let tile_h = tile_h.min(height);
    ensure!(tile_w > 0 && tile_h > 0, "Invalid TIFF chunk dimensions");
    let across = width.div_ceil(tile_w);
    let channels = usize::from(color.num_samples());
    let premultiplied = decoder
        .find_tag_unsigned_vec::<u16>(Tag::ExtraSamples)?
        .is_some_and(|s| s.first() == Some(&1));
    let mut rows = super::rows::Rows::<C>::new(width, height, tw, th);
    let mut chunk = Vec::new();
    let mut band = Vec::new();
    for ty in 0..height.div_ceil(tile_h) {
        let y0 = ty * tile_h;
        let y1 = height.min((ty + 1) * tile_h);
        band.resize((y1 - y0) as usize * width as usize * C, 0);
        for tx in 0..width.div_ceil(tile_w) {
            let index = ty * across + tx;
            let layout = decoder.image_chunk_buffer_layout(index)?;
            chunk.resize(layout.len, 0);
            decoder.read_chunk_bytes(index, &mut chunk)?;
            let stride = layout
                .row_stride
                .ok_or_else(|| anyhow::anyhow!("Missing TIFF chunk stride"))?
                .get();
            let x0 = tx * tile_w;
            let x1 = width.min((tx + 1) * tile_w);
            for y in y0..y1 {
                let start = (y - ty * tile_h) as usize * stride;
                let input = &chunk[start..start + (x1 - x0) as usize * channels];
                let start = ((y - y0) as usize * width as usize + x0 as usize) * C;
                let output = &mut band[start..start + (x1 - x0) as usize * C];
                if channels == C && !premultiplied {
                    output.copy_from_slice(input);
                } else {
                    for (src, dst) in input.chunks_exact(channels).zip(output.chunks_exact_mut(C)) {
                        let (r, g, b, a) = if channels <= 2 {
                            (src[0], src[0], src[0], if C == 4 { src[1] } else { 255 })
                        } else {
                            (src[0], src[1], src[2], src[3])
                        };
                        let unassociate = |v| unassociate(v, a, premultiplied);
                        dst[..3].copy_from_slice(&[unassociate(r), unassociate(g), unassociate(b)]);
                        if C == 4 {
                            dst[3] = a;
                        }
                    }
                }
            }
        }
        for row in band.chunks_exact(width as usize * C) {
            rows.push(row);
        }
    }
    let (data, w, h) = rows.finish();
    if C == 4 {
        Ok(Bitmap::new_rgba(data, w, h))
    } else {
        Ok(Bitmap::new_rgb(data, w, h))
    }
}

// Scalar widening adapted from half 2.7.1 (half/SOURCE.json).
fn binary16_to_f32(value: u16) -> f32 {
    let sign = u32::from(value & 0x8000) << 16;
    let exponent = (value >> 10) & 0x1f;
    let mantissa = u32::from(value & 0x03ff);
    let bits = match (exponent, mantissa) {
        (0, 0) => sign,
        (0, _) => {
            let shift = mantissa.leading_zeros() - 21;
            let exponent = (113 - shift) << 23;
            sign | exponent | (((mantissa << shift) & 0x03ff) << 13)
        }
        (31, 0) => sign | 0x7f80_0000,
        (31, _) => sign | 0x7fc0_0000 | (mantissa << 13),
        _ => sign | ((u32::from(exponent) + 112) << 23) | (mantissa << 13),
    };
    f32::from_bits(bits)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/media/thumbnails/still/tiff.rs"]
mod tests;
