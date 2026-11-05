use super::super::{
    admission,
    scale::Bitmap,
    sniff::{ImageKind, Sniffed},
};
use anyhow::{Result, bail, ensure};
use heic_rs::{
    context::Context,
    hevc::{ChromaFormat, Frame},
    props::{
        ItemProps,
        colr::{MatrixCoefficients, Nclx, Range},
    },
};
use std::{
    io::{Read, Seek, SeekFrom},
    path::Path,
};

const MAX_METADATA_BYTES: u64 = 16 * 1024 * 1024;

fn metadata(reader: &mut (impl Read + Seek)) -> Result<Vec<u8>> {
    let size = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(0))?;
    let mut output = Vec::new();
    let mut position = 0_u64;
    let mut found = 0_u8;
    while position < size {
        let mut header = [0; 8];
        reader.read_exact(&mut header)?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap());
        let mut box_header = header.to_vec();
        let length = match length {
            0 => size - position,
            1 => {
                let mut wide = [0; 8];
                reader.read_exact(&mut wide)?;
                box_header.extend_from_slice(&wide);
                u64::from_be_bytes(wide)
            }
            length => u64::from(length),
        };
        ensure!(
            length >= box_header.len() as u64 && length <= size - position,
            "Invalid image container box size"
        );
        if matches!(&header[4..], b"ftyp" | b"meta") {
            ensure!(
                output.len() as u64 + length <= MAX_METADATA_BYTES,
                "Image container metadata exceeds limit"
            );
            found |= if &header[4..] == b"ftyp" { 1 } else { 2 };
            output.extend_from_slice(&box_header);
            let start = output.len();
            output.resize(start + (length - box_header.len() as u64) as usize, 0);
            reader.read_exact(&mut output[start..])?;
            if found == 3 {
                return Ok(output);
            }
        }
        position += length;
        reader.seek(SeekFrom::Start(position))?;
    }
    bail!("Missing image container metadata")
}

pub(super) fn probe(path: &Path, kind: ImageKind) -> Result<Sniffed> {
    let bytes = metadata(&mut std::io::BufReader::new(std::fs::File::open(path)?))?;
    let ctx = Context::open(&bytes)?;
    let id = ctx.meta.primary;
    let p = ctx.props(id)?;
    let (w, h) = ctx.coded_size(id, &p)?;
    let simple = orientation(&p);
    let (width, height) = if simple.is_some() {
        (w, h)
    } else {
        heic_rs::transform::transformed_size(w, h, &p.transforms)?
    };
    let alpha = ctx.alpha_item(id)?.is_some();
    let canvas_pixels = u64::from(w) * u64::from(h);
    let mut coded_pixels = 0_u64;
    for item in &ctx.meta.items {
        if !matches!(&item.item_type, b"hvc1" | b"hev1" | b"av01") {
            continue;
        }
        let properties = ctx.props(item.id)?;
        let mut item_pixels = properties
            .ispe
            .map_or(0, |size| u64::from(size.width) * u64::from(size.height));
        if let Some(config) = properties.hvcc {
            item_pixels = item_pixels.max(heic_rs::hevc::coded_pixels(&config.parameter_sets())?);
        }
        coded_pixels = coded_pixels
            .checked_add(item_pixels)
            .ok_or_else(|| anyhow::anyhow!("Coded image pixel count overflow"))?;
    }
    ensure!(
        coded_pixels.max(canvas_pixels) <= admission::MAX_SOURCE_PIXELS,
        "Coded image exceeds pixel limit"
    );
    let work_bytes = coded_pixels.max(canvas_pixels) * 16;
    Ok(Sniffed {
        kind,
        width,
        height,
        progressive: false,
        preserve_alpha: alpha,
        orientation: simple.unwrap_or(1),
        work_bytes,
    })
}

