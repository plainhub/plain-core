// Container extraction and TIFF validation adapted from kamadak-exif 0.6.1; see licenses/kamadak-exif-BSD.txt.
mod endian;
mod error;
mod isobmff;
mod jpeg;
mod png;
mod tiff;
mod util;
mod webp;

use super::MAX_METADATA_BYTES;
pub use error::Error;
use std::io::{self, BufRead, Read, Seek};

pub fn read(reader: &mut (impl BufRead + Seek)) -> Result<Vec<u8>, Error> {
    let mut header = Vec::new();
    reader.by_ref().take(4096).read_to_end(&mut header)?;
    let bytes = if jpeg::is_jpeg(&header) {
        jpeg::get_exif_attr(&mut header.chain(reader))?
    } else if png::is_png(&header) {
        png::get_exif_attr(&mut header.chain(reader))?
    } else if isobmff::is_heif(&header) {
        reader.seek(io::SeekFrom::Start(0))?;
        isobmff::get_exif_attr(reader)?
    } else if webp::is_webp(&header) {
        webp::get_exif_attr(&mut header.chain(reader))?
    } else {
        return Err(Error::InvalidFormat("Unknown image format"));
    };
    tiff::validate(&bytes)?;
    Ok(bytes)
}
