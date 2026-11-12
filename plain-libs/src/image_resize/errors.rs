
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImageError {
    UnsupportedPixelType,
}

#[derive(Debug, Clone, Copy)]
pub struct InvalidPixelsSize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageBufferError {
    InvalidBufferSize,
    InvalidBufferAlignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropBoxError {
    PositionIsOutOfImageBoundaries,
    SizeIsOutOfImageBoundaries,
    WidthOrHeightLessThanZero,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResizeError {
    ImageError(ImageError),
    PixelTypesAreDifferent,
}

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPixelType => f.write_str("Pixel type of image is not supported"),
        }
    }
}

impl std::error::Error for ImageError {}

impl std::fmt::Display for InvalidPixelsSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self => f.write_str("Size of container with pixels is smaller than required"),
        }
    }
}

impl std::error::Error for InvalidPixelsSize {}

impl std::fmt::Display for ImageBufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBufferSize => f.write_str("Size of buffer is smaller than required"),
            Self::InvalidBufferAlignment => {
                f.write_str("Alignment of buffer don't match to alignment of required pixel type")
            }
        }
    }
}

impl std::error::Error for ImageBufferError {}

impl std::fmt::Display for CropBoxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PositionIsOutOfImageBoundaries => {
                f.write_str("Position of the crop box is out of the image boundaries")
            }
            Self::SizeIsOutOfImageBoundaries => {
                f.write_str("Size of the crop box is out of the image boundaries")
            }
            Self::WidthOrHeightLessThanZero => {
                f.write_str("Width or height of the crop box is less than zero")
            }
        }
    }
}

impl std::error::Error for CropBoxError {}

impl std::fmt::Display for ResizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ImageError(inner) => f.write_str("Source or destination image is not supported"),
            Self::PixelTypesAreDifferent => {
                f.write_str("Pixel type of source image does not match to destination image")
            }
        }
    }
}

impl std::error::Error for ResizeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ImageError(inner) => Some(inner),
            _ => None,
        }
    }
}

impl From<ImageError> for ResizeError {
    fn from(inner: ImageError) -> Self {
        Self::ImageError(inner)
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/image_resize/errors.rs"]
mod tests;
