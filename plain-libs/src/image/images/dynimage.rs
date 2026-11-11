use crate::image::color::{self, FromColor, IntoColor};
use crate::image::error::{ImageError, ImageResult, ParameterError, ParameterErrorKind};
use crate::image::flat::FlatSamples;
use crate::image::images::buffer::{
    ConvertBuffer, Gray16Image, GrayAlpha16Image, GrayAlphaImage, GrayImage, ImageBuffer,
    Rgb16Image, Rgb32FImage, RgbImage, Rgba16Image, Rgba32FImage, RgbaImage,
};
use crate::image::io::encoder::ImageEncoderBoxed;
use crate::image::io::free_functions::{self, encoder_for_format};
use crate::image::math::resize_dimensions;
use crate::image::metadata::Orientation;
use crate::image::traits::Pixel;
use crate::image::{
    ExtendedColorType, GenericImage, GenericImageView, ImageDecoder, ImageEncoder, ImageFormat,
    ImageReader, Luma, LumaA, imageops,
    metadata::{Cicp, CicpColorPrimaries, CicpTransferCharacteristics},
};
use std::fs::File;
use std::io::{self, BufWriter, Seek, Write};
use std::path::Path;
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum DynamicImage {
    ImageLuma8(GrayImage),
    ImageLumaA8(GrayAlphaImage),
    ImageRgb8(RgbImage),
    ImageRgba8(RgbaImage),
    ImageLuma16(Gray16Image),
    ImageLumaA16(GrayAlpha16Image),
    ImageRgb16(Rgb16Image),
    ImageRgba16(Rgba16Image),
    ImageRgb32F(Rgb32FImage),
    ImageRgba32F(Rgba32FImage),
}
macro_rules ! dynamic_map (($ dynimage : expr , $ image : pat => $ action : expr) => ({ use DynamicImage ::*; match $ dynimage { ImageLuma8 ($ image) => ImageLuma8 ($ action) , ImageLumaA8 ($ image) => ImageLumaA8 ($ action) , ImageRgb8 ($ image) => ImageRgb8 ($ action) , ImageRgba8 ($ image) => ImageRgba8 ($ action) , ImageLuma16 ($ image) => ImageLuma16 ($ action) , ImageLumaA16 ($ image) => ImageLumaA16 ($ action) , ImageRgb16 ($ image) => ImageRgb16 ($ action) , ImageRgba16 ($ image) => ImageRgba16 ($ action) , ImageRgb32F ($ image) => ImageRgb32F ($ action) , ImageRgba32F ($ image) => ImageRgba32F ($ action) , } }) ; ($ dynimage : expr , $ image : pat_param , $ action : expr) => (match $ dynimage { DynamicImage :: ImageLuma8 ($ image) => $ action , DynamicImage :: ImageLumaA8 ($ image) => $ action , DynamicImage :: ImageRgb8 ($ image) => $ action , DynamicImage :: ImageRgba8 ($ image) => $ action , DynamicImage :: ImageLuma16 ($ image) => $ action , DynamicImage :: ImageLumaA16 ($ image) => $ action , DynamicImage :: ImageRgb16 ($ image) => $ action , DynamicImage :: ImageRgba16 ($ image) => $ action , DynamicImage :: ImageRgb32F ($ image) => $ action , DynamicImage :: ImageRgba32F ($ image) => $ action , }) ;) ;
impl Clone for DynamicImage {
    fn clone(&self) -> Self {
        dynamic_map!(*self, ref p, DynamicImage::from(p.clone()))
    }
    fn clone_from(&mut self, source: &Self) {
        match (self, source) {
            (Self::ImageLuma8(p1), Self::ImageLuma8(p2)) => p1.clone_from(p2),
            (Self::ImageLumaA8(p1), Self::ImageLumaA8(p2)) => p1.clone_from(p2),
            (Self::ImageRgb8(p1), Self::ImageRgb8(p2)) => p1.clone_from(p2),
            (Self::ImageRgba8(p1), Self::ImageRgba8(p2)) => p1.clone_from(p2),
            (Self::ImageLuma16(p1), Self::ImageLuma16(p2)) => p1.clone_from(p2),
            (Self::ImageLumaA16(p1), Self::ImageLumaA16(p2)) => p1.clone_from(p2),
            (Self::ImageRgb16(p1), Self::ImageRgb16(p2)) => p1.clone_from(p2),
            (Self::ImageRgba16(p1), Self::ImageRgba16(p2)) => p1.clone_from(p2),
            (Self::ImageRgb32F(p1), Self::ImageRgb32F(p2)) => p1.clone_from(p2),
            (Self::ImageRgba32F(p1), Self::ImageRgba32F(p2)) => p1.clone_from(p2),
            (this, source) => *this = source.clone(),
        }
    }
}
impl DynamicImage {
    #[must_use]
    pub fn new(w: u32, h: u32, color: color::ColorType) -> DynamicImage {
        use color::ColorType::*;
        match color {
            L8 => Self::new_luma8(w, h),
            La8 => Self::new_luma_a8(w, h),
            Rgb8 => Self::new_rgb8(w, h),
            Rgba8 => Self::new_rgba8(w, h),
            L16 => Self::new_luma16(w, h),
            La16 => Self::new_luma_a16(w, h),
            Rgb16 => Self::new_rgb16(w, h),
            Rgba16 => Self::new_rgba16(w, h),
            Rgb32F => Self::new_rgb32f(w, h),
            Rgba32F => Self::new_rgba32f(w, h),
        }
    }
    #[must_use]
    pub fn new_luma8(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageLuma8(ImageBuffer::new(w, h))
    }
    #[must_use]
    pub fn new_luma_a8(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageLumaA8(ImageBuffer::new(w, h))
    }
    #[must_use]
    pub fn new_rgb8(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(ImageBuffer::new(w, h))
    }
    #[must_use]
    pub fn new_rgba8(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(ImageBuffer::new(w, h))
    }
    #[must_use]
    pub fn new_luma16(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageLuma16(ImageBuffer::new(w, h))
    }
    #[must_use]
    pub fn new_luma_a16(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageLumaA16(ImageBuffer::new(w, h))
    }
    #[must_use]
    pub fn new_rgb16(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb16(ImageBuffer::new(w, h))
    }
    #[must_use]
    pub fn new_rgba16(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba16(ImageBuffer::new(w, h))
    }
    #[must_use]
    pub fn new_rgb32f(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb32F(ImageBuffer::new(w, h))
    }
    #[must_use]
    pub fn new_rgba32f(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba32F(ImageBuffer::new(w, h))
    }
    pub fn from_decoder(decoder: impl ImageDecoder) -> ImageResult<Self> {
        decoder_to_image(decoder)
    }
    #[inline]
    #[must_use]
    pub fn to<
        T: Pixel
            + FromColor<color::Rgb<u8>>
            + FromColor<color::Rgb<f32>>
            + FromColor<color::Rgba<u8>>
            + FromColor<color::Rgba<u16>>
            + FromColor<color::Rgba<f32>>
            + FromColor<color::Rgb<u16>>
            + FromColor<Luma<u8>>
            + FromColor<Luma<u16>>
            + FromColor<LumaA<u16>>
            + FromColor<LumaA<u8>>,
    >(
        &self,
    ) -> ImageBuffer<T, Vec<T::Subpixel>> {
        dynamic_map!(*self, ref p, p.convert())
    }
    #[must_use]
    pub fn to_rgb8(&self) -> RgbImage {
        match self {
            DynamicImage::ImageRgb8(x) => x.clone(),
            x => dynamic_map!(x, p, p.cast_in_color_space()),
        }
    }
    #[must_use]
    pub fn to_rgb16(&self) -> Rgb16Image {
        match self {
            DynamicImage::ImageRgb16(x) => x.clone(),
            x => dynamic_map!(x, p, p.cast_in_color_space()),
        }
    }
    #[must_use]
    pub fn to_rgb32f(&self) -> Rgb32FImage {
        match self {
            DynamicImage::ImageRgb32F(x) => x.clone(),
            x => dynamic_map!(x, p, p.cast_in_color_space()),
        }
    }
    #[must_use]
    pub fn to_rgba8(&self) -> RgbaImage {
        match self {
            DynamicImage::ImageRgba8(x) => x.clone(),
            x => dynamic_map!(x, p, p.cast_in_color_space()),
        }
    }
    #[must_use]
    pub fn to_rgba16(&self) -> Rgba16Image {
        match self {
            DynamicImage::ImageRgba16(x) => x.clone(),
            x => dynamic_map!(x, p, p.cast_in_color_space()),
        }
    }
    #[must_use]
    pub fn to_rgba32f(&self) -> Rgba32FImage {
        match self {
            DynamicImage::ImageRgba32F(x) => x.clone(),
            x => dynamic_map!(x, p, p.cast_in_color_space()),
        }
    }
    #[must_use]
    pub fn to_luma8(&self) -> GrayImage {
        match self {
            DynamicImage::ImageLuma8(x) => x.clone(),
            x => dynamic_map!(x, p, p.cast_in_color_space()),
        }
    }
    #[must_use]
    pub fn to_luma16(&self) -> Gray16Image {
        match self {
            DynamicImage::ImageLuma16(x) => x.clone(),
            x => dynamic_map!(x, p, p.cast_in_color_space()),
        }
    }

