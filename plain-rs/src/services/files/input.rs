use plain_rs::host::resource_stream::NativeResourceFile;
#[cfg(unix)]
use std::io::Read;
use std::{
    io::{Seek, SeekFrom},
    path::Path,
};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    sync::watch,
};

enum Reader {
    File(tokio::fs::File),
    #[cfg(unix)]
    Pipe(tokio::io::unix::AsyncFd<std::fs::File>),
}
impl Reader {
    async fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::File(file) => file.read(bytes).await,
            #[cfg(unix)]
            Self::Pipe(file) => loop {
                let mut ready = file.readable().await?;
                match ready.try_io(|file| {
                    let mut source = file.get_ref();
                    source.read(bytes)
                }) {
                    Ok(result) => return result,
                    Err(_) => continue,
                }
            },
        }
    }
}
fn reader(file: std::fs::File, seekable: bool) -> std::io::Result<Reader> {
    #[cfg(unix)]
    if !seekable {
        use std::os::fd::AsRawFd;
        let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
        if flags < 0
            || unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
        {
            return Err(std::io::Error::last_os_error());
        }
        return tokio::io::unix::AsyncFd::new(file).map(Reader::Pipe);
    }
    #[cfg(not(unix))]
    let _ = seekable;
    Ok(Reader::File(tokio::fs::File::from_std(file)))
}

pub async fn prepare(
    mut source: NativeResourceFile,
    directory: &Path,
    mut stop: watch::Receiver<bool>,
) -> Result<NativeResourceFile, String> {
    if *stop.borrow() {
        return Err("Media input cancelled".into());
    }
    if source.offset < 0
        || source.length < -1
        || (source.length >= 0 && source.offset.checked_add(source.length).is_none())
    {
        return Err("Invalid media input range".into());
    }
    let seekable = match source.file.seek(SeekFrom::Start(source.offset as u64)) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotSeekable => false,
        Err(error) => return Err(error.to_string()),
    };
    if seekable && source.length >= 0 {
        return Ok(source);
    }
    let directory = directory.to_path_buf();
    let temporary = tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&directory)?;
        tempfile::tempfile_in(directory)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;
    let copy = async move {
        let mut output = tokio::fs::File::from_std(temporary);
        let mut input = reader(source.file, seekable).map_err(|error| error.to_string())?;
        let mut skip = if seekable { 0 } else { source.offset as u64 };
        let mut remaining = (source.length >= 0).then_some(source.length as u64);
        let mut written = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            if skip == 0 && remaining == Some(0) {
                break;
            }
            let limit = if skip > 0 {
                skip
            } else {
                remaining.unwrap_or(u64::MAX)
            }
            .min(buffer.len() as u64) as usize;
            let count = input
                .read(&mut buffer[..limit])
                .await
                .map_err(|error| error.to_string())?;
            if count == 0 {
                if skip > 0 || remaining.is_some_and(|remaining| remaining > 0) {
                    return Err("Media input ended before the declared range".into());
                }
                break;
            }
            if skip > 0 {
                skip -= count as u64;
                continue;
            }
            output
                .write_all(&buffer[..count])
                .await
                .map_err(|error| error.to_string())?;
            written = written
                .checked_add(count as u64)
                .ok_or("Media input too large")?;
            if let Some(remaining) = &mut remaining {
                *remaining -= count as u64;
            }
        }
        output.flush().await.map_err(|error| error.to_string())?;
        output
            .seek(SeekFrom::Start(0))
            .await
            .map_err(|error| error.to_string())?;
        NativeResourceFile::new(
            output.into_std().await,
            0,
            i64::try_from(written).map_err(|error| error.to_string())?,
        )
    };
    tokio::select! {
        biased;
        _ = stop.changed() => Err("Media input cancelled".into()),
        result = copy => result,
    }
}

pub struct ReadCancellation(pub watch::Sender<bool>);
impl Drop for ReadCancellation {
    fn drop(&mut self) {
        let _ = self.0.send(true);
    }
}

pub struct CancellableReader<R> {
    pub reader: R,
    pub stop: watch::Receiver<bool>,
}
impl<R: std::io::Read> std::io::Read for CancellableReader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if *self.stop.borrow() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "Media read cancelled",
            ));
        }
        self.reader.read(bytes)
    }
}
impl<R: Seek> Seek for CancellableReader<R> {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        if *self.stop.borrow() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "Media read cancelled",
            ));
        }
        self.reader.seek(from)
    }
}

pub type SourceReader =
    CancellableReader<plain_rs::utils::range_reader::RangeReader<std::fs::File>>;

pub async fn read_source<T: Send + 'static>(
    resources: &plain_rs::host::resources::NativeResources,
    cache: &Path,
    path: &str,
    read: impl FnOnce(&mut SourceReader) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let (cancel, stop) = watch::channel(false);
    let _cancel = ReadCancellation(cancel);
    let path = plain_rs::utils::file_uri::local_path(path)?;
    let input = if path.starts_with("content://") || path.starts_with("ph://") {
        let input = resources.read_file(&path, stop.clone()).await?;
        prepare(input, cache, stop.clone()).await?
    } else {
        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
            let metadata = file.metadata().map_err(|error| error.to_string())?;
            if !metadata.is_file() {
                return Err("Regular media file required".to_owned());
            }
            NativeResourceFile::new(
                file,
                0,
                i64::try_from(metadata.len()).map_err(|error| error.to_string())?,
            )
        })
        .await
        .map_err(|error| error.to_string())??
    };
    tokio::task::spawn_blocking(move || {
        let mut reader = CancellableReader {
            reader: input.into_reader()?,
            stop,
        };
        let result = read(&mut reader)?;
        if *reader.stop.borrow() {
            return Err("Media read cancelled".into());
        }
        Ok(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/input.rs"]
mod tests;
