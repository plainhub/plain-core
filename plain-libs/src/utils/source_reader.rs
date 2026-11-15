use std::io::{Read, Seek, SeekFrom};
use tokio::sync::watch;

pub struct CancellableReader<R> {
    pub reader: R,
    pub stop: watch::Receiver<bool>,
}

impl<R: Read> Read for CancellableReader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if *self.stop.borrow() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "Media read canceled",
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
                "Media read canceled",
            ));
        }
        self.reader.seek(from)
    }
}

pub type SourceReader = CancellableReader<super::range_reader::RangeReader<std::fs::File>>;

#[cfg(test)]
#[path = "../../../tests/unit/plain-libs/utils/source_reader.rs"]
mod tests;
