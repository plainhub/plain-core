use crate::media::thumbnails::scale::Bitmap;
use crate::anyhow::{Result, bail};

mod jpeg;

/// Encode a bitmap as JPEG. `quality` is clamped to 1..=100.
pub fn encode_jpeg(bm: Bitmap, quality: u8) -> Result<Vec<u8>> {
    let (w, h) = (bm.width(), bm.height());
    if w == 0 || h == 0 || w > u32::from(u16::MAX) || h > u32::from(u16::MAX) {
        bail!("jpeg encode dimensions out of range: {w}x{h}");
    }
    let quality = quality.clamp(1, 100);
    let bm = bm.flatten_to_rgb();
    let Bitmap::Rgb8 { data, .. } = &bm else {
        unreachable!("flatten_to_rgb always yields Rgb8");
    };

    let bytes = if quality < 90 {
        jpeg::encode(data, w as u16, h as u16, quality)?
    } else {
        let mut bytes = Vec::new();
        crate::image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality).encode(
            data,
            w,
            h,
            crate::image::ExtendedColorType::Rgb8,
        )?;
        bytes
    };
    // Release encoder growth capacity before the bytes enter long-lived shared caches.
    Ok(bytes.into_boxed_slice().into_vec())
}

pub fn encode_png(bm: Bitmap) -> Result<Vec<u8>> {
    use crate::image::ImageEncoder;
    let (pixels, w, h) = bm.raw();
    let color = match &bm {
        Bitmap::Rgb8 { .. } => crate::image::ExtendedColorType::Rgb8,
        Bitmap::Rgba8 { .. } => crate::image::ExtendedColorType::Rgba8,
    };
    let mut bytes = Vec::new();
    crate::image::codecs::png::PngEncoder::new_with_quality(
        &mut bytes,
        crate::image::codecs::png::CompressionType::Fast,
        crate::image::codecs::png::FilterType::Adaptive,
    )
    .write_image(pixels, w, h, color)?;
    Ok(bytes.into_boxed_slice().into_vec())
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/media/thumbnails/encode.rs"]
mod tests;
