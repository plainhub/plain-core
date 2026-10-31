use super::*;
use std::io::{Read as _, Write};

fn stopped() -> (watch::Sender<bool>, watch::Receiver<bool>) {
    watch::channel(false)
}
fn bytes(mut input: NativeResourceFile) -> Vec<u8> {
    input
        .file
        .seek(SeekFrom::Start(input.offset as u64))
        .unwrap();
    let mut result = Vec::new();
    input
        .file
        .take(input.length as u64)
        .read_to_end(&mut result)
        .unwrap();
    result
}

#[tokio::test]
async fn ordinary_asset_keeps_its_descriptor_and_does_not_create_a_temporary_directory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source");
    std::fs::write(&path, b"prefixPAYLOADsuffix").unwrap();
    let file = std::fs::File::open(path).unwrap();
    #[cfg(unix)]
    let descriptor = {
        use std::os::fd::AsRawFd;
        file.as_raw_fd()
    };
    let (_stop, stop) = stopped();
    let cache = dir.path().join("never-created");
    let prepared = prepare(NativeResourceFile::new(file, 6, 7).unwrap(), &cache, stop)
        .await
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        assert_eq!(prepared.file.as_raw_fd(), descriptor);
    }
    assert!(!cache.exists());
    assert_eq!(bytes(prepared), b"PAYLOAD");
}

#[tokio::test]
async fn cancellation_before_input_preparation_creates_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("never-created");
    let (stop, stopped) = stopped();
    stop.send_replace(true);
    assert!(
        prepare(
            NativeResourceFile::new(tempfile::tempfile().unwrap(), 0, 0).unwrap(),
            &cache,
            stopped
        )
        .await
        .is_err()
    );
    assert!(!cache.exists());
}

#[cfg(unix)]
fn pipe() -> (std::fs::File, std::fs::File) {
    use std::os::fd::FromRawFd;
    let mut fds = [-1; 2];
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    unsafe {
        (
            std::fs::File::from_raw_fd(fds[0]),
            std::fs::File::from_raw_fd(fds[1]),
        )
    }
}

#[cfg(unix)]
#[tokio::test]
async fn unknown_length_pipe_is_materialized_and_rewound_by_rust() {
    let dir = tempfile::tempdir().unwrap();
    let (read, mut write) = pipe();
    let source = NativeResourceFile::new(read, 0, -1).unwrap();
    assert_eq!(source.length, -1);
    let producer =
        tokio::task::spawn_blocking(move || write.write_all(b"streamed payload").unwrap());
    let (_stop, stop) = stopped();
    let prepared = prepare(source, dir.path(), stop).await.unwrap();
    producer.await.unwrap();
    assert_eq!(prepared.offset, 0);
    assert_eq!(prepared.length, 16);
    assert_eq!(bytes(prepared), b"streamed payload");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn pipe_asset_skips_prefix_and_does_not_consume_the_suffix() {
    let dir = tempfile::tempdir().unwrap();
    let (read, mut write) = pipe();
    let mut remainder = read.try_clone().unwrap();
    write.write_all(b"prefixPAYLOADtail").unwrap();
    drop(write);
    let source = NativeResourceFile::new(read, 6, 7).unwrap();
    let (_stop, stop) = stopped();
    let prepared = prepare(source, dir.path(), stop).await.unwrap();
    assert_eq!(bytes(prepared), b"PAYLOAD");
    let mut suffix = [0; 4];
    remainder.read_exact(&mut suffix).unwrap();
    assert_eq!(&suffix, b"tail");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn incomplete_pipe_assets_fail_and_leave_no_temporary_path() {
    let dir = tempfile::tempdir().unwrap();
    for (offset, length) in [(0, 7), (7, -1)] {
        let (read, mut write) = pipe();
        write.write_all(b"short").unwrap();
        drop(write);
        let (_stop, stop) = stopped();
        assert!(
            prepare(
                NativeResourceFile::new(read, offset, length).unwrap(),
                dir.path(),
                stop
            )
            .await
            .is_err()
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}

#[cfg(unix)]
#[test]
fn cancellation_retires_a_queued_temporary_allocation_before_returning() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let (read, _write) = pipe();
        let (stop, stopped) = stopped();
        let (release, blocked) = std::sync::mpsc::channel();
        let (started, ready) = tokio::sync::oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            let _ = started.send(());
            let _ = blocked.recv();
        });
        ready.await.unwrap();
        let preparing = prepare(
            NativeResourceFile::new(read, 0, -1).unwrap(),
            dir.path(),
            stopped,
        );
        tokio::pin!(preparing);
        assert!(futures_util::poll!(&mut preparing).is_pending());
        stop.send_replace(true);
        assert!(futures_util::poll!(&mut preparing).is_pending());
        release.send(()).unwrap();
        blocker.await.unwrap();
        assert!(preparing.await.is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    });
}

#[cfg(unix)]
#[tokio::test]
async fn stop_interrupts_a_pipe_without_waiting_for_its_producer_to_close() {
    let dir = tempfile::tempdir().unwrap();
    let (read, _write) = pipe();
    let (stop, stopped) = stopped();
    let preparing = prepare(
        NativeResourceFile::new(read, 0, -1).unwrap(),
        dir.path(),
        stopped,
    );
    tokio::pin!(preparing);
    tokio::select! {
        biased;
        _ = &mut preparing => panic!("empty pipe cannot finish before cancellation"),
        _ = tokio::task::yield_now() => {},
    }
    stop.send_replace(true);
    assert!(preparing.await.is_err());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn cancelling_a_metadata_reader_stops_reads_and_seeks_without_retrying_interrupted_errors() {
    let (cancel, stop) = watch::channel(false);
    let guard = ReadCancellation(cancel);
    let mut input = CancellableReader {
        reader: std::io::Cursor::new(b"bytes"),
        stop,
    };
    drop(guard);
    let error = std::io::Read::read(&mut input, &mut [0; 1]).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::ConnectionAborted);
    assert!(std::io::Read::read_exact(&mut input, &mut [0; 1]).is_err());
    assert!(input.seek(SeekFrom::Start(0)).is_err());
    assert_eq!(input.reader.position(), 0);
}
