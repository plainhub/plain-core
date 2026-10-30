use crate::assembly::server::ServerState;
use serde_json::json;
use std::path::PathBuf;

pub(crate) struct Temporary(pub(crate) PathBuf);

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub(crate) enum Conversion {
    Video3gp,
    PackageIcon,
    Png,
}

pub(crate) async fn convert(
    state: &ServerState,
    conversion: Conversion,
    path: &str,
) -> anyhow::Result<Option<Temporary>> {
    let output = Temporary(
        state
            .directory
            .join(format!(".resource_{}", uuid::Uuid::new_v4())),
    );
    let converted = match conversion {
        Conversion::Video3gp => convert_3gp(state, path, &output.0).await?,
        Conversion::PackageIcon | Conversion::Png => {
            let operation = match conversion {
                Conversion::PackageIcon => "packageIcon",
                _ => "decodePng",
            };
            super::resources::read(&state.host, operation, path, json!({"output":output.0}))
                .await
                .map_err(anyhow::Error::msg)?
                .as_bool()
                == Some(true)
        }
    };
    Ok(converted.then_some(output))
}

async fn convert_3gp(
    state: &ServerState,
    path: &str,
    output: &std::path::Path,
) -> anyhow::Result<bool> {
    if !super::browser::capabilities().contains(&"VIDEO_3GP_CONVERT") {
        return Ok(false);
    }
    let mime = super::resources::read(&state.host, "mime", path, json!({}))
        .await
        .map_err(anyhow::Error::msg)?;
    if !matches!(mime.as_str(), Some("video/3gpp" | "video/3gp")) {
        return Ok(false);
    }
    #[cfg(target_os = "android")]
    {
        let input = state
            .bridge
            .read_file(path, state.stop.clone())
            .await
            .map_err(anyhow::Error::msg)?;
        super::browser::encode_android_file(
            input,
            output.to_owned(),
            plain_rs::platform::android::media::Operation::Convert3gp,
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
                .join(format!(".resource_input_{}", uuid::Uuid::new_v4())),
        );
        let mut reader = state
            .bridge
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
        #[cfg(feature = "media")]
        {
            plain_media::previews::videos::encode_3gp(&input.0, output).await
        }
        #[cfg(not(feature = "media"))]
        {
            let _ = (input, output);
            Ok(false)
        }
    }
}
