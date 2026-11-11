use crate::image::error::{ImageError, ImageFormatHint, ImageResult};
use std::ffi::OsStr;
use std::path::Path;
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(any(), derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    WebP,
    Pnm,
    Tiff,
    Tga,
    Dds,
    Bmp,
    Ico,
    Hdr,
    OpenExr,
    Farbfeld,
    Avif,
    Qoi,
    Pcx,
}
impl ImageFormat {
    #[inline]
    pub fn from_extension<S>(ext: S) -> Option<Self>
    where
        S: AsRef<OsStr>,
    {
        fn inner(ext: &OsStr) -> Option<ImageFormat> {
            let ext = ext.to_str()?.to_ascii_lowercase();
            Some(match ext.as_str() {
                "avif" => ImageFormat::Avif,
                "jpg" | "jpeg" | "jfif" => ImageFormat::Jpeg,
                "png" | "apng" => ImageFormat::Png,
                "gif" => ImageFormat::Gif,
                "webp" => ImageFormat::WebP,
                "tif" | "tiff" => ImageFormat::Tiff,
                "tga" => ImageFormat::Tga,
                "dds" => ImageFormat::Dds,
                "bmp" => ImageFormat::Bmp,
                "ico" => ImageFormat::Ico,
                "hdr" => ImageFormat::Hdr,
                "exr" => ImageFormat::OpenExr,
                "pbm" | "pam" | "ppm" | "pgm" | "pnm" => ImageFormat::Pnm,
                "ff" => ImageFormat::Farbfeld,
                "qoi" => ImageFormat::Qoi,
                _ => return None,
            })
        }
        inner(ext.as_ref())
    }
    #[inline]
    pub fn from_path<P>(path: P) -> ImageResult<Self>
    where
        P: AsRef<Path>,
    {
        fn inner(path: &Path) -> ImageResult<ImageFormat> {
            let exact_ext = path.extension();
            exact_ext
                .and_then(ImageFormat::from_extension)
                .ok_or_else(|| {
                    let format_hint = match exact_ext {
                        None => ImageFormatHint::Unknown,
                        Some(os) => ImageFormatHint::PathExtension(os.into()),
                    };
                    ImageError::Unsupported(format_hint.into())
                })
        }
        inner(path.as_ref())
    }
    pub fn from_mime_type<M>(mime_type: M) -> Option<Self>
    where
        M: AsRef<str>,
    {
        match mime_type.as_ref() {
            "image/avif" => Some(ImageFormat::Avif),
            "image/jpeg" => Some(ImageFormat::Jpeg),
            "image/png" => Some(ImageFormat::Png),
            "image/gif" => Some(ImageFormat::Gif),
            "image/webp" => Some(ImageFormat::WebP),
            "image/tiff" => Some(ImageFormat::Tiff),
            "image/x-targa" | "image/x-tga" => Some(ImageFormat::Tga),
            "image/vnd-ms.dds" => Some(ImageFormat::Dds),
            "image/bmp" => Some(ImageFormat::Bmp),
            "image/x-icon" | "image/vnd.microsoft.icon" => Some(ImageFormat::Ico),
            "image/vnd.radiance" => Some(ImageFormat::Hdr),
            "image/x-exr" => Some(ImageFormat::OpenExr),
            "image/x-portable-bitmap"
            | "image/x-portable-graymap"
            | "image/x-portable-pixmap"
            | "image/x-portable-anymap" => Some(ImageFormat::Pnm),
            "image/x-qoi" => Some(ImageFormat::Qoi),
            _ => None,
        }
    }

    pub fn all() -> impl Iterator<Item = ImageFormat> {
        [
            ImageFormat::Gif,
            ImageFormat::Ico,
            ImageFormat::Jpeg,
            ImageFormat::Png,
            ImageFormat::Bmp,
            ImageFormat::Tiff,
            ImageFormat::Tga,
            ImageFormat::Pnm,
            ImageFormat::Farbfeld,
            ImageFormat::Avif,
            ImageFormat::WebP,
            ImageFormat::OpenExr,
            ImageFormat::Qoi,
            ImageFormat::Dds,
            ImageFormat::Hdr,
            ImageFormat::Pcx,
        ]
        .iter()
        .copied()
    }
}
