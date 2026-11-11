use crate::compression::Decompress;
use crate::compression::io;
use crate::compression::io::{BufRead, Read};
use crate::compression::zio;
use core::mem;

#[derive(Debug)]
pub struct DeflateDecoder<R> {
    obj: R,
    data: Decompress,
}
pub fn reset_decoder_data<R>(zlib: &mut DeflateDecoder<R>) {
    zlib.data.reset(false);
}
impl<R: BufRead> DeflateDecoder<R> {
    pub fn new(r: R) -> DeflateDecoder<R> {
        DeflateDecoder {
            obj: r,
            data: Decompress::new(false),
        }
    }
}
impl<R> DeflateDecoder<R> {
    pub fn reset(&mut self, r: R) -> R {
        reset_decoder_data(self);
        mem::replace(&mut self.obj, r)
    }
    pub fn reset_data(&mut self) {
        reset_decoder_data(self);
    }
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
impl<R: BufRead> Read for DeflateDecoder<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        zio::read(&mut self.obj, &mut self.data, into)
    }
}
