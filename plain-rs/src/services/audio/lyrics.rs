use plain_rs::host::resources::{NativeResourceReader, NativeResources};
use std::io::{Cursor, Read};
use tokio::sync::watch;

struct Cancellation(watch::Sender<bool>);
impl Drop for Cancellation {
    fn drop(&mut self) {
        let _ = self.0.send(true);
    }
}
struct Reader {
    source: NativeResourceReader,
    buffered: Cursor<Vec<u8>>,
    runtime: tokio::runtime::Handle,
    error: Option<String>,
}
impl Read for Reader {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            let count = self.buffered.read(output)?;
            if count != 0 {
                return Ok(count);
            }
            match self.runtime.block_on(self.source.read()) {
                Ok(Some(bytes)) => self.buffered = Cursor::new(bytes),
                Ok(None) => return Ok(0),
                Err(error) => {
                    self.error = Some(error.clone());
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        error,
                    ));
                }
            }
        }
    }
}
pub async fn read_for_playback(
    path: String,
    resources: Option<(&NativeResources, watch::Receiver<bool>)>,
) -> Result<String, String> {
    if path.starts_with("content://") || path.starts_with("ph://") {
        return read(path, resources).await;
    }
    let path = plain_rs::utils::file_uri::local_path(&path)?;
    let task = async {
        tokio::task::spawn_blocking(move || {
            plain_rs::media::audio::lyrics::read_with_sidecar(&path)
        })
        .await
        .map_err(|error| error.to_string())
    };
    if let Some((_, mut stopped)) = resources {
        if *stopped.borrow() {
            return Err("Server stopped".into());
        }
        tokio::select! {
            biased;
            _ = stopped.changed() => Err("Server stopped".into()),
            result = task => result,
        }
    } else {
        task.await
    }
}

pub async fn read(
    path: String,
    resources: Option<(&NativeResources, watch::Receiver<bool>)>,
) -> Result<String, String> {
    if !path.starts_with("content://") && !path.starts_with("ph://") {
        let path = plain_rs::utils::file_uri::local_path(&path)?;
        return tokio::task::spawn_blocking(move || {
            plain_rs::media::audio::lyrics::extract_lyrics_from_path(&path)
        })
        .await
        .map_err(|error| error.to_string());
    }
    let (resources, mut stopped) = resources.ok_or("SDK resources are unavailable")?;
    if *stopped.borrow() {
        return Err("Server stopped".into());
    }
    let (cancel, cancelled) = watch::channel(false);
    let _cancel = Cancellation(cancel);
    let reader = tokio::select! {
        _ = stopped.changed() => return Err("Server stopped".into()),
        reader = resources.read_native(&path, cancelled) => reader.map_err(|error| error.to_string())?,
    };
    let runtime = tokio::runtime::Handle::current();
    let task = tokio::task::spawn_blocking(move || {
        let mut reader = Reader {
            source: reader,
            buffered: Cursor::new(Vec::new()),
            runtime,
            error: None,
        };
        let text = plain_rs::media::audio::lyrics::extract(&mut reader);
        reader.error.map_or(Ok(text), Err)
    });
    tokio::select! {
        _ = stopped.changed() => Err("Server stopped".into()),
        result = task => result.map_err(|error| error.to_string())?,
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/audio/lyrics.rs"]
mod tests;
