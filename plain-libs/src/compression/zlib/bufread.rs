use crate::compression::Decompress;
use crate::compression::io;
use crate::compression::io::{BufRead, Read};
use crate::compression::zio;

#[derive(Debug)]
pub struct ZlibDecoder<R> {
    obj: R,
    data: Decompress,
}
impl<R: BufRead> ZlibDecoder<R> {
    pub fn new(r: R) -> ZlibDecoder<R> {
        ZlibDecoder {
            obj: r,
            data: Decompress::new(true),
        }
    }
    pub fn new_with_decompress(r: R, decompression: Decompress) -> ZlibDecoder<R> {
        ZlibDecoder {
            obj: r,
            data: decompression,
        }
    }
}
pub fn reset_decoder_data<R>(zlib: &mut ZlibDecoder<R>) {
    zlib.data.reset(true);
}
impl<R> ZlibDecoder<R> {
    pub fn get_ref(&self) -> &R {
        &self.obj
    }
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.obj
    }
    pub fn into_inner(self) -> R {
        self.obj
    }
    pub fn total_in(&self) -> u64 {
        self.data.total_in()
    }
    pub fn total_out(&self) -> u64 {
        self.data.total_out()
    }
}
impl<R: BufRead> Read for ZlibDecoder<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        zio::read(&mut self.obj, &mut self.data, into)
    }
}
