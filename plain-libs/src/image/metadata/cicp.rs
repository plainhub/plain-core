use crate::image::{
    ImageError, Pixel, Primitive,
    color::FromPrimitive,
    error::{ParameterError, ParameterErrorKind},
    math::multiply_accumulate,
    traits::{
        PixelWithColorType,
        private::{LayoutWithColor, SealedPixelWithColorType},
    },
    utils::vec_try_with_capacity,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Cicp {
    pub primaries: CicpColorPrimaries,
    pub transfer: CicpTransferCharacteristics,
    pub matrix: CicpMatrixCoefficients,
    pub full_range: CicpVideoFullRangeFlag,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) struct CicpRgb {
    pub(crate) primaries: CicpColorPrimaries,
    pub(crate) transfer: CicpTransferCharacteristics,
    pub(crate) luminance: DerivedLuminance,
}
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum CicpColorPrimaries {
    SRgb = 1,
    Unspecified = 2,
    RgbM = 4,
    RgbB = 5,
    Bt601 = 6,
    Rgb240m = 7,
    GenericFilm = 8,
    Rgb2020 = 9,
    Xyz = 10,
    SmpteRp431 = 11,
    SmpteRp432 = 12,
    Industry22 = 22,
}
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum CicpTransferCharacteristics {
    Bt709 = 1,
    Unspecified = 2,
    Bt470M = 4,
    Bt470BG = 5,
    Bt601 = 6,
    Smpte240m = 7,
    Linear = 8,
    Log100 = 9,
    LogSqrt = 10,
    Iec61966_2_4 = 11,
    Bt1361 = 12,
    SRgb = 13,
    Bt2020_10bit = 14,
    Bt2020_12bit = 15,
    Smpte2084 = 16,
    Smpte428 = 17,
    Bt2100Hlg = 18,
}
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum CicpMatrixCoefficients {
    Identity = 0,
    Bt709 = 1,
    Unspecified = 2,
    UsFCC = 4,
    Bt470BG = 5,
    Smpte170m = 6,
    Smpte240m = 7,
    YCgCo = 8,
    Bt2020NonConstant = 9,
    Bt2020Constant = 10,
    Smpte2085 = 11,
    ChromaticityDerivedNonConstant = 12,
    ChromaticityDerivedConstant = 13,
    Bt2100 = 14,
    IptPqC2 = 15,
    YCgCoRe = 16,
    YCgCoRo = 17,
}
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum CicpVideoFullRangeFlag {
    NarrowRange = 0,
    FullRange = 1,
}
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum DerivedLuminance {
    Constant,
    NonConstant,
}
struct PixelCast;
impl PixelCast {
    pub(crate) fn expand_luma_rgb<P: ColorComponentForCicp>(luma: &[P], rgb: &mut [f32]) {
        for (&pix, rgb) in luma.iter().zip(rgb.as_chunks_mut::<3>().0.iter_mut()) {
            let luma = pix.expand_to_f32();
            rgb[0] = luma;
            rgb[1] = luma;
            rgb[2] = luma;
        }
    }
    pub(crate) fn expand_luma_rgba<P: ColorComponentForCicp>(luma: &[P], rgb: &mut [f32]) {
        let luma_chunks = luma.as_chunks::<2>().0.iter();
        let rgb_chunks = rgb.as_chunks_mut::<4>().0.iter_mut();
        for (pix, rgb) in luma_chunks.zip(rgb_chunks) {
            let luma = pix[0].expand_to_f32();
            rgb[0] = luma;
            rgb[1] = luma;
            rgb[2] = luma;
            rgb[3] = pix[1].expand_to_f32();
        }
    }
    pub(crate) fn expand_rgb<P: ColorComponentForCicp>(input: &[P], output: &mut [f32]) {
        for (&component, val) in input.iter().zip(output) {
            *val = component.expand_to_f32();
        }
    }
    pub(crate) fn expand_rgba<P: ColorComponentForCicp>(input: &[P], output: &mut [f32]) {
        for (&component, val) in input.iter().zip(output) {
            *val = component.expand_to_f32();
        }
    }
    pub(crate) fn clamp_rgb<P: ColorComponentForCicp>(input: &[f32], output: &mut [P]) {
        for (&component, val) in input.iter().zip(output) {
            *val = P::clamp_from_f32(component);
        }
    }
    pub(crate) fn clamp_rgba<P: ColorComponentForCicp>(input: &[f32], output: &mut [P]) {
        for (&component, val) in input.iter().zip(output) {
            *val = P::clamp_from_f32(component);
        }
    }
    pub(crate) fn clamp_rgb_luma<P: ColorComponentForCicp>(
        input: &[f32],
        output: &mut [P],
        coef: [f32; 3],
    ) {
        for (rgb, pix) in input.as_chunks::<3>().0.iter().zip(output) {
            let mut luma = 0.0;
            for (&component, coef) in rgb.iter().zip(coef) {
                luma = multiply_accumulate(luma, component, coef);
            }
            *pix = P::clamp_from_f32(luma);
        }
    }
    pub(crate) fn clamp_rgba_luma<P: ColorComponentForCicp>(
        input: &[f32],
        output: &mut [P],
        coef: [f32; 3],
    ) {
        let input_chunks = input.as_chunks::<4>().0.iter();
        let output_chunks = output.as_chunks_mut::<2>().0.iter_mut();
        for (rgba, pix) in input_chunks.zip(output_chunks) {
            let mut luma = 0.0;
            for (&component, coef) in rgba[..3].iter().zip(coef) {
                luma = multiply_accumulate(luma, component, coef);
            }
            pix[0] = P::clamp_from_f32(luma);
            pix[1] = P::clamp_from_f32(rgba[3]);
        }
    }
}
impl CicpRgb {
    pub(crate) fn cast_pixels<FromColor, IntoColor>(
        &self,
        buffer: &[FromColor::Subpixel],
        color_space_fallback: &dyn Fn() -> [f32; 3],
    ) -> Vec<IntoColor::Subpixel>
    where
        FromColor: Pixel + SealedPixelWithColorType<TransformableSubpixel = FromColor::Subpixel>,
        IntoColor: Pixel,
        IntoColor: CicpPixelCast<FromColor>,
        FromColor::Subpixel: ColorComponentForCicp,
        IntoColor::Subpixel: ColorComponentForCicp + FromPrimitive<FromColor::Subpixel>,
    {
        use crate::image::traits::private::PrivateToken;
        let from_layout = <FromColor as SealedPixelWithColorType>::layout(PrivateToken);
        let into_layout = <IntoColor as SealedPixelWithColorType>::layout(PrivateToken);
        self.cast_pixels_by_layout(buffer, color_space_fallback, from_layout, into_layout)
    }
    fn cast_pixels_by_layout<FromSubpixel, IntoSubpixel>(
        &self,
        buffer: &[FromSubpixel],
        color_space_fallback: &dyn Fn() -> [f32; 3],
        from_layout: LayoutWithColor,
        into_layout: LayoutWithColor,
    ) -> Vec<IntoSubpixel>
    where
        FromSubpixel: ColorComponentForCicp + Primitive,
        IntoSubpixel: ColorComponentForCicp + FromPrimitive<FromSubpixel> + Primitive,
    {
        let mut output = match self.cast_pixels_from_subpixels(buffer, from_layout, into_layout) {
            Ok(ok) => return ok,
            Err(buffer) => buffer,
        };
        let color_space_coefs = self
            .derived_luminance()
            .unwrap_or_else(color_space_fallback);
        let pixels = buffer.len() / from_layout.channels();
        Self::create_output::<IntoSubpixel>(&mut output, pixels, into_layout);
        Self::cast_pixels_by_fallback(
            buffer,
            output.as_mut_slice(),
            from_layout,
            into_layout,
            color_space_coefs,
        );
        output
    }
    fn create_output<Into: Primitive>(
        output: &mut Vec<Into>,
        pixels: usize,
        into_layout: LayoutWithColor,
    ) {
        output.resize(pixels * into_layout.channels(), Into::DEFAULT_MIN_VALUE);
    }
    fn cast_pixels_by_fallback<
        From: Primitive + ColorComponentForCicp,
        Into: ColorComponentForCicp,
    >(
        buffer: &[From],
        output: &mut [Into],
        from_layout: LayoutWithColor,
        into_layout: LayoutWithColor,
        color_space_coefs: [f32; 3],
    ) {
        use LayoutWithColor as Layout;
        const STEP: usize = 256;
        let pixels = buffer.len() / from_layout.channels();
        let mut ibuffer = [0.0f32; 4 * STEP];
        let mut obuffer = [0.0f32; 4 * STEP];
        let ibuf_step = match from_layout {
            Layout::Rgb | Layout::Luma => 3,
            Layout::Rgba | Layout::LumaAlpha => 4,
        };
        let obuf_step = match into_layout {
            Layout::Rgb | Layout::Luma => 3,
            Layout::Rgba | Layout::LumaAlpha => 4,
        };
        for start_idx in (0..pixels).step_by(STEP) {
            let end_idx = (start_idx + STEP).min(pixels);
            let count = end_idx - start_idx;
            let ibuffer = &mut ibuffer[..ibuf_step * count];
            match from_layout {
                Layout::Rgb => PixelCast::expand_rgb(&buffer[3 * start_idx..3 * end_idx], ibuffer),
                Layout::Rgba => {
                    PixelCast::expand_rgba(&buffer[4 * start_idx..4 * end_idx], ibuffer)
                }
                Layout::Luma => PixelCast::expand_luma_rgb(&buffer[start_idx..end_idx], ibuffer),
                Layout::LumaAlpha => {
                    PixelCast::expand_luma_rgba(&buffer[2 * start_idx..2 * end_idx], ibuffer)
                }
            }
            let obuffer = match (ibuf_step, obuf_step) {
                (3, 4) => {
                    let ibuffer_chunks = ibuffer.as_chunks::<3>().0.iter();
                    let obuffer_chunks = obuffer.as_chunks_mut::<4>().0.iter_mut();
                    for (rgb, rgba) in ibuffer_chunks.zip(obuffer_chunks).take(count) {
                        rgba[0] = rgb[0];
                        rgba[1] = rgb[1];
                        rgba[2] = rgb[2];
                        rgba[3] = 1.0;
                    }
                    &obuffer[..4 * count]
                }
                (4, 3) => {
                    let ibuffer_chunks = ibuffer.as_chunks::<4>().0.iter();
                    let obuffer_chunks = obuffer.as_chunks_mut::<3>().0.iter_mut();
                    for (rgba, rgb) in ibuffer_chunks.zip(obuffer_chunks).take(count) {
                        rgb[0] = rgba[0];
                        rgb[1] = rgba[1];
                        rgb[2] = rgba[2];
                    }
                    &obuffer[..3 * count]
                }
                (n, m) => {
                    debug_assert_eq!(n, m);
                    &ibuffer[..m * count]
                }
            };
            match into_layout {
                Layout::Rgb => {
                    PixelCast::clamp_rgb(obuffer, &mut output[3 * start_idx..3 * end_idx]);
                }
                Layout::Rgba => {
                    PixelCast::clamp_rgba(obuffer, &mut output[4 * start_idx..4 * end_idx]);
                }
                Layout::Luma => {
                    PixelCast::clamp_rgb_luma(
                        obuffer,
                        &mut output[start_idx..end_idx],
                        color_space_coefs,
                    );
                }
                Layout::LumaAlpha => {
                    PixelCast::clamp_rgba_luma(
                        obuffer,
                        &mut output[2 * start_idx..2 * end_idx],
                        color_space_coefs,
                    );
                }
            }
        }
    }
    pub(crate) fn cast_pixels_from_subpixels<FromSubpixel, IntoSubpixel>(
        &self,
        buffer: &[FromSubpixel],
        from_layout: LayoutWithColor,
        into_layout: LayoutWithColor,
    ) -> Result<Vec<IntoSubpixel>, Vec<IntoSubpixel>>
    where
        FromSubpixel: ColorComponentForCicp,
        IntoSubpixel: ColorComponentForCicp + FromPrimitive<FromSubpixel> + Primitive,
    {
        use crate::image::traits::private::LayoutWithColor as Layout;
        assert!(buffer.len().is_multiple_of(from_layout.channels()));
        let pixels = buffer.len() / from_layout.channels();
        let mut output: Vec<IntoSubpixel> = vec_try_with_capacity(pixels * into_layout.channels())
            .expect("input layout already allocated with appropriate layout");
        let map_channel = <IntoSubpixel as FromPrimitive<FromSubpixel>>::from_primitive;
        match (from_layout, into_layout) {
            (Layout::Rgb, Layout::Rgb)
            | (Layout::Rgba, Layout::Rgba)
            | (Layout::Luma, Layout::Luma)
            | (Layout::LumaAlpha, Layout::LumaAlpha) => {
                output.extend(buffer.iter().copied().map(map_channel));
            }
            (Layout::Rgb, Layout::Rgba) => {
                Self::subpixel_cast_rgb_to_rgba(&mut output, buffer);
            }
            (Layout::Rgba, Layout::Rgb) => {
                Self::subpixel_cast_rgba_to_rgb(&mut output, buffer);
            }
            (Layout::Luma, Layout::LumaAlpha) => {
                Self::subpixel_cast_luma_to_luma_alpha(&mut output, buffer);
            }
            (Layout::LumaAlpha, Layout::Luma) => {
                Self::subpixel_cast_luma_alpha_to_luma(&mut output, buffer);
            }
            _ => return Err(output),
        }
        Ok(output)
    }
    fn subpixel_cast_rgb_to_rgba<FromSubpixel, IntoSubpixel>(
        output: &mut Vec<IntoSubpixel>,
        buffer: &[FromSubpixel],
    ) where
        FromSubpixel: ColorComponentForCicp,
        IntoSubpixel: ColorComponentForCicp + FromPrimitive<FromSubpixel> + Primitive,
    {
        let pixels = buffer.len() / LayoutWithColor::Rgb.channels();
        Self::create_output::<IntoSubpixel>(output, pixels, LayoutWithColor::Rgba);
        let map_channel = <IntoSubpixel as FromPrimitive<FromSubpixel>>::from_primitive;
        let default_alpha = <IntoSubpixel as Primitive>::DEFAULT_MAX_VALUE;
        let buffer_chunks = buffer.as_chunks::<3>().0;
        let output_chunks = output.as_chunks_mut::<4>().0;
        for (&[r, g, b], out) in buffer_chunks.iter().zip(output_chunks) {
            *out = [
                map_channel(r),
                map_channel(g),
                map_channel(b),
                default_alpha,
            ];
        }
    }
    fn subpixel_cast_rgba_to_rgb<FromSubpixel, IntoSubpixel>(
        output: &mut Vec<IntoSubpixel>,
        buffer: &[FromSubpixel],
    ) where
        FromSubpixel: ColorComponentForCicp,
        IntoSubpixel: ColorComponentForCicp + FromPrimitive<FromSubpixel> + Primitive,
    {
        let pixels = buffer.len() / LayoutWithColor::Rgba.channels();
        Self::create_output::<IntoSubpixel>(output, pixels, LayoutWithColor::Rgb);
        let map_channel = <IntoSubpixel as FromPrimitive<FromSubpixel>>::from_primitive;
        let buffer_chunks = buffer.as_chunks::<4>().0;
        let output_chunks = output.as_chunks_mut::<3>().0;
        for (&[r, g, b, _], out) in buffer_chunks.iter().zip(output_chunks) {
            *out = [map_channel(r), map_channel(g), map_channel(b)];
        }
    }
    fn subpixel_cast_luma_to_luma_alpha<FromSubpixel, IntoSubpixel>(
        output: &mut Vec<IntoSubpixel>,
        buffer: &[FromSubpixel],
    ) where
        FromSubpixel: ColorComponentForCicp,
        IntoSubpixel: ColorComponentForCicp + FromPrimitive<FromSubpixel> + Primitive,
    {
        let map_channel = <IntoSubpixel as FromPrimitive<FromSubpixel>>::from_primitive;
        output.extend(buffer.iter().copied().flat_map(|l| {
            [
                map_channel(l),
                <IntoSubpixel as Primitive>::DEFAULT_MAX_VALUE,
            ]
        }));
    }
    fn subpixel_cast_luma_alpha_to_luma<FromSubpixel, IntoSubpixel>(
        output: &mut Vec<IntoSubpixel>,
        buffer: &[FromSubpixel],
    ) where
        FromSubpixel: ColorComponentForCicp,
        IntoSubpixel: ColorComponentForCicp + FromPrimitive<FromSubpixel> + Primitive,
    {
        let map_channel = <IntoSubpixel as FromPrimitive<FromSubpixel>>::from_primitive;
        let buffer_chunks = buffer.as_chunks::<2>().0;
        output.extend(buffer_chunks.iter().map(|&[l, _]| map_channel(l)));
    }
}
pub(crate) trait CicpPixelCast<FromColor>
where
    Self: Pixel + SealedPixelWithColorType<TransformableSubpixel = <Self as Pixel>::Subpixel>,
    FromColor:
        Pixel + SealedPixelWithColorType<TransformableSubpixel = <FromColor as Pixel>::Subpixel>,
    Self::Subpixel: ColorComponentForCicp + FromPrimitive<FromColor::Subpixel>,
    FromColor::Subpixel: ColorComponentForCicp,
{
}
impl<FromColor, IntoColor> CicpPixelCast<FromColor> for IntoColor
where
    IntoColor: Pixel + SealedPixelWithColorType<TransformableSubpixel = IntoColor::Subpixel>,
    FromColor: Pixel + SealedPixelWithColorType<TransformableSubpixel = FromColor::Subpixel>,
    IntoColor::Subpixel: ColorComponentForCicp + FromPrimitive<FromColor::Subpixel>,
    FromColor::Subpixel: ColorComponentForCicp,
{
}
pub(crate) trait ColorComponentForCicp: Copy {
    fn expand_to_f32(self) -> f32;
    fn clamp_from_f32(val: f32) -> Self;
}
impl ColorComponentForCicp for u8 {
    fn expand_to_f32(self) -> f32 {
        const R: f32 = 1.0 / u8::MAX as f32;
        self as f32 * R
    }
    #[inline]
    fn clamp_from_f32(val: f32) -> Self {
        (val * Self::MAX as f32).round() as u8
    }
}
impl ColorComponentForCicp for u16 {
    fn expand_to_f32(self) -> f32 {
        const R: f32 = 1.0 / u16::MAX as f32;
        self as f32 * R
    }
    #[inline]
    fn clamp_from_f32(val: f32) -> Self {
        (val * Self::MAX as f32).round() as u16
    }
}
impl ColorComponentForCicp for f32 {
    fn expand_to_f32(self) -> f32 {
        self
    }
    fn clamp_from_f32(val: f32) -> Self {
        val
    }
}
impl Cicp {
    pub const SRGB: Self = Cicp {
        primaries: CicpColorPrimaries::SRgb,
        transfer: CicpTransferCharacteristics::SRgb,
        matrix: CicpMatrixCoefficients::Identity,
        full_range: CicpVideoFullRangeFlag::FullRange,
    };
    pub const SRGB_LINEAR: Self = Cicp {
        primaries: CicpColorPrimaries::SRgb,
        transfer: CicpTransferCharacteristics::Linear,
        matrix: CicpMatrixCoefficients::Identity,
        full_range: CicpVideoFullRangeFlag::FullRange,
    };
    pub const DISPLAY_P3: Self = Cicp {
        primaries: CicpColorPrimaries::SmpteRp432,
        transfer: CicpTransferCharacteristics::SRgb,
        matrix: CicpMatrixCoefficients::Identity,
        full_range: CicpVideoFullRangeFlag::FullRange,
    };
    pub(crate) const fn qualify_stability(&self) -> bool {
        const _: () = {
            assert!(Cicp::SRGB.qualify_stability());
            assert!(Cicp::SRGB_LINEAR.qualify_stability());
            assert!(Cicp::DISPLAY_P3.qualify_stability());
        };
        matches!(self.full_range, CicpVideoFullRangeFlag::FullRange)
            && matches!(
                self.matrix,
                CicpMatrixCoefficients::Identity
                    | CicpMatrixCoefficients::ChromaticityDerivedNonConstant
            )
            && matches!(
                self.primaries,
                CicpColorPrimaries::SRgb
                    | CicpColorPrimaries::SmpteRp431
                    | CicpColorPrimaries::SmpteRp432
                    | CicpColorPrimaries::Bt601
                    | CicpColorPrimaries::Rgb240m
            )
            && matches!(
                self.transfer,
                CicpTransferCharacteristics::SRgb
                    | CicpTransferCharacteristics::Bt709
                    | CicpTransferCharacteristics::Bt601
                    | CicpTransferCharacteristics::Linear
            )
    }
    pub(crate) const fn into_rgb(self) -> CicpRgb {
        CicpRgb {
            primaries: self.primaries,
            transfer: self.transfer,
            luminance: DerivedLuminance::NonConstant,
        }
    }
    pub(crate) fn try_into_rgb(self) -> Result<CicpRgb, ImageError> {
        if Cicp::from(self.into_rgb()) != self {
            Err(ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::RgbCicpRequired(self),
            )))
        } else {
            Ok(self.into_rgb())
        }
    }
}
impl CicpRgb {
    pub(crate) fn derived_luminance(&self) -> Option<[f32; 3]> {
        let primaries = match self.primaries {
            CicpColorPrimaries::SRgb => super::luminance::ColorPrimaries::BT_709,
            CicpColorPrimaries::RgbM => super::luminance::ColorPrimaries::BT_470M,
            CicpColorPrimaries::RgbB => super::luminance::ColorPrimaries::BT_470BG,
            CicpColorPrimaries::Bt601 => super::luminance::ColorPrimaries::BT_601,
            CicpColorPrimaries::Rgb240m => super::luminance::ColorPrimaries::SMPTE_240,
            CicpColorPrimaries::GenericFilm => super::luminance::ColorPrimaries::GENERIC_FILM,
            CicpColorPrimaries::Rgb2020 => super::luminance::ColorPrimaries::BT_2020,
            CicpColorPrimaries::Xyz => super::luminance::ColorPrimaries::XYZ,
            CicpColorPrimaries::SmpteRp431 => super::luminance::ColorPrimaries::DISPLAY_P3,
            CicpColorPrimaries::SmpteRp432 => super::luminance::ColorPrimaries::DISPLAY_P3,
            CicpColorPrimaries::Industry22 => super::luminance::ColorPrimaries::EBU_3213,
            CicpColorPrimaries::Unspecified => return None,
        };
        const ILLUMINANT_C: super::luminance::Chromaticity =
            super::luminance::Chromaticity::new(0.310, 0.316);
        let whitepoint = match self.primaries {
            CicpColorPrimaries::SRgb => super::luminance::Chromaticity::D65,
            CicpColorPrimaries::RgbM => ILLUMINANT_C,
            CicpColorPrimaries::RgbB => super::luminance::Chromaticity::D65,
            CicpColorPrimaries::Bt601 => super::luminance::Chromaticity::D65,
            CicpColorPrimaries::Rgb240m => super::luminance::Chromaticity::D65,
            CicpColorPrimaries::GenericFilm => ILLUMINANT_C,
            CicpColorPrimaries::Rgb2020 => super::luminance::Chromaticity::D65,
            CicpColorPrimaries::Xyz => super::luminance::Chromaticity::new(1. / 3., 1. / 3.),
            CicpColorPrimaries::SmpteRp431 => super::luminance::Chromaticity::new(0.314, 0.351),
            CicpColorPrimaries::SmpteRp432 => super::luminance::Chromaticity::D65,
            CicpColorPrimaries::Industry22 => super::luminance::Chromaticity::D65,
            CicpColorPrimaries::Unspecified => return None,
        };
        let matrix = primaries.transform_to_xyz(whitepoint);
        Some(matrix.v[1])
    }
}
impl From<CicpRgb> for Cicp {
    fn from(cicp: CicpRgb) -> Self {
        Cicp {
            primaries: cicp.primaries,
            transfer: cicp.transfer,
            matrix: CicpMatrixCoefficients::Identity,
            full_range: CicpVideoFullRangeFlag::FullRange,
        }
    }
}
