mod decoder;
mod read;

use super::scale::Bitmap;
use crate::image::ImageDecoder;
use std::io::BufReader;
use std::io::{BufRead, Seek};
use std::path::Path;

const INPUT_BUFFER_SIZE: usize = 64 * 1024;

fn decoder<R: BufRead + Seek>(input: R) -> crate::anyhow::Result<decoder::BmpDecoder<R>> {
    let decoder = decoder::BmpDecoder::new(input)?;
    let (width, height) = decoder.dimensions();
    crate::anyhow::ensure!(
        u64::from(width) * u64::from(height) <= super::admission::MAX_SOURCE_PIXELS,
        "BMP exceeds pixel guard"
    );
    Ok(decoder)
}

pub(super) fn decode_reader(input: impl BufRead + Seek) -> crate::anyhow::Result<Bitmap> {
    Ok(Bitmap::from_dynamic(crate::image::DynamicImage::from_decoder(
        decoder(input)?,
    )?))
}

pub(super) fn thumbnail(
    path: &Path,
    width: u32,
    height: u32,
    crop: Option<(u32, u32)>,
) -> crate::anyhow::Result<Bitmap> {
    let input = BufReader::with_capacity(INPUT_BUFFER_SIZE, std::fs::File::open(path)?);
    let mut decoder = decoder(input)?;
    if let Some(bitmap) = decoder.read_scaled(width, height, crop)? {
        return Ok(super::scale::fir_resize(bitmap, width, height));
    }
    let bitmap = Bitmap::from_dynamic(crate::image::DynamicImage::from_decoder(decoder)?);
    Ok(super::scale::resize_to(
        super::scale::crop_for_fill(bitmap, crop),
        width,
        height,
    ))
}

pub(super) fn has_alpha(header: &[u8]) -> bool {
    let number = |offset: usize| {
        header
            .get(offset..offset + 4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
    };
    matches!(number(14), Some(56 | 108 | 124))
        && number(30) == Some(3)
        && number(66).is_some_and(|mask| mask != 0)
}
