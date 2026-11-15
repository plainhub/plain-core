use crate::runtime::Runtime;
use std::path::Path;

static CONVERSIONS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

#[derive(Clone, Copy)]
pub(super) enum Format {
    Office,
    Csv,
}

impl Format {
    pub(super) fn from_name(name: &str) -> Option<Self> {
        let extension = Path::new(name).extension()?.to_str()?;
        if extension.eq_ignore_ascii_case("csv") {
            Some(Self::Csv)
        } else {
            undoc::detect::FormatType::ALL
                .iter()
                .any(|format| format.extension().eq_ignore_ascii_case(extension))
                .then_some(Self::Office)
        }
    }

    pub(super) fn from_mime(mime: &str) -> Option<Self> {
        let mime = mime.split(';').next()?.trim();
        if mime.eq_ignore_ascii_case("text/csv") {
            Some(Self::Csv)
        } else {
            undoc::detect::FormatType::ALL
                .iter()
                .any(|format| {
                    plain_libs::utils::mime::mime_from_ext(&format!(
                        "document.{}",
                        format.extension()
                    )) == mime
                })
                .then_some(Self::Office)
        }
    }
}

pub(super) async fn read(
    state: &Runtime,
    path: &str,
    format: Format,
) -> plain_libs::anyhow::Result<String> {
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
            render(match format {
                Format::Csv => undoc::parse_csv(&bytes)?,
                Format::Office => undoc::parse_bytes(&bytes)?,
            })
        })
        .await?;
    }
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        convert(Path::new(&path), format)
    })
    .await?
}

fn convert(path: &Path, format: Format) -> plain_libs::anyhow::Result<String> {
    use std::io::Read;
    render(match format {
        Format::Office => undoc::parse_file(path)?,
        Format::Csv => {
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(undoc::limits::MAX_INPUT_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            undoc::parse_csv(&bytes)?
        }
    })
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
