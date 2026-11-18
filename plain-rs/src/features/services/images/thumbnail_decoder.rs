use crate::features::services::images::thumbnails::Request;

pub fn uses_system_thumbnail(request: &Request) -> bool {
    cfg!(any(target_os = "android", target_os = "ios"))
        && !request.file_id.is_empty()
        && request.width <= 512
        && !uses_rust_still_decoder(request)
}

pub(super) fn uses_rust_still_decoder(request: &Request) -> bool {
    cfg!(feature = "media")
        && std::path::Path::new(&request.path).is_absolute()
        && (request.short_edge.is_some()
            || matches!(
                plain_libs::utils::mime::mime_from_ext(&request.path),
                "image/bmp" | "image/tiff" | "image/avif" | "image/heic"
            ))
}

pub async fn native(
    request: &Request,
    _cache: &std::path::Path,
) -> Result<Option<Vec<u8>>, String> {
    let path = request.path.clone();
    let width = request.width.min(16384);
    let height = request.height.min(16384);
    let short_edge = request.short_edge;
    tokio::task::spawn_blocking(move || {
        plain_libs::media::image_io::thumbnail(std::path::Path::new(&path), width, height, short_edge)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/images/thumbnail_decoder.rs"]
mod tests;
