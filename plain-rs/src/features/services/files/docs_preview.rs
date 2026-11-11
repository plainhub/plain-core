use crate::runtime::Runtime;
use std::path::Path;

static CONVERSIONS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

pub fn supports_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            undoc::detect::FormatType::ALL
                .iter()
                .any(|format| format.extension().eq_ignore_ascii_case(extension))
        })
}

pub fn supports_mime(mime: &str) -> bool {
    undoc::detect::FormatType::ALL.iter().any(|format| {
        plain_libs::utils::mime::mime_from_ext(&format!("document.{}", format.extension())) == mime
    })
}

pub async fn read(state: &Runtime, path: &str) -> plain_libs::anyhow::Result<String> {
    let permit = CONVERSIONS.acquire().await?;
    if path.starts_with("content://") || path.starts_with("ph://") {
        let mut input = state
            .resources
            .read_native(path, state.stop.clone())
            .await
            .map_err(plain_libs::anyhow::Error::msg)?;
        let mut bytes = Vec::new();
        while let Some(chunk) = input.read().await.map_err(plain_libs::anyhow::Error::msg)? {
            plain_libs::anyhow::ensure!(
                chunk.len() <= undoc::limits::MAX_INPUT_BYTES - bytes.len(),
                "Document exceeds the 32 MiB input limit"
            );
            bytes.extend_from_slice(&chunk);
        }
        return tokio::task::spawn_blocking(move || {
            let _permit = permit;
            render(undoc::parse_bytes(&bytes)?)
        })
        .await?;
    }
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        convert(Path::new(&path))
    })
    .await?
}

fn convert(path: &Path) -> plain_libs::anyhow::Result<String> {
    render(undoc::parse_file(path)?)
}

fn render(document: undoc::Document) -> plain_libs::anyhow::Result<String> {
    Ok(undoc::render::to_markdown(
        &document,
        &undoc::render::RenderOptions::default().with_images(false),
    )?)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/docs_preview.rs"]
mod tests;
