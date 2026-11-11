use crate::image_codecs::png::Filter;
use crate::image_codecs::png::text_metadata::{ITXtChunk, TEXtChunk, ZTXtChunk};
use crate::image_codecs::png::{chunk, encoder};
use io::Write;
use std::{borrow::Cow, convert::TryFrom, fmt, io};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ColorType {
    Grayscale = 0,
    Rgb = 2,
    Indexed = 3,
    GrayscaleAlpha = 4,
    Rgba = 6,
}
impl ColorType {
    pub fn samples(self) -> usize {
        self.samples_u8().into()
    }
    pub(crate) fn samples_u8(self) -> u8 {
        use self::ColorType::*;
        match self {
            Grayscale | Indexed => 1,
            Rgb => 3,
            GrayscaleAlpha => 2,
            Rgba => 4,
        }
    }
    pub fn from_u8(n: u8) -> Option<ColorType> {
        match n {
            0 => Some(ColorType::Grayscale),
            2 => Some(ColorType::Rgb),
            3 => Some(ColorType::Indexed),
            4 => Some(ColorType::GrayscaleAlpha),
            6 => Some(ColorType::Rgba),
            _ => None,
        }
    }
    pub(crate) fn checked_raw_row_length(self, depth: BitDepth, width: u32) -> Option<usize> {
        let bits = u64::from(width) * u64::from(self.samples_u8()) * u64::from(depth.into_u8());
        TryFrom::try_from(1 + bits.div_ceil(8)).ok()
    }
    pub(crate) fn raw_row_length_from_width(self, depth: BitDepth, width: u32) -> usize {
        let samples = width as usize * self.samples();
        1 + match depth {
            BitDepth::Sixteen => samples * 2,
            BitDepth::Eight => samples,
            subbyte => {
                let samples_per_byte = 8 / subbyte as usize;
                let whole = samples / samples_per_byte;
                let fract = usize::from(samples % samples_per_byte > 0);
                whole + fract
            }
        }
    }
    pub(crate) fn is_combination_invalid(self, bit_depth: BitDepth) -> bool {
        ((bit_depth == BitDepth::One || bit_depth == BitDepth::Two || bit_depth == BitDepth::Four)
            && (self == ColorType::Rgb
                || self == ColorType::GrayscaleAlpha
                || self == ColorType::Rgba))
            || (bit_depth == BitDepth::Sixteen && self == ColorType::Indexed)
    }
    pub(crate) fn bits_per_pixel(&self, bit_depth: BitDepth) -> usize {
        self.samples() * bit_depth as usize
    }
    pub(crate) fn bytes_per_pixel(&self, bit_depth: BitDepth) -> usize {
        self.samples() * ((bit_depth as usize + 7) >> 3)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BitDepth {
    One = 1,
    Two = 2,
    Four = 4,
    Eight = 8,
    Sixteen = 16,
}
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub(crate) enum BytesPerPixel {
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
    Six = 6,
    Eight = 8,
}
impl BitDepth {
    pub fn from_u8(n: u8) -> Option<BitDepth> {
        match n {
            1 => Some(BitDepth::One),
            2 => Some(BitDepth::Two),
            4 => Some(BitDepth::Four),
            8 => Some(BitDepth::Eight),
            16 => Some(BitDepth::Sixteen),
            _ => None,
        }
    }
    pub(crate) fn into_u8(self) -> u8 {
        self as u8
    }
}
#[derive(Clone, Copy, Debug)]
pub struct PixelDimensions {
    pub xppu: u32,
    pub yppu: u32,
    pub unit: Unit,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Unit {
    Unspecified = 0,
    Meter = 1,
}
impl Unit {
    pub fn from_u8(n: u8) -> Option<Unit> {
        match n {
            0 => Some(Unit::Unspecified),
            1 => Some(Unit::Meter),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DisposeOp {
    None = 0,
    Background = 1,
    Previous = 2,
}
impl DisposeOp {
    pub fn from_u8(n: u8) -> Option<DisposeOp> {
        match n {
            0 => Some(DisposeOp::None),
            1 => Some(DisposeOp::Background),
            2 => Some(DisposeOp::Previous),
            _ => None,
        }
    }
}
impl fmt::Display for DisposeOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match *self {
            DisposeOp::None => "DISPOSE_OP_NONE",
            DisposeOp::Background => "DISPOSE_OP_BACKGROUND",
            DisposeOp::Previous => "DISPOSE_OP_PREVIOUS",
        };
        write!(f, "{}", name)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BlendOp {
    Source = 0,
    Over = 1,
}
impl BlendOp {
    pub fn from_u8(n: u8) -> Option<BlendOp> {
        match n {
            0 => Some(BlendOp::Source),
            1 => Some(BlendOp::Over),
            _ => None,
        }
    }
}
impl fmt::Display for BlendOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match *self {
            BlendOp::Source => "BLEND_OP_SOURCE",
            BlendOp::Over => "BLEND_OP_OVER",
        };
        write!(f, "{}", name)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct FrameControl {
    pub sequence_number: u32,
    pub width: u32,
    pub height: u32,
    pub x_offset: u32,
    pub y_offset: u32,
    pub delay_num: u16,
    pub delay_den: u16,
    pub dispose_op: DisposeOp,
    pub blend_op: BlendOp,
}
impl Default for FrameControl {
    fn default() -> FrameControl {
        FrameControl {
            sequence_number: 0,
            width: 0,
            height: 0,
            x_offset: 0,
            y_offset: 0,
            delay_num: 1,
            delay_den: 30,
            dispose_op: DisposeOp::None,
            blend_op: BlendOp::Source,
        }
    }
}
impl FrameControl {
    pub fn encode<W: Write>(self, w: &mut W) -> encoder::Result<()> {
        let mut data = [0u8; 26];
        data[..4].copy_from_slice(&self.sequence_number.to_be_bytes());
        data[4..8].copy_from_slice(&self.width.to_be_bytes());
        data[8..12].copy_from_slice(&self.height.to_be_bytes());
        data[12..16].copy_from_slice(&self.x_offset.to_be_bytes());
        data[16..20].copy_from_slice(&self.y_offset.to_be_bytes());
        data[20..22].copy_from_slice(&self.delay_num.to_be_bytes());
        data[22..24].copy_from_slice(&self.delay_den.to_be_bytes());
        data[24] = self.dispose_op as u8;
        data[25] = self.blend_op as u8;
        encoder::write_chunk(w, chunk::fcTL, &data)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct AnimationControl {
    pub num_frames: u32,
    pub num_plays: u32,
}
impl AnimationControl {
    pub fn encode<W: Write>(self, w: &mut W) -> encoder::Result<()> {
        let mut data = [0; 8];
        data[..4].copy_from_slice(&self.num_frames.to_be_bytes());
        data[4..].copy_from_slice(&self.num_plays.to_be_bytes());
        encoder::write_chunk(w, chunk::acTL, &data)
    }
}
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum Compression {
    NoCompression,
    Fastest,
    Fast,
    Balanced,
    High,
}
impl Default for Compression {
    fn default() -> Self {
        Self::Balanced
    }
}
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub enum DeflateCompression {
    NoCompression,
    FdeflateUltraFast,
    Level(u8),
}
impl Default for DeflateCompression {
    fn default() -> Self {
        Self::from_simple(Compression::Balanced)
    }
}
impl DeflateCompression {
    pub(crate) fn from_simple(value: Compression) -> Self {
        match value {
            Compression::NoCompression => Self::NoCompression,
            Compression::Fastest => Self::FdeflateUltraFast,
            Compression::Fast => Self::FdeflateUltraFast,
            Compression::Balanced => Self::Level(flate2::Compression::default().level() as u8),
            Compression::High => Self::Level(flate2::Compression::best().level() as u8),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScaledFloat(u32);
impl ScaledFloat {
    pub const SCALING: f32 = 100_000.0;
    pub fn in_range(value: f32) -> bool {
        value >= 0.0 && (value * Self::SCALING).floor() <= u32::MAX as f32
    }
    pub fn exact(value: f32) -> bool {
        let there = Self::forward(value);
        let back = Self::reverse(there);
        value == back
    }
    fn forward(value: f32) -> u32 {
        (value.max(0.0) * Self::SCALING).floor() as u32
    }
    fn reverse(encoded: u32) -> f32 {
        encoded as f32 / Self::SCALING
    }
    pub fn new(value: f32) -> Self {
        Self(Self::forward(value))
    }
    pub fn from_scaled(val: u32) -> Self {
        Self(val)
    }
    pub fn into_scaled(self) -> u32 {
        self.0
    }
    pub fn into_value(self) -> f32 {
        Self::reverse(self.0)
    }
    pub(crate) fn encode_gama<W: Write>(self, w: &mut W) -> encoder::Result<()> {
        encoder::write_chunk(w, chunk::gAMA, &self.into_scaled().to_be_bytes())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceChromaticities {
    pub white: (ScaledFloat, ScaledFloat),
    pub red: (ScaledFloat, ScaledFloat),
    pub green: (ScaledFloat, ScaledFloat),
    pub blue: (ScaledFloat, ScaledFloat),
}
impl SourceChromaticities {
    pub fn new(white: (f32, f32), red: (f32, f32), green: (f32, f32), blue: (f32, f32)) -> Self {
        SourceChromaticities {
            white: (ScaledFloat::new(white.0), ScaledFloat::new(white.1)),
            red: (ScaledFloat::new(red.0), ScaledFloat::new(red.1)),
            green: (ScaledFloat::new(green.0), ScaledFloat::new(green.1)),
            blue: (ScaledFloat::new(blue.0), ScaledFloat::new(blue.1)),
        }
    }    # [rustfmt :: skip]    pub fn to_be_bytes (self) -> [u8 ; 32] { let white_x = self . white . 0 . into_scaled () . to_be_bytes () ; let white_y = self . white . 1 . into_scaled () . to_be_bytes () ; let red_x = self . red . 0 . into_scaled () . to_be_bytes () ; let red_y = self . red . 1 . into_scaled () . to_be_bytes () ; let green_x = self . green . 0 . into_scaled () . to_be_bytes () ; let green_y = self . green . 1 . into_scaled () . to_be_bytes () ; let blue_x = self . blue . 0 . into_scaled () . to_be_bytes () ; let blue_y = self . blue . 1 . into_scaled () . to_be_bytes () ; [white_x [0] , white_x [1] , white_x [2] , white_x [3] , white_y [0] , white_y [1] , white_y [2] , white_y [3] , red_x [0] , red_x [1] , red_x [2] , red_x [3] , red_y [0] , red_y [1] , red_y [2] , red_y [3] , green_x [0] , green_x [1] , green_x [2] , green_x [3] , green_y [0] , green_y [1] , green_y [2] , green_y [3] , blue_x [0] , blue_x [1] , blue_x [2] , blue_x [3] , blue_y [0] , blue_y [1] , blue_y [2] , blue_y [3] ,] }
    pub fn encode<W: Write>(self, w: &mut W) -> encoder::Result<()> {
        encoder::write_chunk(w, chunk::cHRM, &self.to_be_bytes())
    }
}
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SrgbRenderingIntent {
    Perceptual = 0,
    RelativeColorimetric = 1,
    Saturation = 2,
    AbsoluteColorimetric = 3,
}
impl SrgbRenderingIntent {
    pub(crate) fn into_raw(self) -> u8 {
        self as u8
    }
    pub(crate) fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(SrgbRenderingIntent::Perceptual),
            1 => Some(SrgbRenderingIntent::RelativeColorimetric),
            2 => Some(SrgbRenderingIntent::Saturation),
            3 => Some(SrgbRenderingIntent::AbsoluteColorimetric),
            _ => None,
        }
    }
    pub fn encode<W: Write>(self, w: &mut W) -> encoder::Result<()> {
        encoder::write_chunk(w, chunk::sRGB, &[self.into_raw()])
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodingIndependentCodePoints {
    pub color_primaries: u8,
    pub transfer_function: u8,
    pub matrix_coefficients: u8,
    pub is_video_full_range_image: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MasteringDisplayColorVolume {
    pub chromaticities: SourceChromaticities,
    pub max_luminance: u32,
    pub min_luminance: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContentLightLevelInfo {
    pub max_content_light_level: u32,
    pub max_frame_average_light_level: u32,
}
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Info<'a> {
    pub width: u32,
    pub height: u32,
    pub bit_depth: BitDepth,
    pub color_type: ColorType,
    pub interlaced: bool,
    pub sbit: Option<Cow<'a, [u8]>>,
    pub trns: Option<Cow<'a, [u8]>>,
    pub pixel_dims: Option<PixelDimensions>,
    pub palette: Option<Cow<'a, [u8]>>,
    pub gama_chunk: Option<ScaledFloat>,
    pub chrm_chunk: Option<SourceChromaticities>,
    pub bkgd: Option<Cow<'a, [u8]>>,
    pub frame_control: Option<FrameControl>,
    pub animation_control: Option<AnimationControl>,
    pub source_gamma: Option<ScaledFloat>,
    pub source_chromaticities: Option<SourceChromaticities>,
    pub srgb: Option<SrgbRenderingIntent>,
    pub icc_profile: Option<Cow<'a, [u8]>>,
    pub coding_independent_code_points: Option<CodingIndependentCodePoints>,
    pub mastering_display_color_volume: Option<MasteringDisplayColorVolume>,
    pub content_light_level: Option<ContentLightLevelInfo>,
    pub exif_metadata: Option<Cow<'a, [u8]>>,
    pub uncompressed_latin1_text: Vec<TEXtChunk>,
    pub compressed_latin1_text: Vec<ZTXtChunk>,
    pub utf8_text: Vec<ITXtChunk>,
}
impl Default for Info<'_> {
    fn default() -> Info<'static> {
        Info {
            width: 0,
            height: 0,
            bit_depth: BitDepth::Eight,
            color_type: ColorType::Grayscale,
            interlaced: false,
            palette: None,
            sbit: None,
            trns: None,
            gama_chunk: None,
            chrm_chunk: None,
            bkgd: None,
            pixel_dims: None,
            frame_control: None,
            animation_control: None,
            source_gamma: None,
            source_chromaticities: None,
            srgb: None,
            icc_profile: None,
            coding_independent_code_points: None,
            mastering_display_color_volume: None,
            content_light_level: None,
            exif_metadata: None,
            uncompressed_latin1_text: Vec::new(),
            compressed_latin1_text: Vec::new(),
            utf8_text: Vec::new(),
        }
    }
}
impl Info<'_> {
    pub fn with_size(width: u32, height: u32) -> Self {
        Info {
            width,
            height,
            ..Default::default()
        }
    }
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn is_animated(&self) -> bool {
        self.frame_control.is_some() && self.animation_control.is_some()
    }
    pub fn animation_control(&self) -> Option<&AnimationControl> {
        self.animation_control.as_ref()
    }
    pub fn frame_control(&self) -> Option<&FrameControl> {
        self.frame_control.as_ref()
    }
    pub fn bits_per_pixel(&self) -> usize {
        self.color_type.bits_per_pixel(self.bit_depth)
    }
    pub fn bytes_per_pixel(&self) -> usize {
        self.color_type.bytes_per_pixel(self.bit_depth)
    }
    pub(crate) fn bpp_in_prediction(&self) -> BytesPerPixel {
        BytesPerPixel::from_usize(self.bytes_per_pixel())
    }
    pub fn raw_bytes(&self) -> usize {
        self.height as usize * self.raw_row_length()
    }
    pub fn raw_row_length(&self) -> usize {
        self.raw_row_length_from_width(self.width)
    }
    pub(crate) fn checked_raw_row_length(&self) -> Option<usize> {
        self.color_type
            .checked_raw_row_length(self.bit_depth, self.width)
    }
    pub fn raw_row_length_from_width(&self, width: u32) -> usize {
        self.color_type
            .raw_row_length_from_width(self.bit_depth, width)
    }
    pub fn gamma(&self) -> Option<ScaledFloat> {
        if self.srgb.is_some() {
            Some(crate::image_codecs::png::srgb::substitute_gamma())
        } else {
            self.gama_chunk
        }
    }
    pub fn chromaticities(&self) -> Option<SourceChromaticities> {
        if self.srgb.is_some() {
            Some(crate::image_codecs::png::srgb::substitute_chromaticities())
        } else {
            self.chrm_chunk
        }
    }
    pub(crate) fn set_source_srgb(&mut self, rendering_intent: SrgbRenderingIntent) {
        self.srgb = Some(rendering_intent);
        self.icc_profile = None;
    }
}
impl BytesPerPixel {
    pub(crate) fn from_usize(bpp: usize) -> Self {
        match bpp {
            1 => BytesPerPixel::One,
            2 => BytesPerPixel::Two,
            3 => BytesPerPixel::Three,
            4 => BytesPerPixel::Four,
            6 => BytesPerPixel::Six,
            8 => BytesPerPixel::Eight,
            _ => unreachable!("Not a possible byte rounded pixel width"),
        }
    }
    pub(crate) fn into_usize(self) -> usize {
        self as usize
    }
}
bitflags::bitflags! { # [derive (Clone , Copy , Debug , Eq , Hash , Ord , PartialEq , PartialOrd)] pub struct Transformations : u32 { const IDENTITY = 0x00000 ; const STRIP_16 = 0x00001 ; const EXPAND = 0x00010 ; const ALPHA = 0x10000 ; } }
impl Transformations {}
impl Default for Transformations {
    fn default() -> Transformations {
        Transformations::IDENTITY
    }
}
#[derive(Debug)]
pub struct ParameterError {
    inner: ParameterErrorKind,
}
#[derive(Debug)]
pub(crate) enum ParameterErrorKind {
    ImageBufferSize { expected: usize, actual: usize },
    PolledAfterEndOfImage,
    PolledAfterFatalError,
}
impl From<ParameterErrorKind> for ParameterError {
    fn from(inner: ParameterErrorKind) -> Self {
        ParameterError { inner }
    }
}
impl fmt::Display for ParameterError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use ParameterErrorKind::*;
        match self.inner {
            ImageBufferSize { expected, actual } => {
                write!(fmt, "wrong data size, expected {} got {}", expected, actual)
            }
            PolledAfterEndOfImage => write!(fmt, "End of image has been reached"),
            PolledAfterFatalError => {
                write!(fmt, "A fatal decoding error has been encounted earlier")
            }
        }
    }
}
