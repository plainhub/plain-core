pub(crate) use crate::checksum::crc32::Hasher as Crc;
use crate::compression::io;
use crate::compression::io::{BufRead, Read};
#[derive(Debug)]
pub struct CrcReader<R> {
    inner: R,
    crc: Crc,
}
impl<R: Read> CrcReader<R> {
    pub fn new(r: R) -> CrcReader<R> {
        CrcReader {
            inner: r,
            crc: Crc::new(),
        }
    }
}
impl<R> CrcReader<R> {
    pub fn crc(&self) -> &Crc {
        &self.crc
    }
    pub fn into_inner(self) -> R {
        self.inner
    }
    pub fn get_ref(&self) -> &R {
        &self.inner
    }
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }
    pub fn reset(&mut self) {
        self.crc.reset();
    }
}
impl<R: Read> Read for CrcReader<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        let amt = self.inner.read(into)?;
        self.crc.update(&into[..amt]);
        Ok(amt)
    }
}
impl<R: BufRead> BufRead for CrcReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.inner.fill_buf()
    }
    fn consume(&mut self, amt: usize) {
        if let Ok(data) = self.inner.fill_buf() {
            self.crc.update(&data[..amt]);
        }
        self.inner.consume(amt);
    }
}
