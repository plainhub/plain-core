#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ColorSpace {
    RGB,
    RGBA,
    YCbCr,
    Luma,
    LumaA,
    YCCK,
    CMYK,
    BGR,
    BGRA,
    Unknown,
    ARGB,
    HSL,
    HSV,
    MultiBand(core::num::NonZeroU32),
}
impl ColorSpace {
    pub const fn num_components(&self) -> usize {
        match self {
            Self::RGB | Self::YCbCr | Self::BGR | Self::HSV | Self::HSL => 3,
            Self::RGBA | Self::YCCK | Self::CMYK | Self::BGRA | Self::ARGB => 4,
            Self::Luma => 1,
            Self::LumaA => 2,
            Self::Unknown => 0,
            Self::MultiBand(n) => n.get() as usize,
        }
    }
    pub const fn has_alpha(&self) -> bool {
        matches!(self, Self::RGBA | Self::LumaA | Self::BGRA | Self::ARGB)
    }
}
pub static ALL_COLORSPACES: [ColorSpace; 12] = [
    ColorSpace::RGB,
    ColorSpace::RGBA,
    ColorSpace::LumaA,
    ColorSpace::Luma,
    ColorSpace::CMYK,
    ColorSpace::BGRA,
    ColorSpace::BGR,
    ColorSpace::YCCK,
    ColorSpace::YCbCr,
    ColorSpace::ARGB,
    ColorSpace::HSL,
    ColorSpace::HSV,
];
