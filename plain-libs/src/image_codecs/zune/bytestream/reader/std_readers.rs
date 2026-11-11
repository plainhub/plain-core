use crate::image_codecs::zune::bytestream::reader::{ZByteIoError, ZSeekFrom};
use crate::image_codecs::zune::bytestream::{ZByteReaderTrait, ZCursor};
use std::io;
use std::io::SeekFrom;
impl<T: io::BufRead + io::Seek> ZByteReaderTrait for T {
    #[inline(always)]
    fn read_byte_no_error(&mut self) -> u8 {
        let mut buf = [0];
        let _ = self.read(&mut buf);
        buf[0]
    }
    #[inline(always)]
    fn read_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        let mut bytes_read = 0;
        while bytes_read < buf.len() {
            match self.read(&mut buf[bytes_read..]) {
                Ok(0) => {
                    self.seek(SeekFrom::Current(-(bytes_read as i64)))
                        .map_err(ZByteIoError::from)?;
                    return Err(ZByteIoError::NotEnoughBytes(bytes_read, buf.len()));
                }
                Ok(bytes) => {
                    bytes_read += bytes;
                }
                Err(e) => return Err(ZByteIoError::from(e)),
            }
        }
        Ok(())
    }
    #[inline(always)]
    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError> {
        self.read(buf).map_err(ZByteIoError::from)
    }
    #[inline(always)]
    fn peek_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError> {
        let bytes_read = self.read_bytes(buf)?;
        let converted = -i64::try_from(bytes_read).map_err(ZByteIoError::from)?;
        self.seek(std::io::SeekFrom::Current(converted))
            .map_err(ZByteIoError::from)?;
        Ok(bytes_read)
    }
    #[inline(always)]
    fn peek_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        self.read_exact_bytes(buf)?;
        let converted = -i64::try_from(buf.len()).map_err(ZByteIoError::from)?;
        self.seek(std::io::SeekFrom::Current(converted))
            .map_err(ZByteIoError::from)?;
        Ok(())
    }
    #[inline(always)]
    fn z_seek(&mut self, from: ZSeekFrom) -> Result<u64, ZByteIoError> {
        self.seek(from.to_std_seek()).map_err(ZByteIoError::from)
    }
    #[inline(always)]
    fn is_eof(&mut self) -> Result<bool, ZByteIoError> {
        self.fill_buf()
            .map(|b| b.is_empty())
            .map_err(ZByteIoError::from)
    }
    #[inline(always)]
    fn z_position(&mut self) -> Result<u64, ZByteIoError> {
        self.stream_position().map_err(ZByteIoError::from)
    }
    #[inline(always)]
    fn read_remaining(&mut self, sink: &mut Vec<u8>) -> Result<usize, ZByteIoError> {
        self.read_to_end(sink).map_err(ZByteIoError::from)
    }
}
impl<T: AsRef<[u8]>> std::io::Read for ZCursor<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.read_bytes_impl(buf)
            .map_err(|e| std::io::Error::other(format!("{e:?}")))
    }
}
impl<T: AsRef<[u8]>> std::io::BufRead for ZCursor<T> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        Ok(ZCursor::split(self).1)
    }
    fn consume(&mut self, amount: usize) {
        self.position += amount;
    }
}
impl<T: AsRef<[u8]>> std::io::Seek for ZCursor<T> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let (base_pos, offset) = match pos {
            std::io::SeekFrom::Start(n) => {
                self.position = n as usize;
                return Ok(n);
            }
            std::io::SeekFrom::End(n) => (self.stream.as_ref().len(), n as isize),
            std::io::SeekFrom::Current(n) => (self.position, n as isize),
        };
        match base_pos.checked_add_signed(offset) {
            Some(n) => {
                self.position = n;
                Ok(self.position as u64)
            }
            None => Err(std::io::Error::other("Negative seek")),
        }
    }
}
