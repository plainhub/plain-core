use crate::utils::source_reader::SourceReader;
use std::{fs::File, io::BufReader, path::Path};

// Keep codec instantiations in this library behind concrete application operations.
pub fn file_dimensions(reader: File) -> Option<(u32, u32)> {
    crate::image::ImageReader::new(BufReader::new(reader))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

pub fn source_dimensions(reader: &mut SourceReader) -> Option<(u32, u32)> {
    crate::image::ImageReader::new(BufReader::new(reader))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_dimensions().ok())
}

pub fn thumbnail(path: &Path, width: u32, height: u32) -> Result<Option<Vec<u8>>, String> {
    use crate::image::ImageDecoder;
    let reader = crate::image::ImageReader::open(path)
        .map_err(|error| error.to_string())?
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    let format = reader.format();
    let mut decoder = match reader.into_decoder() {
        Ok(decoder) => decoder,
        Err(crate::image::ImageError::Unsupported(_)) => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let orientation = decoder
        .orientation()
        .unwrap_or(crate::image::metadata::Orientation::NoTransforms);
    let mut image =
        crate::image::DynamicImage::from_decoder(decoder).map_err(|error| error.to_string())?;
    image.apply_orientation(orientation);
    let (width, height) = crate::media::thumbnail_limits::compute_target_size(
        image.width(),
        image.height(),
        width,
        height,
    );
    let image = image.thumbnail_exact(width, height);
    let mut bytes = Vec::new();
    if format == Some(crate::image::ImageFormat::Png) {
        image
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                crate::image::ImageFormat::Png,
            )
            .map_err(|error| error.to_string())?;
    } else {
        crate::image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85)
            .encode_image(&image)
            .map_err(|error| error.to_string())?;
    }
    Ok(Some(bytes))
}

pub fn convert_to_png(path: &Path, output: &Path) -> Result<bool, String> {
    let image = match crate::image::open(path) {
        Ok(image) => image,
        Err(crate::image::ImageError::Unsupported(_)) => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    image
        .save_with_format(output, crate::image::ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(true)
}
