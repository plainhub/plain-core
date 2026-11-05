use crate::features::services::images::thumbnails::Request;

pub fn uses_system_thumbnail(request: &Request) -> bool {
    cfg!(any(target_os = "android", target_os = "ios"))
        && !request.file_id.is_empty()
        && request.width <= 512
        && !uses_rust_bmp_decoder(request)
}

pub(super) fn uses_rust_bmp_decoder(request: &Request) -> bool {
    cfg!(feature = "media")
        && std::path::Path::new(&request.path).is_absolute()
        && plain_libs::utils::mime::mime_from_ext(&request.path) == "image/bmp"
}

pub async fn native(
    request: &Request,
    _cache: &std::path::Path,
) -> Result<Option<Vec<u8>>, String> {
    let path = request.path.clone();
    let width = request.width.min(16384);
    let height = request.height.min(16384);
    let crop = request.center_crop;
    tokio::task::spawn_blocking(move || {
        use image::ImageDecoder;
        let reader = image::ImageReader::open(path)
            .map_err(|error| error.to_string())?
            .with_guessed_format()
            .map_err(|error| error.to_string())?;
        let format = reader.format();
        let mut decoder = match reader.into_decoder() {
            Ok(decoder) => decoder,
            Err(image::ImageError::Unsupported(_)) => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        let orientation = decoder
            .orientation()
            .unwrap_or(image::metadata::Orientation::NoTransforms);
        let mut image =
            image::DynamicImage::from_decoder(decoder).map_err(|error| error.to_string())?;
        image.apply_orientation(orientation);
        let image = if crop {
            image.resize_to_fill(width, height, image::imageops::FilterType::Lanczos3)
        } else {
            image.thumbnail(width, height)
        };
        let mut bytes = Vec::new();
        if format == Some(image::ImageFormat::Png) {
            image
                .write_to(
                    &mut std::io::Cursor::new(&mut bytes),
                    image::ImageFormat::Png,
                )
                .map_err(|error| error.to_string())?;
        } else {
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85)
                .encode_image(&image)
                .map_err(|error| error.to_string())?;
        }
        Ok(Some(bytes))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/images/thumbnail_decoder.rs"]
mod tests;
