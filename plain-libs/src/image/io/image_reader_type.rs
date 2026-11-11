use super::free_functions;
use crate::image::error::{ImageFormatHint, ImageResult, UnsupportedError, UnsupportedErrorKind};
use crate::image::io::limits::Limits;
use crate::image::{DynamicImage, ImageDecoder, ImageError, ImageFormat};
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Cursor, Read, Seek, SeekFrom};
use std::iter;
use std::path::Path;
#[derive(Clone)]
enum Format {
    BuiltIn(ImageFormat),
    Extension(OsString),
}
pub struct ImageReader<R: Read + Seek> {
    inner: R,
    format: Option<Format>,
    limits: Limits,
}
impl<'a, R: 'a + BufRead + Seek> ImageReader<R> {
    pub fn new(buffered_reader: R) -> Self {
        ImageReader {
            inner: buffered_reader,
            format: None,
            limits: Limits::default(),
        }
    }

    pub fn format(&self) -> Option<ImageFormat> {
        match self.format {
            Some(Format::BuiltIn(ref format)) => Some(*format),
            Some(Format::Extension(ref ext)) => ImageFormat::from_extension(ext),
            None => None,
        }
    }
    pub fn set_format(&mut self, format: ImageFormat) {
        self.format = Some(Format::BuiltIn(format));
    }

    pub fn no_limits(&mut self) {
        self.limits = Limits::no_limits();
    }
    pub fn limits(&mut self, limits: Limits) {
        self.limits = limits;
    }
    pub fn into_inner(self) -> R {
        self.inner
    }
    fn make_decoder(
        format: Format,
        reader: R,
        limits_for_png: Limits,
    ) -> ImageResult<Box<dyn ImageDecoder + 'a>> {
        use crate::image::codecs::*;
        let format = match format {
            Format::BuiltIn(format) => format,
            Format::Extension(ext) => ImageFormat::from_extension(&ext).ok_or(
                ImageError::Unsupported(ImageFormatHint::PathExtension(ext.into()).into()),
            )?,
        };
        Ok(match format {
            ImageFormat::Png => Box::new(crate::image::codecs::png::PngDecoder::with_limits(
                reader,
                limits_for_png,
            )?),
            ImageFormat::Gif => Box::new(crate::image::codecs::gif::GifDecoder::new(reader)?),
            ImageFormat::Jpeg => Box::new(jpeg::JpegDecoder::new(reader)?),
            ImageFormat::WebP => Box::new(webp::WebPDecoder::new(reader)?),
            format => {
                return Err(ImageError::Unsupported(
                    ImageFormatHint::Exact(format).into(),
                ));
            }
        })
    }
    pub fn into_decoder(mut self) -> ImageResult<impl ImageDecoder + 'a> {
        let mut decoder =
            Self::make_decoder(self.require_format()?, self.inner, self.limits.clone())?;
        decoder.set_limits(self.limits)?;
        Ok(decoder)
    }
    pub fn with_guessed_format(mut self) -> io::Result<Self> {
        let format = self.guess_format()?;
        self.format = format.or(self.format);
        Ok(self)
    }
    fn guess_format(&mut self) -> io::Result<Option<Format>> {
        let mut start = [0; 16];
        let cur = self.inner.stream_position()?;
        let len = io::copy(
            &mut self.inner.by_ref().take(16),
            &mut Cursor::new(&mut start[..]),
        )?;
        self.inner.seek(SeekFrom::Start(cur))?;
        if let Some(format) = free_functions::guess_format_impl(&start[..len as usize]) {
            return Ok(Some(Format::BuiltIn(format)));
        }
        Ok(None)
    }
    pub fn into_dimensions(self) -> ImageResult<(u32, u32)> {
        self.into_decoder().map(|d| d.dimensions())
    }
    pub fn decode(mut self) -> ImageResult<DynamicImage> {
        let format = self.require_format()?;
        let mut limits = self.limits;
        let mut decoder = Self::make_decoder(format, self.inner, limits.clone())?;
        limits.reserve(decoder.total_bytes())?;
        decoder.set_limits(limits)?;
        DynamicImage::from_decoder(decoder)
    }
    fn require_format(&mut self) -> ImageResult<Format> {
        self.format.clone().ok_or_else(|| {
            ImageError::Unsupported(UnsupportedError::from_format_and_kind(
                ImageFormatHint::Unknown,
                UnsupportedErrorKind::Format(ImageFormatHint::Unknown),
            ))
        })
    }
}
impl ImageReader<BufReader<File>> {
    pub fn open<P>(path: P) -> io::Result<Self>
    where
        P: AsRef<Path>,
    {
        Self::open_impl(path.as_ref())
    }
    fn open_impl(path: &Path) -> io::Result<Self> {
        let format = path
            .extension()
            .filter(|ext| !ext.is_empty())
            .map(|ext| Format::Extension(ext.to_owned()));
        Ok(ImageReader {
            inner: BufReader::new(File::open(path)?),
            format,
            limits: Limits::default(),
        })
    }
}
