use super::{GzHeader, GzHeaderParser, corrupt, read_into};
use crate::compression::crc::CrcReader;
use crate::compression::deflate;
use crate::compression::io;
use crate::compression::io::{BufRead, Read};
use core::mem;

#[inline]
fn finish(buf: &[u8; 8]) -> (u32, u32) {
    let crc = (buf[0] as u32)
        | ((buf[1] as u32) << 8)
        | ((buf[2] as u32) << 16)
        | ((buf[3] as u32) << 24);
    let amt = (buf[4] as u32)
        | ((buf[5] as u32) << 8)
        | ((buf[6] as u32) << 16)
        | ((buf[7] as u32) << 24);
    (crc, amt)
}
#[derive(Debug)]
pub struct GzDecoder<R> {
    state: GzState,
    reader: CrcReader<deflate::bufread::DeflateDecoder<R>>,
}
#[derive(Debug)]
enum GzState {
    Header(GzHeaderParser),
    Body(GzHeader),
    Finished(GzHeader, usize, [u8; 8]),
    Err(io::Error),
    End(Option<GzHeader>),
}
pub fn reset_decoder_data<R>(decoder: &mut GzDecoder<R>) {
    decoder.state = GzState::Header(GzHeaderParser::new());
    decoder.reader.reset();
    decoder.reader.get_mut().reset_data();
}
impl<R: BufRead> GzDecoder<R> {
    pub fn new(mut r: R) -> GzDecoder<R> {
        let mut header_parser = GzHeaderParser::new();
        let state = match header_parser.parse(&mut r) {
            Ok(_) => GzState::Body(GzHeader::from(header_parser)),
            Err(ref err) if io::ErrorKind::WouldBlock == err.kind() => {
                GzState::Header(header_parser)
            }
            Err(err) => GzState::Err(err),
        };
        GzDecoder {
            state,
            reader: CrcReader::new(deflate::bufread::DeflateDecoder::new(r)),
        }
    }
}
impl<R> GzDecoder<R> {
    pub fn header(&self) -> Option<&GzHeader> {
        match &self.state {
            GzState::Body(header) | GzState::Finished(header, _, _) => Some(header),
            GzState::End(header) => header.as_ref(),
            _ => None,
        }
    }
    pub fn get_ref(&self) -> &R {
        self.reader.get_ref().get_ref()
    }
    pub fn get_mut(&mut self) -> &mut R {
        self.reader.get_mut().get_mut()
    }
    pub fn into_inner(self) -> R {
        self.reader.into_inner().into_inner()
    }
}
impl<R: BufRead> Read for GzDecoder<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        loop {
            match &mut self.state {
                GzState::Header(parser) => {
                    parser.parse(self.reader.get_mut().get_mut())?;
                    self.state = GzState::Body(GzHeader::from(mem::take(parser)));
                }
                GzState::Body(header) => {
                    if into.is_empty() {
                        return Ok(0);
                    }
                    match self.reader.read(into)? {
                        0 => {
                            self.state = GzState::Finished(mem::take(header), 0, [0; 8]);
                        }
                        n => {
                            return Ok(n);
                        }
                    }
                }
                GzState::Finished(header, pos, buf) => {
                    if *pos < buf.len() {
                        *pos += read_into(self.reader.get_mut().get_mut(), &mut buf[*pos..])?;
                    } else {
                        let (crc, amt) = finish(buf);
                        if crc != self.reader.crc().sum() || amt != self.reader.crc().amount() {
                            self.state = GzState::End(Some(mem::take(header)));
                            return Err(corrupt());
                        } else {
                            self.state = GzState::End(Some(mem::take(header)));
                        }
                    }
                }
                GzState::Err(err) => {
                    let result = Err(mem::replace(err, io::ErrorKind::Other.into()));
                    self.state = GzState::End(None);
                    return result;
                }
                GzState::End(_) => return Ok(0),
            }
        }
    }
}
