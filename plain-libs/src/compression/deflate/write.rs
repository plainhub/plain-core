use crate::compression::Compress;
use crate::compression::io;
use crate::compression::io::Write;
use crate::compression::zio;
#[derive(Debug)]
pub struct DeflateEncoder<W: Write> {
    inner: zio::Writer<W, Compress>,
}
impl<W: Write> DeflateEncoder<W> {
    pub fn new(w: W, level: crate::compression::Compression) -> DeflateEncoder<W> {
        DeflateEncoder {
            inner: zio::Writer::new(w, Compress::new(level, false)),
        }
    }
    pub fn get_ref(&self) -> &W {
        self.inner.get_ref()
    }
    pub fn get_mut(&mut self) -> &mut W {
        self.inner.get_mut()
    }
    pub fn reset(&mut self, w: W) -> io::Result<W> {
        self.inner.finish()?;
        self.inner.data.reset();
        Ok(self.inner.replace(w))
    }
    pub fn try_finish(&mut self) -> io::Result<()> {
        self.inner.finish()
    }
    pub fn finish(mut self) -> io::Result<W> {
        self.inner.finish()?;
        Ok(self.inner.take_inner())
    }
    pub fn flush_finish(mut self) -> io::Result<W> {
        self.inner.flush()?;
        Ok(self.inner.take_inner())
    }
    pub fn total_in(&self) -> u64 {
        self.inner.data.total_in()
    }
    pub fn total_out(&self) -> u64 {
        self.inner.data.total_out()
    }
}
impl<W: Write> Write for DeflateEncoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
