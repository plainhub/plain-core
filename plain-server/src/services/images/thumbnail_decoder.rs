use super::thumbnails::Request;

pub(crate) fn uses_system_thumbnail(request: &Request) -> bool {
    cfg!(any(target_os = "android", target_os = "ios"))
        && !request.file_id.is_empty()
        && request.width <= 512
}

#[cfg(feature = "media")]
pub(crate) async fn native(
    request: &Request,
    cache: &std::path::Path,
) -> Result<Option<Vec<u8>>, String> {
    let meta = tokio::fs::metadata(&request.path)
        .await
        .map_err(|error| error.to_string())?;
    let modified = meta
        .modified()
        .map_err(|error| error.to_string())?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let spec = plain_media::previews::images::ThumbSpec::sanitize(
        &request.path,
        i32::try_from(request.width).map_err(|error| error.to_string())?,
        i32::try_from(request.height).map_err(|error| error.to_string())?,
        85,
        modified,
        meta.len() as i64,
    );
    let outcome = plain_media::previews::images::get_thumbnail_with_cache(spec.clone(), cache)
        .await
        .map_err(|error| error.to_string())?;
    let bytes = match outcome {
        plain_media::previews::images::ThumbOutcome::Generated(bytes) => bytes,
        plain_media::previews::images::ThumbOutcome::Original { data, .. } => data,
    };
    let bytes = if request.center_crop {
        tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
            let image = image::load_from_memory(&bytes)
                .map_err(|error| error.to_string())?
                .resize_to_fill(spec.w, spec.h, image::imageops::FilterType::Lanczos3);
            let mut bytes = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85)
                .encode_image(&image)
                .map_err(|error| error.to_string())?;
            Ok(bytes)
        })
        .await
        .map_err(|error| error.to_string())??
    } else {
        (*bytes).clone()
    };
    Ok(Some(bytes))
}

#[cfg(not(feature = "media"))]
pub(crate) async fn native(
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
