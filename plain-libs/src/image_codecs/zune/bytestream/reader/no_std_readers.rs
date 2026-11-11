use crate::image_codecs::zune::bytestream::{ZByteIoError, ZByteReaderTrait, ZSeekFrom};
pub struct ZCursor<T> {
    pub(crate) stream: T,
    pub(crate) position: usize,
}
impl<T: AsRef<[u8]>> ZCursor<T> {
    pub fn new(buffer: T) -> ZCursor<T> {
        ZCursor {
            stream: buffer,
            position: 0,
        }
    }
}
impl<T: AsRef<[u8]>> ZCursor<T> {
    #[inline]
    pub fn skip(&mut self, num: usize) {
        self.position = self.position.wrapping_add(num);
    }
    #[inline]
    pub fn rewind(&mut self, num: usize) {
        self.position = self.position.saturating_sub(num);
    }
    pub fn split(&self) -> (&[u8], &[u8]) {
        let slice = self.stream.as_ref();
        let pos = self.position.min(slice.len());
        slice.split_at(pos)
    }
}
impl<T: AsRef<[u8]>> ZCursor<T> {
    #[inline(always)]
    pub(crate) fn read_exact_bytes_impl(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        let bytes_read = self.read_bytes(buf)?;
        if bytes_read != buf.len() {
            self.rewind(bytes_read);
            return Err(ZByteIoError::NotEnoughBytes(bytes_read, buf.len()));
        }
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn read_bytes_impl(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError> {
        let len = self.peek_bytes_impl(buf)?;
        self.skip(len);
        Ok(len)
    }
    #[inline(always)]
    pub(crate) fn peek_bytes_impl(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError> {
        let stream_end = self.stream.as_ref().len();
        let start = core::cmp::min(self.position, stream_end);
        let end = core::cmp::min(self.position + buf.len(), stream_end);
        let slice = self.stream.as_ref().get(start..end).unwrap();
        buf[..slice.len()].copy_from_slice(slice);
        let len = slice.len();
        Ok(len)
    }
}
impl<T: AsRef<[u8]>> From<T> for ZCursor<T> {
    fn from(value: T) -> Self {
        ZCursor::new(value)
    }
}
