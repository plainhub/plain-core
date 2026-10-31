use plain_rs::services::images::thumbnails::Request;

pub fn uses_system_thumbnail(request: &Request) -> bool {
    cfg!(any(target_os = "android", target_os = "ios"))
        && !request.file_id.is_empty()
        && request.width <= 512
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
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85)
            .encode_image(&image)
            .map_err(|error| error.to_string())?;
        Ok(Some(bytes))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
#[path = "../../../tests/unit/services/images/thumbnail_decoder.rs"]
mod tests;
