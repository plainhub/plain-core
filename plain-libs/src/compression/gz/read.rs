use super::GzHeader;
use super::bufread;
use crate::compression::bufreader::BufReader;
use crate::compression::io;
use crate::compression::io::Read;
#[derive(Debug)]
pub struct GzDecoder<R> {
    inner: bufread::GzDecoder<BufReader<R>>,
}
impl<R: Read> GzDecoder<R> {
    pub fn new(r: R) -> GzDecoder<R> {
        GzDecoder {
            inner: bufread::GzDecoder::new(BufReader::new(r)),
        }
    }
}
impl<R> GzDecoder<R> {
    pub fn header(&self) -> Option<&GzHeader> {
        self.inner.header()
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
    pub fn reset(&mut self, r: R) -> R {
        super::bufread::reset_decoder_data(&mut self.inner);
        self.inner.get_mut().reset(r)
    }
}
impl<R: Read> Read for GzDecoder<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        self.inner.read(into)
    }
}
