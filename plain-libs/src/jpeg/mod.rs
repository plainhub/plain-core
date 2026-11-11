// Adapted and pruned from the upstream source identified in SOURCE.json.
pub(crate) use decoder::ycbcr_to_rgb;
pub use decoder::{Decoder, PixelFormat};
use std::io;
mod arch;
mod decoder;
mod error;
mod huffman;
mod idct;
mod marker;
mod parser;
mod upsampler;
mod worker;
fn read_u8<R: io::Read>(reader: &mut R) -> io::Result<u8> {
    let mut buf = [0];
    reader.read_exact(&mut buf)?;
    Ok(buf[0])
}
fn read_u16_from_be<R: io::Read>(reader: &mut R) -> io::Result<u16> {
    let mut buf = [0, 0];
    reader.read_exact(&mut buf)?;
    Ok(u16::from_be_bytes(buf))
}
