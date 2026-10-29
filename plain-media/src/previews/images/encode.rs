use crate::previews::images::scale::Bitmap;
use anyhow::{Result, bail};

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

    if quality < 90 {
        jpeg::encode(data, w as u16, h as u16, quality)
    } else {
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality).encode(
            data,
            w,
            h,
            image::ExtendedColorType::Rgb8,
        )?;
        Ok(bytes)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/previews/images/encode.rs"]
mod tests;
