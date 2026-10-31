use plain_rs::host::bridge::Host;
use serde_json::{Value, json};
use std::path::Path;

pub async fn read(
    host: &Host,
    operation: &str,
    path: &str,
    params: Value,
) -> Result<Value, String> {
    if !matches!(
        operation,
        "mime" | "probe" | "animated" | "decodePng" | "packageIcon"
    ) {
        return Err(format!("Unsupported resource primitive: {operation}"));
    }
    if operation == "mime" {
        if path.starts_with("content://") {
            return host
                .call(
                    "systemFileResource",
                    json!({"operation":operation,"path":path,"params":params}),
                )
                .await;
        }
        return Ok(Value::String(
            plain_rs::utils::mime::mime_from_ext(path).into(),
        ));
    }
    if operation == "probe" {
        let input = path.to_owned();
        let tracks = tokio::task::spawn_blocking(move || {
            plain_rs::media::isobmff::tracks(Path::new(&input))
        })
        .await
        .map_err(|error| error.to_string())?;
        if let Some(tracks) = tracks {
            return Ok(Value::String(tracks.first_video_codec.unwrap_or_default()));
        }
        if cfg!(any(target_os = "android", target_os = "ios")) {
            return Ok(Value::String(String::new()));
        }
        return native(operation, path, params).await;
    }
    if operation == "animated" {
        let path = path.to_owned();
        return tokio::task::spawn_blocking(move || {
            plain_rs::files::filesystem::image_format::is_animated_image_or_svg(Path::new(&path))
        })
        .await
        .map(Value::Bool)
        .map_err(|error| error.to_string());
    }
    if cfg!(any(target_os = "android", target_os = "ios")) {
        return host
            .call_wait(
                "systemFileResource",
                json!({"operation":operation,"path":path,"params":params}),
            )
            .await;
    }
    native(operation, path, params).await
}

pub async fn write(host: &Host, source: &Path, destination: &str) -> Result<(), String> {
    if !destination.starts_with("content://") {
        return Err("Unsupported writable resource".into());
    }
    let result = host
        .call_wait(
            "systemFileResource",
            json!({"operation":"write", "path":destination, "params":{"source":source}}),
        )
        .await?;
    if result.as_bool() != Some(true) {
        return Err("Resource write failed".into());
    }
    Ok(())
}

async fn native(operation: &str, path: &str, params: Value) -> Result<Value, String> {
    match operation {
        "decodePng" => {
            let mime = plain_rs::utils::mime::mime_from_ext(path);
            if !mime.starts_with("image/")
                || matches!(
                    mime,
                    "image/jpeg" | "image/png" | "image/gif" | "image/webp" | "image/svg+xml"
                )
            {
                return Ok(Value::Bool(false));
            }
            let output = params["output"]
                .as_str()
                .ok_or("PNG output path required")?
                .to_owned();
            let path = path.to_owned();
            tokio::task::spawn_blocking(move || -> Result<Value, String> {
                let image = match image::open(path) {
                    Ok(image) => image,
                    Err(image::ImageError::Unsupported(_)) => return Ok(Value::Bool(false)),
                    Err(error) => return Err(error.to_string()),
                };
                image
                    .save_with_format(output, image::ImageFormat::Png)
                    .map_err(|error| error.to_string())?;
                Ok(Value::Bool(true))
            })
            .await
            .map_err(|error| error.to_string())?
        }
        "probe" => {
            #[cfg(feature = "runtime-media")]
            {
                Ok(Value::String(
                    plain_rs::services::files::videos_preview::probe_video_codec(path).await,
                ))
            }
            #[cfg(not(feature = "runtime-media"))]
            {
                Ok(Value::String(String::new()))
            }
        }
        _ => Err(format!(
            "Unsupported native resource primitive: {operation}"
        )),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/resources.rs"]
mod tests;
