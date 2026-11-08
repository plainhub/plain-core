use crate::runtime::Runtime as CoreRuntime;
use serde_json::json;
use std::path::PathBuf;

pub struct Temporary(pub PathBuf);

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub enum Conversion {
    Video3gp,
    PackageIcon,
    Png,
}

pub async fn convert(
    state: &CoreRuntime,
    conversion: Conversion,
    path: &str,
) -> anyhow::Result<Option<Temporary>> {
    let output = Temporary(
        state
            .directory
            .join(format!(".resource_{}", plain_libs::utils::uuid::Uuid::new_v4())),
    );
    let converted = match conversion {
        Conversion::Video3gp => convert_3gp(state, path, &output.0).await?,
        Conversion::PackageIcon | Conversion::Png => {
            let operation = match conversion {
                Conversion::PackageIcon => "packageIcon",
                _ => "decodePng",
            };
            crate::features::services::files::resources::read(
                &state.host,
                operation,
                path,
                json!({"output":output.0}),
            )
            .await
            .map_err(anyhow::Error::msg)?
            .as_bool()
                == Some(true)
        }
    };
    Ok(converted.then_some(output))
}

async fn convert_3gp(
    state: &CoreRuntime,
    path: &str,
    output: &std::path::Path,
) -> anyhow::Result<bool> {
    #[cfg(target_os = "android")]
    let available = crate::infrastructure::platform::android::media::supports_3gp_conversion();
    #[cfg(target_os = "ios")]
    let available = false;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let available = crate::features::services::files::browser::ffmpeg_available();
    if !available {
        return Ok(false);
    }
    let mime =
        crate::features::services::files::resources::read(&state.host, "mime", path, json!({}))
            .await
            .map_err(anyhow::Error::msg)?;
    if !matches!(mime.as_str(), Some("video/3gpp" | "video/3gp")) {
        return Ok(false);
    }
    #[cfg(target_os = "android")]
    {
        let input = state
            .resources
            .read_file(path, state.stop.clone())
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        crate::features::services::files::browser::encode_android_file(
            input,
            output.to_owned(),
            crate::infrastructure::platform::android::media::Operation::Convert3gp,
            state.stop.clone(),
        )
        .await
        .map_err(anyhow::Error::msg)
    }
    #[cfg(not(target_os = "android"))]
    {
        let input = Temporary(
            state
                .cache
                .root()
                .join(format!(".resource_input_{}", plain_libs::utils::uuid::Uuid::new_v4())),
        );
        let mut reader = state
            .resources
            .read_native(path, state.stop.clone())
            .await
            .map_err(|status| anyhow::anyhow!("Video source returned {status}"))?;
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::File::create(&input.0).await?;
        while let Some(chunk) = reader.read().await.map_err(anyhow::Error::msg)? {
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        drop(file);
        #[cfg(feature = "runtime-media")]
        {
            crate::features::services::files::videos_preview::encode_3gp(&input.0, output).await
        }
        #[cfg(not(feature = "runtime-media"))]
        {
            let _ = (input, output);
            Ok(false)
        }
    }
}