fn coded(ctx: &Context<'_>, id: u32, p: &ItemProps<'_>) -> heic_rs::Result<Frame> {
    if ctx.meta.props.unknown_essential(id).is_some() {
        return Err(heic_rs::Error::Unsupported(
            "Unknown essential image property",
        ));
    }
    let item = ctx.meta.item(id).ok_or(heic_rs::Error::MissingItem(id))?;
    if &item.item_type != b"av01" {
        return heic_rs::decode::decode_coded(ctx, id, p);
    }
    let config = p.av1c.ok_or(heic_rs::Error::MissingBox("av1C"))?;
    let data = ctx.item_data(id)?;
    let mut input = Vec::with_capacity(config.len() - 4 + data.len());
    input.extend_from_slice(&config[4..]);
    input.extend_from_slice(&data);
    let picture = rav1d::still::decode(input, {
        let s = p.ispe.ok_or(heic_rs::Error::MissingBox("ispe"))?;
        u64::from(s.width)
            .saturating_mul(u64::from(s.height))
            .min(admission::MAX_SOURCE_PIXELS) as u32
    })
    .map_err(|_| heic_rs::Error::Malformed("Invalid AV1 image bitstream"))?;
    if p.ispe
        .is_some_and(|s| (s.width, s.height) != (picture.width, picture.height))
    {
        return Err(heic_rs::Error::Malformed(
            "AV1 dimensions disagree with ispe",
        ));
    }
    let chroma = match picture.chroma {
        0 => ChromaFormat::Monochrome,
        1 => ChromaFormat::Yuv420,
        2 => ChromaFormat::Yuv422,
        3 => ChromaFormat::Yuv444,
        _ => return Err(heic_rs::Error::Malformed("Invalid AV1 chroma layout")),
    };
    let color = Nclx {
        primaries: picture.primaries,
        transfer: picture.transfer,
        matrix: MatrixCoefficients::from_code(picture.matrix),
        matrix_code: picture.matrix,
        range: if picture.full_range {
            Range::Full
        } else {
            Range::Limited
        },
    };
    let [y, cb, cr] = picture.planes;
    Ok(Frame {
        width: picture.width,
        height: picture.height,
        bit_depth: picture.bit_depth,
        chroma,
        y,
        cb,
        cr,
        y_stride: picture.width,
        c_stride: chroma.chroma_size(picture.width, picture.height).0,
        color: Some(color),
    })
}

pub(super) fn decode(bytes: &[u8]) -> Result<Bitmap> {
    let ctx = Context::open(bytes)?;
    decode_primary(&ctx, true, &coded)
}

fn orientation(properties: &ItemProps<'_>) -> Option<u8> {
    use heic_rs::props::{
        Transform,
        simple::{Mirror, Rotation},
    };
    let matrices: [[i8; 4]; 8] = [
        [1, 0, 0, 1],
        [-1, 0, 0, 1],
        [-1, 0, 0, -1],
        [1, 0, 0, -1],
        [0, 1, 1, 0],
        [0, -1, 1, 0],
        [0, -1, -1, 0],
        [0, 1, -1, 0],
    ];
    let mut matrix = matrices[0];
    for transform in &properties.transforms {
        let step = match transform {
            Transform::Rotate(Rotation::None) => matrices[0],
            Transform::Rotate(Rotation::Ccw90) => matrices[7],
            Transform::Rotate(Rotation::Ccw180) => matrices[2],
            Transform::Rotate(Rotation::Ccw270) => matrices[5],
            Transform::Mirror(Mirror::LeftRight) => matrices[1],
            Transform::Mirror(Mirror::TopBottom) => matrices[3],
            Transform::Crop(_) => return None,
        };
        let [a, b, c, d] = matrix;
        let [e, f, g, h] = step;
        matrix = [e * a + f * c, e * b + f * d, g * a + h * c, g * b + h * d];
    }
    matrices
        .iter()
        .position(|m| *m == matrix)
        .map(|n| n as u8 + 1)
}

fn decode_primary(
    ctx: &Context<'_>,
    apply_transforms: bool,
    decoder: &heic_rs::decode::CodedDecoder<'_>,
) -> Result<Bitmap> {
    let alpha = ctx.alpha_item(ctx.meta.primary)?.is_some();
    let options = heic_rs::DecodeOptions {
        layout: if alpha {
            heic_rs::PixelLayout::Rgba8
        } else {
            heic_rs::PixelLayout::Rgb8
        },
        max_pixels: Some(admission::MAX_SOURCE_PIXELS),
        threads: Some(1),
        apply_transforms,
        ..Default::default()
    };
    let image = heic_rs::decode::decode_context(ctx, &options, decoder)?;
    if alpha {
        Ok(Bitmap::new_rgba(image.data, image.width, image.height))
    } else {
        Ok(Bitmap::new_rgb(image.data, image.width, image.height))
    }
}

pub(super) fn decode_unoriented(bytes: &[u8]) -> Result<Bitmap> {
    let ctx = Context::open(bytes)?;
    let properties = ctx.props(ctx.meta.primary)?;
    decode_primary(&ctx, orientation(&properties).is_none(), &coded)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/media/thumbnails/still/heif.rs"]
mod tests;
