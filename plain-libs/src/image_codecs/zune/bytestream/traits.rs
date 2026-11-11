use crate::image_codecs::zune::bytestream::reader::{ZByteIoError, ZSeekFrom};
pub trait ZByteReaderTrait {
    fn read_byte_no_error(&mut self) -> u8;
    fn read_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError>;
    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError>;
    fn peek_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError>;
    fn peek_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError>;
    fn z_seek(&mut self, from: ZSeekFrom) -> Result<u64, ZByteIoError>;
    fn is_eof(&mut self) -> Result<bool, ZByteIoError>;
    fn z_position(&mut self) -> Result<u64, ZByteIoError>;
    fn read_remaining(&mut self, sink: &mut std::vec::Vec<u8>) -> Result<usize, ZByteIoError>;
}
