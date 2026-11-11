use super::bufread;
use crate::compression::Decompress;
use crate::compression::bufreader::BufReader;
use crate::compression::io;
use crate::compression::io::Read;
use std::vec::Vec;
#[derive(Debug)]
pub struct ZlibDecoder<R> {
    inner: bufread::ZlibDecoder<BufReader<R>>,
}
impl<R: Read> ZlibDecoder<R> {
    pub fn new(r: R) -> ZlibDecoder<R> {
        ZlibDecoder::new_with_buf(r, vec![0; 32 * 1024])
    }
    pub fn new_with_buf(r: R, buf: Vec<u8>) -> ZlibDecoder<R> {
        ZlibDecoder {
            inner: bufread::ZlibDecoder::new(BufReader::with_buf(buf, r)),
        }
    }
    pub fn new_with_decompress(r: R, decompression: Decompress) -> ZlibDecoder<R> {
        ZlibDecoder::new_with_decompress_and_buf(r, vec![0; 32 * 1024], decompression)
    }
    pub fn new_with_decompress_and_buf(
        r: R,
        buf: Vec<u8>,
        decompression: Decompress,
    ) -> ZlibDecoder<R> {
        ZlibDecoder {
            inner: bufread::ZlibDecoder::new_with_decompress(
                BufReader::with_buf(buf, r),
                decompression,
            ),
        }
    }
}
impl<R> ZlibDecoder<R> {
    pub fn reset(&mut self, r: R) -> R {
        super::bufread::reset_decoder_data(&mut self.inner);
        self.inner.get_mut().reset(r)
    }
    pub fn get_ref(&self) -> &R {
        self.inner.get_ref().get_ref()
    }
    pub fn get_mut(&mut self) -> &mut R {
        self.inner.get_mut().get_mut()
    }
    pub fn into_inner(self) -> R {
        self.inner.into_inner().into_inner()
    }
    pub fn total_in(&self) -> u64 {
        self.inner.total_in()
    }
    pub fn total_out(&self) -> u64 {
        self.inner.total_out()
    }
}
impl<R: Read> Read for ZlibDecoder<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        self.inner.read(into)
    }
}
