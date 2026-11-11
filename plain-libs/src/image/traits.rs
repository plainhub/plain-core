use crate::image::ExtendedColorType;
use crate::image::color::{Luma, LumaA, Rgb, Rgba};
use num_traits::{Bounded, Num, NumCast};
use std::ops::AddAssign;
pub trait EncodableLayout: seals::EncodableLayout {
    fn as_bytes(&self) -> &[u8];
}
impl EncodableLayout for [u8] {
    fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self)
    }
}
impl EncodableLayout for [u16] {
    fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self)
    }
}
impl EncodableLayout for [f32] {
    fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self)
    }
}
pub trait Primitive: Copy + NumCast + Num + PartialOrd<Self> + Clone + Bounded {
    const DEFAULT_MAX_VALUE: Self;
    const DEFAULT_MIN_VALUE: Self;
}
macro_rules! declare_primitive {
    ($ base : ty : ($ from : expr) ..$ to : expr) => {
        impl Primitive for $base {
            const DEFAULT_MAX_VALUE: Self = $to;
            const DEFAULT_MIN_VALUE: Self = $from;
        }
    };
}
declare_primitive ! (usize : (0) .. Self :: MAX);
declare_primitive ! (u8 : (0) .. Self :: MAX);
declare_primitive ! (u16 : (0) .. Self :: MAX);
declare_primitive ! (u32 : (0) .. Self :: MAX);
declare_primitive ! (u64 : (0) .. Self :: MAX);
declare_primitive ! (isize : (Self :: MIN) .. Self :: MAX);
declare_primitive ! (i8 : (Self :: MIN) .. Self :: MAX);
declare_primitive ! (i16 : (Self :: MIN) .. Self :: MAX);
declare_primitive ! (i32 : (Self :: MIN) .. Self :: MAX);
declare_primitive ! (i64 : (Self :: MIN) .. Self :: MAX);
declare_primitive ! (f32 : (0.0) .. 1.0);
declare_primitive ! (f64 : (0.0) .. 1.0);
pub trait Enlargeable: Sized + Bounded + NumCast {
    type Larger: Copy + NumCast + Num + PartialOrd<Self::Larger> + Clone + Bounded + AddAssign;
    fn clamp_from(n: Self::Larger) -> Self {
        if n > Self::max_value().to_larger() {
            Self::max_value()
        } else if n < Self::min_value().to_larger() {
            Self::min_value()
        } else {
            NumCast::from(n).unwrap()
        }
    }
    fn to_larger(self) -> Self::Larger {
        NumCast::from(self).unwrap()
    }
}
impl Enlargeable for u8 {
    type Larger = u32;
}
impl Enlargeable for u16 {
    type Larger = u32;
}
impl Enlargeable for u32 {
    type Larger = u64;
}
impl Enlargeable for u64 {
    type Larger = u128;
}
impl Enlargeable for usize {
    type Larger = u128;
}
impl Enlargeable for i8 {
    type Larger = i32;
}
impl Enlargeable for i16 {
    type Larger = i32;
}
impl Enlargeable for i32 {
    type Larger = i64;
}
impl Enlargeable for i64 {
    type Larger = i128;
}
impl Enlargeable for isize {
    type Larger = i128;
}
impl Enlargeable for f32 {
    type Larger = f64;
}
impl Enlargeable for f64 {
    type Larger = f64;
}
pub trait Lerp: Bounded + NumCast {
    type Ratio: Primitive;
    fn lerp(a: Self, b: Self, ratio: Self::Ratio) -> Self {
        let a = <Self::Ratio as NumCast>::from(a).unwrap();
        let b = <Self::Ratio as NumCast>::from(b).unwrap();
        let res = a + (b - a) * ratio;
        if res > NumCast::from(Self::max_value()).unwrap() {
            Self::max_value()
        } else if res < NumCast::from(0).unwrap() {
            NumCast::from(0).unwrap()
        } else {
            NumCast::from(res).unwrap()
        }
    }
}
impl Lerp for u8 {
    type Ratio = f32;
}
impl Lerp for u16 {
    type Ratio = f32;
}
impl Lerp for u32 {
    type Ratio = f64;
}
impl Lerp for f32 {
    type Ratio = f32;
    fn lerp(a: Self, b: Self, ratio: Self::Ratio) -> Self {
        a + (b - a) * ratio
    }
}
pub trait PixelWithColorType:
    Pixel + private::SealedPixelWithColorType<TransformableSubpixel = <Self as Pixel>::Subpixel>
{
    const COLOR_TYPE: ExtendedColorType;
}
impl PixelWithColorType for Rgb<u8> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::Rgb8;
}
impl PixelWithColorType for Rgb<u16> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::Rgb16;
}
impl PixelWithColorType for Rgb<f32> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::Rgb32F;
}
impl PixelWithColorType for Rgba<u8> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::Rgba8;
}
impl PixelWithColorType for Rgba<u16> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::Rgba16;
}
impl PixelWithColorType for Rgba<f32> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::Rgba32F;
}
impl PixelWithColorType for Luma<u8> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::L8;
}
impl PixelWithColorType for Luma<u16> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::L16;
}
impl PixelWithColorType for LumaA<u8> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::La8;
}
impl PixelWithColorType for LumaA<u16> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::La16;
}
pub(crate) mod private {
    use crate::image::color::*;
    #[derive(Clone, Copy, Debug)]
    pub enum LayoutWithColor {
        Rgb,
        Rgba,
        Luma,
        LumaAlpha,
    }
    impl From<ColorType> for LayoutWithColor {
        fn from(color: ColorType) -> LayoutWithColor {
            match color {
                ColorType::L8 | ColorType::L16 => LayoutWithColor::Luma,
                ColorType::La8 | ColorType::La16 => LayoutWithColor::LumaAlpha,
                ColorType::Rgb8 | ColorType::Rgb16 | ColorType::Rgb32F => LayoutWithColor::Rgb,
                ColorType::Rgba8 | ColorType::Rgba16 | ColorType::Rgba32F => LayoutWithColor::Rgba,
            }
        }
    }
    impl LayoutWithColor {
        pub(crate) fn channels(self) -> usize {
            match self {
                Self::Rgb => 3,
                Self::Rgba => 4,
                Self::Luma => 1,
                Self::LumaAlpha => 2,
            }
        }
    }
    #[derive(Clone, Copy)]
    pub struct PrivateToken;
    pub trait SealedPixelWithColorType {
        type TransformableSubpixel;
        fn layout(_: PrivateToken) -> LayoutWithColor;
    }
    impl SealedPixelWithColorType for Rgb<u8> {
        type TransformableSubpixel = u8;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Rgb
        }
    }
    impl SealedPixelWithColorType for Rgb<u16> {
        type TransformableSubpixel = u16;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Rgb
        }
    }
    impl SealedPixelWithColorType for Rgb<f32> {
        type TransformableSubpixel = f32;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Rgb
        }
    }
    impl SealedPixelWithColorType for Rgba<u8> {
        type TransformableSubpixel = u8;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Rgba
        }
    }
    impl SealedPixelWithColorType for Rgba<u16> {
        type TransformableSubpixel = u16;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Rgba
        }
    }
    impl SealedPixelWithColorType for Rgba<f32> {
        type TransformableSubpixel = f32;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Rgba
        }
    }
    impl SealedPixelWithColorType for Luma<u8> {
        type TransformableSubpixel = u8;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Luma
        }
    }
    impl SealedPixelWithColorType for LumaA<u8> {
        type TransformableSubpixel = u8;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::LumaAlpha
        }
    }
    impl SealedPixelWithColorType for Luma<u16> {
        type TransformableSubpixel = u16;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Luma
        }
    }
    impl SealedPixelWithColorType for Luma<f32> {
        type TransformableSubpixel = f32;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Luma
        }
    }
    impl SealedPixelWithColorType for LumaA<u16> {
        type TransformableSubpixel = u16;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::LumaAlpha
        }
    }
    impl SealedPixelWithColorType for LumaA<f32> {
        type TransformableSubpixel = f32;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::LumaAlpha
        }
    }
}
pub trait Pixel: Copy + Clone {
    type Subpixel: Primitive;
    const CHANNEL_COUNT: u8;
    fn channels(&self) -> &[Self::Subpixel];
    fn channels_mut(&mut self) -> &mut [Self::Subpixel];
    const COLOR_MODEL: &'static str;
    const HAS_ALPHA: bool = false;
    #[inline]
    fn alpha(&self) -> Self::Subpixel {
        if Self::HAS_ALPHA {
            *self.to_luma_alpha().channels().last().unwrap()
        } else {
            Self::Subpixel::DEFAULT_MAX_VALUE
        }
    }
    fn channels4(
        &self,
    ) -> (
        Self::Subpixel,
        Self::Subpixel,
        Self::Subpixel,
        Self::Subpixel,
    );
    fn from_channels(
        a: Self::Subpixel,
        b: Self::Subpixel,
        c: Self::Subpixel,
        d: Self::Subpixel,
    ) -> Self;
    fn from_slice(slice: &[Self::Subpixel]) -> &Self;
    fn from_slice_mut(slice: &mut [Self::Subpixel]) -> &mut Self;
    fn to_rgb(&self) -> Rgb<Self::Subpixel>;
    fn to_rgba(&self) -> Rgba<Self::Subpixel>;
    fn to_luma(&self) -> Luma<Self::Subpixel>;
    fn to_luma_alpha(&self) -> LumaA<Self::Subpixel>;
    fn map<F>(&self, f: F) -> Self
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel;
    fn apply<F>(&mut self, f: F)
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel;
    fn map_with_alpha<F, G>(&self, f: F, g: G) -> Self
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
        G: FnMut(Self::Subpixel) -> Self::Subpixel;
    fn apply_with_alpha<F, G>(&mut self, f: F, g: G)
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
        G: FnMut(Self::Subpixel) -> Self::Subpixel;
    fn map_without_alpha<F>(&self, f: F) -> Self
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
    {
        let mut this = *self;
        this.apply_with_alpha(f, |x| x);
        this
    }
    fn apply_without_alpha<F>(&mut self, f: F)
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
    {
        self.apply_with_alpha(f, |x| x);
    }
    fn map2<F>(&self, other: &Self, f: F) -> Self
    where
        F: FnMut(Self::Subpixel, Self::Subpixel) -> Self::Subpixel;
    fn apply2<F>(&mut self, other: &Self, f: F)
    where
        F: FnMut(Self::Subpixel, Self::Subpixel) -> Self::Subpixel;
    fn invert(&mut self);
    fn blend(&mut self, other: &Self);
}
mod seals {
    pub trait EncodableLayout {}
    impl EncodableLayout for [u8] {}
    impl EncodableLayout for [u16] {}
    impl EncodableLayout for [f32] {}
}
