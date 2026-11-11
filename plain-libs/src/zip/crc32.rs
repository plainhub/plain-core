use crate::checksum::crc32::Hasher;
use std::io::{self, Read};
#[derive(Debug)]
pub struct Crc32Reader<R> {
    inner: R,
    hasher: Hasher,
    check: u32,
    enabled: bool,
    size_limit: Option<u64>,
    bytes_read: u64,
}
impl<R> Crc32Reader<R> {
    pub(crate) fn new(inner: R, checksum: u32, should_disable: bool) -> Crc32Reader<R> {
        Crc32Reader {
            inner,
            hasher: Hasher::new(),
            check: checksum,
            enabled: !should_disable,
            size_limit: None,
            bytes_read: 0,
        }
    }
    pub(crate) fn with_size_limit(mut self, size_limit: Option<u64>) -> Crc32Reader<R> {
        self.size_limit = size_limit;
        self
    }
    fn count(&mut self, n: usize) -> io::Result<()> {
        self.bytes_read = self.bytes_read.saturating_add(n as u64);
        match self.size_limit {
            Some(limit) if self.bytes_read > limit => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "File is larger than its declared uncompressed size",
            )),
            _ => Ok(()),
        }
    }
    fn check_matches(&self) -> bool {
        let computed_value = self.hasher.clone().finalize();
        self.check == computed_value
    }
    pub fn inner(&self) -> &R {
        &self.inner
    }
    pub fn into_inner(self) -> R {
        self.inner
    }
}
macro_rules! invalid_checksum {
    ($ ($ x : expr) ,*) => {
        io::Error::new(io::ErrorKind::InvalidData, "Invalid checksum")
    };
}
impl<R: Read> Read for Crc32Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buf)?;
        self.count(count)?;
        if self.enabled {
            if count == 0 && !buf.is_empty() && !self.check_matches() {
                return Err(invalid_checksum!());
            }
            self.hasher.update(&buf[..count]);
        }
        Ok(count)
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        let start = buf.len();
        let n = match self.size_limit {
            Some(limit) => (&mut self.inner)
                .take(limit.saturating_sub(self.bytes_read).saturating_add(1))
                .read_to_end(buf)?,
            None => self.inner.read_to_end(buf)?,
        };
        self.count(n)?;
        if self.enabled {
            self.hasher.update(&buf[start..]);
            if !self.check_matches() {
                return Err(invalid_checksum!());
            }
        }
        Ok(n)
    }
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        let start = buf.len();
        let n = match self.size_limit {
            Some(limit) => (&mut self.inner)
                .take(limit.saturating_sub(self.bytes_read).saturating_add(1))
                .read_to_string(buf)?,
            None => self.inner.read_to_string(buf)?,
        };
        self.count(n)?;
        if self.enabled {
            self.hasher.update(&buf.as_bytes()[start..]);
            if !self.check_matches() {
                return Err(invalid_checksum!());
            }
        }
        Ok(n)
    }
}
