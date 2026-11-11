use crate::image::error::{
    ImageError, ImageFormatHint, ImageResult, LimitError, LimitErrorKind, ParameterError,
    ParameterErrorKind, UnsupportedError, UnsupportedErrorKind,
};
use crate::image::io::encoder::ImageEncoderBoxed;
use crate::image::{DynamicImage, ImageDecoder, ImageFormat};
use crate::image::{ExtendedColorType, ImageReader, codecs::*};
use std::fs::File;
use std::io::{self, BufRead, BufWriter, Seek, Write};
use std::path::Path;
use std::{iter, mem::size_of};
pub fn load<R: BufRead + Seek>(r: R, format: ImageFormat) -> ImageResult<DynamicImage> {
    let mut reader = ImageReader::new(r);
    reader.set_format(format);
    reader.decode()
}
pub fn save_buffer(
    path: impl AsRef<Path>,
    buf: &[u8],
    width: u32,
    height: u32,
    color: impl Into<ExtendedColorType>,
) -> ImageResult<()> {
    let format = ImageFormat::from_path(path.as_ref())?;
    save_buffer_with_format(path, buf, width, height, color, format)
}
pub fn save_buffer_with_format(
    path: impl AsRef<Path>,
    buf: &[u8],
    width: u32,
    height: u32,
    color: impl Into<ExtendedColorType>,
    format: ImageFormat,
) -> ImageResult<()> {
    let buffered_file_write = &mut BufWriter::new(File::create(path)?);
    let encoder = encoder_for_format(format, buffered_file_write)?;
    encoder.write_image(buf, width, height, color.into())
}
pub(crate) fn encoder_for_format<'a, W: Write + Seek>(
    format: ImageFormat,
    buffered_write: &'a mut W,
) -> ImageResult<Box<dyn ImageEncoderBoxed + 'a>> {
    Ok(match format {
        ImageFormat::Png => Box::new(crate::image::codecs::png::PngEncoder::new(buffered_write)),
        ImageFormat::Jpeg => Box::new(jpeg::JpegEncoder::new(buffered_write)),
        ImageFormat::Gif => Box::new(crate::image::codecs::gif::GifEncoder::new(buffered_write)),
        ImageFormat::WebP => Box::new(webp::WebPEncoder::new_lossless(buffered_write)),
        _ => {
            return Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormatHint::Unknown,
                    UnsupportedErrorKind::Format(ImageFormatHint::Name(format!("{format:?}"))),
                ),
            ));
        }
    })
}
static MAGIC_BYTES: [(&[u8], &[u8], ImageFormat); 22] = [
    (b"\x89PNG\r\n\x1a\n", b"", ImageFormat::Png),
    (&[0xff, 0xd8, 0xff], b"", ImageFormat::Jpeg),
    (b"GIF89a", b"", ImageFormat::Gif),
    (b"GIF87a", b"", ImageFormat::Gif),
    (
        b"RIFF\0\0\0\0WEBP",
        b"\xFF\xFF\xFF\xFF\0\0\0\0",
        ImageFormat::WebP,
    ),
    (b"MM\x00*", b"", ImageFormat::Tiff),
    (b"II*\x00", b"", ImageFormat::Tiff),
    (b"DDS ", b"", ImageFormat::Dds),
    (b"BM", b"", ImageFormat::Bmp),
    (&[0, 0, 1, 0], b"", ImageFormat::Ico),
    (b"#?RADIANCE", b"", ImageFormat::Hdr),
    (b"\0\0\0\0ftypavif", b"\xFF\xFF\0\0", ImageFormat::Avif),
    (&[0x76, 0x2f, 0x31, 0x01], b"", ImageFormat::OpenExr),
    (b"qoif", b"", ImageFormat::Qoi),
    (b"P1", b"", ImageFormat::Pnm),
    (b"P2", b"", ImageFormat::Pnm),
    (b"P3", b"", ImageFormat::Pnm),
    (b"P4", b"", ImageFormat::Pnm),
    (b"P5", b"", ImageFormat::Pnm),
    (b"P6", b"", ImageFormat::Pnm),
    (b"P7", b"", ImageFormat::Pnm),
    (b"farbfeld", b"", ImageFormat::Farbfeld),
];
pub fn guess_format(buffer: &[u8]) -> ImageResult<ImageFormat> {
    match guess_format_impl(buffer) {
        Some(format) => Ok(format),
        None => Err(ImageError::Unsupported(ImageFormatHint::Unknown.into())),
    }
}
pub(crate) fn guess_format_impl(buffer: &[u8]) -> Option<ImageFormat> {
    for &(signature, mask, format) in &MAGIC_BYTES {
        if mask.is_empty() {
            if buffer.starts_with(signature) {
                return Some(format);
            }
        } else if buffer.len() >= signature.len()
            && buffer
                .iter()
                .zip(signature.iter())
                .zip(mask.iter().chain(iter::repeat(&0xFF)))
                .all(|((&byte, &sig), &mask)| byte & mask == sig)
        {
            return Some(format);
        }
    }
    None
}

pub(crate) fn decoder_to_vec<T>(decoder: impl ImageDecoder) -> ImageResult<Vec<T>>
where
    T: crate::image::traits::Primitive + crate::byte_cast::Pod,
{
    let total_bytes = usize::try_from(decoder.total_bytes());
    if total_bytes.is_err() || total_bytes.unwrap() > isize::MAX as usize {
        return Err(ImageError::Limits(LimitError::from_kind(
            LimitErrorKind::InsufficientMemory,
        )));
    }
    let mut buf = vec![num_traits::Zero::zero(); total_bytes.unwrap() / size_of::<T>()];
    decoder.read_image(crate::byte_cast::cast_slice_mut(buf.as_mut_slice()))?;
    Ok(buf)
}
