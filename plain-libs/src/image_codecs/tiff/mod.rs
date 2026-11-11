pub mod decoder;
mod directory;
#[cfg(all())]
mod fax;

mod error;
pub mod tags;

pub use self::directory::Directory;
pub use self::error::{TiffError, TiffFormatError, TiffResult, TiffUnsupportedError, UsageError};
#[derive(Copy, PartialEq, Eq, Debug, Clone, Hash)]
#[non_exhaustive]
pub enum ColorType {
    Gray(u8),
    RGB(u8),
    Palette(u8),
    GrayA(u8),
    RGBA(u8),
    CMYK(u8),
    CMYKA(u8),
    YCbCr(u8),
    Lab(u8),
    Multiband { bit_depth: u8, num_samples: u16 },
}
impl ColorType {
    pub fn bit_depth(&self) -> u8 {
        match *self {
            ColorType::Gray(b)
            | ColorType::RGB(b)
            | ColorType::Palette(b)
            | ColorType::GrayA(b)
            | ColorType::RGBA(b)
            | ColorType::CMYK(b)
            | ColorType::CMYKA(b)
            | ColorType::YCbCr(b)
            | ColorType::Lab(b)
            | ColorType::Multiband { bit_depth: b, .. } => b,
        }
    }
    pub fn num_samples(&self) -> u16 {
        match *self {
            ColorType::Gray(_) => 1,
            ColorType::RGB(_) => 3,
            ColorType::Palette(_) => 1,
            ColorType::GrayA(_) => 2,
            ColorType::RGBA(_) => 4,
            ColorType::CMYK(_) => 4,
            ColorType::CMYKA(_) => 5,
            ColorType::YCbCr(_) => 3,
            ColorType::Lab(_) => 3,
            ColorType::Multiband { num_samples, .. } => num_samples,
        }
    }
}