    #[must_use]
    pub fn to_luma_alpha8(&self) -> GrayAlphaImage {
        match self {
            DynamicImage::ImageLumaA8(x) => x.clone(),
            x => dynamic_map!(x, p, p.cast_in_color_space()),
        }
    }
    #[must_use]
    pub fn to_luma_alpha16(&self) -> GrayAlpha16Image {
        match self {
            DynamicImage::ImageLumaA16(x) => x.clone(),
            x => dynamic_map!(x, p, p.cast_in_color_space()),
        }
    }

    #[must_use]
    pub fn into_rgb8(self) -> RgbImage {
        match self {
            DynamicImage::ImageRgb8(x) => x,
            x => x.to_rgb8(),
        }
    }
    #[must_use]
    pub fn into_rgb16(self) -> Rgb16Image {
        match self {
            DynamicImage::ImageRgb16(x) => x,
            x => x.to_rgb16(),
        }
    }

    #[must_use]
    pub fn into_rgba8(self) -> RgbaImage {
        match self {
            DynamicImage::ImageRgba8(x) => x,
            x => x.to_rgba8(),
        }
    }
    #[must_use]
    pub fn into_rgba16(self) -> Rgba16Image {
        match self {
            DynamicImage::ImageRgba16(x) => x,
            x => x.to_rgba16(),
        }
    }
    #[must_use]
    pub fn into_rgba32f(self) -> Rgba32FImage {
        match self {
            DynamicImage::ImageRgba32F(x) => x,
            x => x.to_rgba32f(),
        }
    }
    #[must_use]
    pub fn into_luma8(self) -> GrayImage {
        match self {
            DynamicImage::ImageLuma8(x) => x,
            x => x.to_luma8(),
        }
    }
    #[must_use]
    pub fn into_luma16(self) -> Gray16Image {
        match self {
            DynamicImage::ImageLuma16(x) => x,
            x => x.to_luma16(),
        }
    }
    #[must_use]
    pub fn into_luma_alpha8(self) -> GrayAlphaImage {
        match self {
            DynamicImage::ImageLumaA8(x) => x,
            x => x.to_luma_alpha8(),
        }
    }
    #[must_use]
    pub fn into_luma_alpha16(self) -> GrayAlpha16Image {
        match self {
            DynamicImage::ImageLumaA16(x) => x,
            x => x.to_luma_alpha16(),
        }
    }
    #[must_use]
    pub fn crop(&mut self, x: u32, y: u32, width: u32, height: u32) -> DynamicImage {
        dynamic_map ! (* self , ref mut p => imageops :: crop (p , x , y , width , height) . to_image ())
    }
    #[must_use]
    pub fn crop_imm(&self, x: u32, y: u32, width: u32, height: u32) -> DynamicImage {
        dynamic_map ! (* self , ref p => imageops :: crop_imm (p , x , y , width , height) . to_image ())
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        dynamic_map!(
            *self,
            ref image_buffer,
            bytemuck::cast_slice(image_buffer.as_raw())
        )
    }
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        dynamic_map!(self, image_buffer, {
            match bytemuck::allocation::try_cast_vec(image_buffer.into_raw()) {
                Ok(vec) => vec,
                Err((_, vec)) => bytemuck::cast_slice(&vec).to_owned(),
            }
        })
    }
    #[must_use]
    pub fn color(&self) -> color::ColorType {
        match *self {
            DynamicImage::ImageLuma8(_) => color::ColorType::L8,
            DynamicImage::ImageLumaA8(_) => color::ColorType::La8,
            DynamicImage::ImageRgb8(_) => color::ColorType::Rgb8,
            DynamicImage::ImageRgba8(_) => color::ColorType::Rgba8,
            DynamicImage::ImageLuma16(_) => color::ColorType::L16,
            DynamicImage::ImageLumaA16(_) => color::ColorType::La16,
            DynamicImage::ImageRgb16(_) => color::ColorType::Rgb16,
            DynamicImage::ImageRgba16(_) => color::ColorType::Rgba16,
            DynamicImage::ImageRgb32F(_) => color::ColorType::Rgb32F,
            DynamicImage::ImageRgba32F(_) => color::ColorType::Rgba32F,
        }
    }
    #[must_use]
    pub fn width(&self) -> u32 {
        dynamic_map!(*self, ref p, { p.width() })
    }
    #[must_use]
    pub fn height(&self) -> u32 {
        dynamic_map!(*self, ref p, { p.height() })
    }
    pub fn set_rgb_primaries(&mut self, color: CicpColorPrimaries) {
        dynamic_map!(self, p, p.set_rgb_primaries(color));
    }
    pub fn set_transfer_function(&mut self, tf: CicpTransferCharacteristics) {
        dynamic_map!(self, p, p.set_transfer_function(tf));
    }
    pub fn color_space(&self) -> Cicp {
        dynamic_map!(self, p, p.color_space())
    }
    pub fn set_color_space(&mut self, cicp: Cicp) -> ImageResult<()> {
        dynamic_map!(self, p, p.set_color_space(cicp))
    }
    #[must_use]
    pub fn has_alpha(&self) -> bool {
        self.color().has_alpha()
    }

    #[must_use]
    pub fn resize(&self, nwidth: u32, nheight: u32, filter: imageops::FilterType) -> DynamicImage {
        if (nwidth, nheight) == self.dimensions() {
            return self.clone();
        }
        let (width2, height2) =
            resize_dimensions(self.width(), self.height(), nwidth, nheight, false);
        self.resize_exact(width2, height2, filter)
    }
    #[must_use]
    pub fn resize_exact(
        &self,
        nwidth: u32,
        nheight: u32,
        filter: imageops::FilterType,
    ) -> DynamicImage {
        dynamic_map ! (* self , ref p => imageops :: resize (p , nwidth , nheight , filter))
    }
    #[must_use]
    pub fn thumbnail(&self, nwidth: u32, nheight: u32) -> DynamicImage {
        let (width2, height2) =
            resize_dimensions(self.width(), self.height(), nwidth, nheight, false);
        self.thumbnail_exact(width2, height2)
    }
    #[must_use]
    pub fn thumbnail_exact(&self, nwidth: u32, nheight: u32) -> DynamicImage {
        dynamic_map ! (* self , ref p => imageops :: thumbnail (p , nwidth , nheight))
    }
    #[must_use]
    pub fn resize_to_fill(
        &self,
        nwidth: u32,
        nheight: u32,
        filter: imageops::FilterType,
    ) -> DynamicImage {
        let (width2, height2) =
            resize_dimensions(self.width(), self.height(), nwidth, nheight, true);
        let mut intermediate = self.resize_exact(width2, height2, filter);
        let (iwidth, iheight) = intermediate.dimensions();
        let ratio = u64::from(iwidth) * u64::from(nheight);
        let nratio = u64::from(nwidth) * u64::from(iheight);
        if nratio > ratio {
            intermediate.crop(0, (iheight - nheight) / 2, nwidth, nheight)
        } else {
            intermediate.crop((iwidth - nwidth) / 2, 0, nwidth, nheight)
        }
    }

    fn flipv_in_place(&mut self) {
        dynamic_map!(*self, ref mut p, imageops::flip_vertical_in_place(p))
    }

    fn fliph_in_place(&mut self) {
        dynamic_map!(*self, ref mut p, imageops::flip_horizontal_in_place(p))
    }
    #[must_use]
    pub fn rotate90(&self) -> DynamicImage {
        dynamic_map ! (* self , ref p => imageops :: rotate90 (p))
    }
    #[must_use]
    pub fn rotate180(&self) -> DynamicImage {
        dynamic_map ! (* self , ref p => imageops :: rotate180 (p))
    }
    fn rotate180_in_place(&mut self) {
        dynamic_map!(*self, ref mut p, imageops::rotate180_in_place(p))
    }
    #[must_use]
    pub fn rotate270(&self) -> DynamicImage {
        dynamic_map ! (* self , ref p => imageops :: rotate270 (p))
    }
    pub fn apply_orientation(&mut self, orientation: Orientation) {
        let image = self;
        match orientation {
            Orientation::NoTransforms => (),
            Orientation::Rotate90 => *image = image.rotate90(),
            Orientation::Rotate180 => image.rotate180_in_place(),
            Orientation::Rotate270 => *image = image.rotate270(),
            Orientation::FlipHorizontal => image.fliph_in_place(),
            Orientation::FlipVertical => image.flipv_in_place(),
            Orientation::Rotate90FlipH => {
                let mut new_image = image.rotate90();
                new_image.fliph_in_place();
                *image = new_image;
            }
            Orientation::Rotate270FlipH => {
                let mut new_image = image.rotate270();
                new_image.fliph_in_place();
                *image = new_image;
            }
        }
    }
    fn write_with_encoder_impl<'a>(
        &self,
        encoder: Box<dyn ImageEncoderBoxed + 'a>,
    ) -> ImageResult<()> {
        let converted =
            encoder.make_compatible_img(crate::image::io::encoder::MethodSealedToImage, self);
        let img = converted.as_ref().unwrap_or(self);
        encoder.write_image(
            img.as_bytes(),
            img.width(),
            img.height(),
            img.color().into(),
        )
    }
    pub fn write_to<W: Write + Seek>(&self, mut w: W, format: ImageFormat) -> ImageResult<()> {
        let encoder = encoder_for_format(format, &mut w)?;
        self.write_with_encoder_impl(encoder)
    }
    pub fn write_with_encoder(&self, encoder: impl ImageEncoder) -> ImageResult<()> {
        self.write_with_encoder_impl(Box::new(encoder))
    }
    pub fn save<Q>(&self, path: Q) -> ImageResult<()>
    where
        Q: AsRef<Path>,
    {
        let format = ImageFormat::from_path(path.as_ref())?;
        self.save_with_format(path, format)
    }
    pub fn save_with_format<Q>(&self, path: Q, format: ImageFormat) -> ImageResult<()>
    where
        Q: AsRef<Path>,
    {
        let file = &mut BufWriter::new(File::create(path)?);
        let encoder = encoder_for_format(format, file)?;
        self.write_with_encoder_impl(encoder)
    }
}
impl From<GrayImage> for DynamicImage {
    fn from(image: GrayImage) -> Self {
        DynamicImage::ImageLuma8(image)
    }
}
impl From<GrayAlphaImage> for DynamicImage {
    fn from(image: GrayAlphaImage) -> Self {
        DynamicImage::ImageLumaA8(image)
    }
}
impl From<RgbImage> for DynamicImage {
    fn from(image: RgbImage) -> Self {
        DynamicImage::ImageRgb8(image)
    }
}
impl From<RgbaImage> for DynamicImage {
    fn from(image: RgbaImage) -> Self {
        DynamicImage::ImageRgba8(image)
    }
}
impl From<Gray16Image> for DynamicImage {
    fn from(image: Gray16Image) -> Self {
        DynamicImage::ImageLuma16(image)
    }
}
impl From<GrayAlpha16Image> for DynamicImage {
    fn from(image: GrayAlpha16Image) -> Self {
        DynamicImage::ImageLumaA16(image)
    }
}
impl From<Rgb16Image> for DynamicImage {
    fn from(image: Rgb16Image) -> Self {
        DynamicImage::ImageRgb16(image)
    }
}
impl From<Rgba16Image> for DynamicImage {
    fn from(image: Rgba16Image) -> Self {
        DynamicImage::ImageRgba16(image)
    }
}
impl From<Rgb32FImage> for DynamicImage {
    fn from(image: Rgb32FImage) -> Self {
        DynamicImage::ImageRgb32F(image)
    }
}
impl From<Rgba32FImage> for DynamicImage {
    fn from(image: Rgba32FImage) -> Self {
        DynamicImage::ImageRgba32F(image)
    }
}
impl From<ImageBuffer<Luma<f32>, Vec<f32>>> for DynamicImage {
    fn from(image: ImageBuffer<Luma<f32>, Vec<f32>>) -> Self {
        DynamicImage::ImageRgb32F(image.convert())
    }
}
impl From<ImageBuffer<LumaA<f32>, Vec<f32>>> for DynamicImage {
    fn from(image: ImageBuffer<LumaA<f32>, Vec<f32>>) -> Self {
        DynamicImage::ImageRgba32F(image.convert())
    }
}
impl GenericImageView for DynamicImage {
    type Pixel = color::Rgba<u8>;
    fn dimensions(&self) -> (u32, u32) {
        dynamic_map!(*self, ref p, p.dimensions())
    }
    fn get_pixel(&self, x: u32, y: u32) -> color::Rgba<u8> {
        dynamic_map!(*self, ref p, p.get_pixel(x, y).to_rgba().into_color())
    }
}
impl GenericImage for DynamicImage {
    fn put_pixel(&mut self, x: u32, y: u32, pixel: color::Rgba<u8>) {
        match *self {
            DynamicImage::ImageLuma8(ref mut p) => p.put_pixel(x, y, pixel.to_luma()),
            DynamicImage::ImageLumaA8(ref mut p) => p.put_pixel(x, y, pixel.to_luma_alpha()),
            DynamicImage::ImageRgb8(ref mut p) => p.put_pixel(x, y, pixel.to_rgb()),
            DynamicImage::ImageRgba8(ref mut p) => p.put_pixel(x, y, pixel),
            DynamicImage::ImageLuma16(ref mut p) => p.put_pixel(x, y, pixel.to_luma().into_color()),
            DynamicImage::ImageLumaA16(ref mut p) => {
                p.put_pixel(x, y, pixel.to_luma_alpha().into_color());
            }
            DynamicImage::ImageRgb16(ref mut p) => p.put_pixel(x, y, pixel.to_rgb().into_color()),
            DynamicImage::ImageRgba16(ref mut p) => p.put_pixel(x, y, pixel.into_color()),
            DynamicImage::ImageRgb32F(ref mut p) => p.put_pixel(x, y, pixel.to_rgb().into_color()),
            DynamicImage::ImageRgba32F(ref mut p) => p.put_pixel(x, y, pixel.into_color()),
        }
    }
    fn blend_pixel(&mut self, x: u32, y: u32, pixel: color::Rgba<u8>) {
        match *self {
            DynamicImage::ImageLuma8(ref mut p) => p.blend_pixel(x, y, pixel.to_luma()),
            DynamicImage::ImageLumaA8(ref mut p) => p.blend_pixel(x, y, pixel.to_luma_alpha()),
            DynamicImage::ImageRgb8(ref mut p) => p.blend_pixel(x, y, pixel.to_rgb()),
            DynamicImage::ImageRgba8(ref mut p) => p.blend_pixel(x, y, pixel),
            DynamicImage::ImageLuma16(ref mut p) => {
                p.blend_pixel(x, y, pixel.to_luma().into_color());
            }
            DynamicImage::ImageLumaA16(ref mut p) => {
                p.blend_pixel(x, y, pixel.to_luma_alpha().into_color());
            }
            DynamicImage::ImageRgb16(ref mut p) => p.blend_pixel(x, y, pixel.to_rgb().into_color()),
            DynamicImage::ImageRgba16(ref mut p) => p.blend_pixel(x, y, pixel.into_color()),
            DynamicImage::ImageRgb32F(ref mut p) => {
                p.blend_pixel(x, y, pixel.to_rgb().into_color());
            }
            DynamicImage::ImageRgba32F(ref mut p) => p.blend_pixel(x, y, pixel.into_color()),
        }
    }
    fn get_pixel_mut(&mut self, _: u32, _: u32) -> &mut color::Rgba<u8> {
        unimplemented!()
    }
}
impl Default for DynamicImage {
    fn default() -> Self {
        Self::ImageRgba8(Default::default())
    }
}
fn decoder_to_image<I: ImageDecoder>(decoder: I) -> ImageResult<DynamicImage> {
    let (w, h) = decoder.dimensions();
    let color_type = decoder.color_type();
    let mut image = match color_type {
        color::ColorType::Rgb8 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgb8)
        }
        color::ColorType::Rgba8 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgba8)
        }
        color::ColorType::L8 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageLuma8)
        }
        color::ColorType::La8 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageLumaA8)
        }
        color::ColorType::Rgb16 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgb16)
        }
        color::ColorType::Rgba16 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgba16)
        }
        color::ColorType::Rgb32F => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgb32F)
        }
        color::ColorType::Rgba32F => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgba32F)
        }
        color::ColorType::L16 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageLuma16)
        }
        color::ColorType::La16 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageLumaA16)
        }
    }
    .ok_or_else(|| {
        ImageError::Parameter(ParameterError::from_kind(
            ParameterErrorKind::DimensionMismatch,
        ))
    })?;
    image.set_rgb_primaries(Cicp::SRGB.primaries);
    image.set_transfer_function(Cicp::SRGB.transfer);
    Ok(image)
}
pub fn open<P>(path: P) -> ImageResult<DynamicImage>
where
    P: AsRef<Path>,
{
    ImageReader::open(path)?.decode()
}
pub fn image_dimensions<P>(path: P) -> ImageResult<(u32, u32)>
where
    P: AsRef<Path>,
{
    ImageReader::open(path)?.into_dimensions()
}
pub fn write_buffer_with_format<W: Write + Seek>(
    buffered_writer: &mut W,
    buf: &[u8],
    width: u32,
    height: u32,
    color: impl Into<ExtendedColorType>,
    format: ImageFormat,
) -> ImageResult<()> {
    let encoder = encoder_for_format(format, buffered_writer)?;
    encoder.write_image(buf, width, height, color.into())
}
pub fn load_from_memory(buffer: &[u8]) -> ImageResult<DynamicImage> {
    ImageReader::new(io::Cursor::new(buffer))
        .with_guessed_format()?
        .decode()
}
#[inline(always)]
pub fn load_from_memory_with_format(buf: &[u8], format: ImageFormat) -> ImageResult<DynamicImage> {
    let b = io::Cursor::new(buf);
    free_functions::load(b, format)
}
