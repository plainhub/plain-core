use crate::image::color::{FromColor, FromPrimitive, Luma, LumaA, Rgb, Rgba};
use crate::image::error::{
    ImageResult, ParameterError, ParameterErrorKind, UnsupportedError, UnsupportedErrorKind,
};
use crate::image::flat::{FlatSamples, SampleLayout, ViewOfPixel};
use crate::image::math::Rect;
use crate::image::metadata::cicp::{CicpPixelCast, CicpRgb, ColorComponentForCicp};
use crate::image::traits::{EncodableLayout, Pixel, PixelWithColorType};
use crate::image::utils::expand_packed;
use crate::image::{DynamicImage, GenericImage, GenericImageView, ImageEncoder, ImageFormat};
use crate::image::{
    ImageError,
    metadata::{Cicp, CicpColorPrimaries, CicpTransferCharacteristics},
    save_buffer, save_buffer_with_format, write_buffer_with_format,
};
use num_traits::Zero;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Index, IndexMut, Range};
use std::path::Path;
use std::slice::{ChunksExact, ChunksExactMut};
pub struct Pixels<'a, P: Pixel + 'a>
where
    P::Subpixel: 'a,
{
    chunks: ChunksExact<'a, P::Subpixel>,
}
impl<'a, P: Pixel + 'a> Iterator for Pixels<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = &'a P;
    #[inline(always)]
    fn next(&mut self) -> Option<&'a P> {
        self.chunks.next().map(|v| <P as Pixel>::from_slice(v))
    }
    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}
impl<'a, P: Pixel + 'a> ExactSizeIterator for Pixels<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.chunks.len()
    }
}
impl<'a, P: Pixel + 'a> DoubleEndedIterator for Pixels<'a, P>
where
    P::Subpixel: 'a,
{
    #[inline(always)]
    fn next_back(&mut self) -> Option<&'a P> {
        self.chunks.next_back().map(|v| <P as Pixel>::from_slice(v))
    }
}
impl<P: Pixel> Clone for Pixels<'_, P> {
    fn clone(&self) -> Self {
        Pixels {
            chunks: self.chunks.clone(),
        }
    }
}
impl<P: Pixel> fmt::Debug for Pixels<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Pixels")
            .field("chunks", &self.chunks)
            .finish()
    }
}
pub struct PixelsMut<'a, P: Pixel + 'a>
where
    P::Subpixel: 'a,
{
    chunks: ChunksExactMut<'a, P::Subpixel>,
}
impl<'a, P: Pixel + 'a> Iterator for PixelsMut<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = &'a mut P;
    #[inline(always)]
    fn next(&mut self) -> Option<&'a mut P> {
        self.chunks.next().map(|v| <P as Pixel>::from_slice_mut(v))
    }
    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}
impl<'a, P: Pixel + 'a> ExactSizeIterator for PixelsMut<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.chunks.len()
    }
}
impl<'a, P: Pixel + 'a> DoubleEndedIterator for PixelsMut<'a, P>
where
    P::Subpixel: 'a,
{
    #[inline(always)]
    fn next_back(&mut self) -> Option<&'a mut P> {
        self.chunks
            .next_back()
            .map(|v| <P as Pixel>::from_slice_mut(v))
    }
}
impl<P: Pixel> fmt::Debug for PixelsMut<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PixelsMut")
            .field("chunks", &self.chunks)
            .finish()
    }
}
pub struct Rows<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    pixels: ChunksExact<'a, P::Subpixel>,
}
impl<'a, P: Pixel + 'a> Rows<'a, P> {
    fn with_image(pixels: &'a [P::Subpixel], width: u32, height: u32) -> Self {
        let row_len = (width as usize) * usize::from(<P as Pixel>::CHANNEL_COUNT);
        if row_len == 0 {
            Rows {
                pixels: [].chunks_exact(1),
            }
        } else {
            let pixels = pixels
                .get(..row_len * height as usize)
                .expect("Pixel buffer has too few subpixels");
            Rows {
                pixels: pixels.chunks_exact(row_len),
            }
        }
    }
}
impl<'a, P: Pixel + 'a> Iterator for Rows<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = Pixels<'a, P>;
    #[inline(always)]
    fn next(&mut self) -> Option<Pixels<'a, P>> {
        let row = self.pixels.next()?;
        Some(Pixels {
            chunks: row.chunks_exact(<P as Pixel>::CHANNEL_COUNT as usize),
        })
    }
    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}
impl<'a, P: Pixel + 'a> ExactSizeIterator for Rows<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.pixels.len()
    }
}
impl<'a, P: Pixel + 'a> DoubleEndedIterator for Rows<'a, P>
where
    P::Subpixel: 'a,
{
    #[inline(always)]
    fn next_back(&mut self) -> Option<Pixels<'a, P>> {
        let row = self.pixels.next_back()?;
        Some(Pixels {
            chunks: row.chunks_exact(<P as Pixel>::CHANNEL_COUNT as usize),
        })
    }
}
impl<P: Pixel> Clone for Rows<'_, P> {
    fn clone(&self) -> Self {
        Rows {
            pixels: self.pixels.clone(),
        }
    }
}
impl<P: Pixel> fmt::Debug for Rows<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Rows")
            .field("pixels", &self.pixels)
            .finish()
    }
}
pub struct RowsMut<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    pixels: ChunksExactMut<'a, P::Subpixel>,
}
impl<'a, P: Pixel + 'a> RowsMut<'a, P> {
    fn with_image(pixels: &'a mut [P::Subpixel], width: u32, height: u32) -> Self {
        let row_len = (width as usize) * usize::from(<P as Pixel>::CHANNEL_COUNT);
        if row_len == 0 {
            RowsMut {
                pixels: [].chunks_exact_mut(1),
            }
        } else {
            let pixels = pixels
                .get_mut(..row_len * height as usize)
                .expect("Pixel buffer has too few subpixels");
            RowsMut {
                pixels: pixels.chunks_exact_mut(row_len),
            }
        }
    }
}
impl<'a, P: Pixel + 'a> Iterator for RowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = PixelsMut<'a, P>;
    #[inline(always)]
    fn next(&mut self) -> Option<PixelsMut<'a, P>> {
        let row = self.pixels.next()?;
        Some(PixelsMut {
            chunks: row.chunks_exact_mut(<P as Pixel>::CHANNEL_COUNT as usize),
        })
    }
    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}
impl<'a, P: Pixel + 'a> ExactSizeIterator for RowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.pixels.len()
    }
}
impl<'a, P: Pixel + 'a> DoubleEndedIterator for RowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    #[inline(always)]
    fn next_back(&mut self) -> Option<PixelsMut<'a, P>> {
        let row = self.pixels.next_back()?;
        Some(PixelsMut {
            chunks: row.chunks_exact_mut(<P as Pixel>::CHANNEL_COUNT as usize),
        })
    }
}
impl<P: Pixel> fmt::Debug for RowsMut<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RowsMut")
            .field("pixels", &self.pixels)
            .finish()
    }
}
pub struct EnumeratePixels<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    pixels: Pixels<'a, P>,
    x: u32,
    y: u32,
    width: u32,
}
impl<'a, P: Pixel + 'a> Iterator for EnumeratePixels<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = (u32, u32, &'a P);
    #[inline(always)]
    fn next(&mut self) -> Option<(u32, u32, &'a P)> {
        if self.x >= self.width {
            self.x = 0;
            self.y += 1;
        }
        let (x, y) = (self.x, self.y);
        self.x += 1;
        self.pixels.next().map(|p| (x, y, p))
    }
    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}
impl<'a, P: Pixel + 'a> ExactSizeIterator for EnumeratePixels<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.pixels.len()
    }
}
impl<P: Pixel> Clone for EnumeratePixels<'_, P> {
    fn clone(&self) -> Self {
        EnumeratePixels {
            pixels: self.pixels.clone(),
            ..*self
        }
    }
}
impl<P: Pixel> fmt::Debug for EnumeratePixels<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EnumeratePixels")
            .field("pixels", &self.pixels)
            .field("x", &self.x)
            .field("y", &self.y)
            .field("width", &self.width)
            .finish()
    }
}
pub struct EnumerateRows<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    rows: Rows<'a, P>,
    y: u32,
    width: u32,
}
impl<'a, P: Pixel + 'a> Iterator for EnumerateRows<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = (u32, EnumeratePixels<'a, P>);
    #[inline(always)]
    fn next(&mut self) -> Option<(u32, EnumeratePixels<'a, P>)> {
        let y = self.y;
        self.y += 1;
        self.rows.next().map(|r| {
            (
                y,
                EnumeratePixels {
                    x: 0,
                    y,
                    width: self.width,
                    pixels: r,
                },
            )
        })
    }
    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}
impl<'a, P: Pixel + 'a> ExactSizeIterator for EnumerateRows<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.rows.len()
    }
}
impl<P: Pixel> Clone for EnumerateRows<'_, P> {
    fn clone(&self) -> Self {
        EnumerateRows {
            rows: self.rows.clone(),
            ..*self
        }
    }
}
impl<P: Pixel> fmt::Debug for EnumerateRows<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EnumerateRows")
            .field("rows", &self.rows)
            .field("y", &self.y)
            .field("width", &self.width)
            .finish()
    }
}
pub struct EnumeratePixelsMut<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    pixels: PixelsMut<'a, P>,
    x: u32,
    y: u32,
    width: u32,
}
impl<'a, P: Pixel + 'a> Iterator for EnumeratePixelsMut<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = (u32, u32, &'a mut P);
    #[inline(always)]
    fn next(&mut self) -> Option<(u32, u32, &'a mut P)> {
        if self.x >= self.width {
            self.x = 0;
            self.y += 1;
        }
        let (x, y) = (self.x, self.y);
        self.x += 1;
        self.pixels.next().map(|p| (x, y, p))
    }
    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}
impl<'a, P: Pixel + 'a> ExactSizeIterator for EnumeratePixelsMut<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.pixels.len()
    }
}
impl<P: Pixel> fmt::Debug for EnumeratePixelsMut<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EnumeratePixelsMut")
            .field("pixels", &self.pixels)
            .field("x", &self.x)
            .field("y", &self.y)
            .field("width", &self.width)
            .finish()
    }
}
pub struct EnumerateRowsMut<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    rows: RowsMut<'a, P>,
    y: u32,
    width: u32,
}
impl<'a, P: Pixel + 'a> Iterator for EnumerateRowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = (u32, EnumeratePixelsMut<'a, P>);
    #[inline(always)]
    fn next(&mut self) -> Option<(u32, EnumeratePixelsMut<'a, P>)> {
        let y = self.y;
        self.y += 1;
        self.rows.next().map(|r| {
            (
                y,
                EnumeratePixelsMut {
                    x: 0,
                    y,
                    width: self.width,
                    pixels: r,
                },
            )
        })
    }
    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}
impl<'a, P: Pixel + 'a> ExactSizeIterator for EnumerateRowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.rows.len()
    }
}
impl<P: Pixel> fmt::Debug for EnumerateRowsMut<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EnumerateRowsMut")
            .field("rows", &self.rows)
            .field("y", &self.y)
            .field("width", &self.width)
            .finish()
    }
}
#[derive(Debug, Hash, PartialEq, Eq)]
pub struct ImageBuffer<P: Pixel, Container> {
    width: u32,
    height: u32,
    _phantom: PhantomData<P>,
    color: CicpRgb,
    data: Container,
}
impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]>,
{
    pub fn from_raw(width: u32, height: u32, buf: Container) -> Option<ImageBuffer<P, Container>> {
        if Self::check_image_fits(width, height, buf.len()) {
            Some(ImageBuffer {
                data: buf,
                width,
                height,
                color: Cicp::SRGB.into_rgb(),
                _phantom: PhantomData,
            })
        } else {
            None
        }
    }
    pub fn into_raw(self) -> Container {
        self.data
    }
    pub fn as_raw(&self) -> &Container {
        &self.data
    }
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub(crate) fn inner_pixels(&self) -> &[P::Subpixel] {
        let len = Self::image_buffer_len(self.width, self.height).unwrap();
        &self.data[..len]
    }
    pub fn pixels(&self) -> Pixels<'_, P> {
        Pixels {
            chunks: self
                .inner_pixels()
                .chunks_exact(<P as Pixel>::CHANNEL_COUNT as usize),
        }
    }
    pub fn rows(&self) -> Rows<'_, P> {
        Rows::with_image(&self.data, self.width, self.height)
    }
    pub fn enumerate_pixels(&self) -> EnumeratePixels<'_, P> {
        EnumeratePixels {
            pixels: self.pixels(),
            x: 0,
            y: 0,
            width: self.width,
        }
    }

    #[inline]
    #[track_caller]
    pub fn get_pixel(&self, x: u32, y: u32) -> &P {
        match self.pixel_indices(x, y) {
            None => panic!(
                "Image index {:?} out of bounds {:?}",
                (x, y),
                (self.width, self.height)
            ),
            Some(pixel_indices) => <P as Pixel>::from_slice(&self.data[pixel_indices]),
        }
    }

    fn check_image_fits(width: u32, height: u32, len: usize) -> bool {
        let checked_len = Self::image_buffer_len(width, height);
        checked_len.is_some_and(|min_len| min_len <= len)
    }
    fn image_buffer_len(width: u32, height: u32) -> Option<usize> {
        Some(<P as Pixel>::CHANNEL_COUNT as usize)
            .and_then(|size| size.checked_mul(width as usize))
            .and_then(|size| size.checked_mul(height as usize))
    }
    #[inline(always)]
    fn pixel_indices(&self, x: u32, y: u32) -> Option<Range<usize>> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(self.pixel_indices_unchecked(x, y))
    }
    #[inline(always)]
    fn pixel_indices_unchecked(&self, x: u32, y: u32) -> Range<usize> {
        let no_channels = <P as Pixel>::CHANNEL_COUNT as usize;
        let min_index = (y as usize * self.width as usize + x as usize) * no_channels;
        min_index..min_index + no_channels
    }
    pub fn sample_layout(&self) -> SampleLayout {
        SampleLayout::row_major_packed(<P as Pixel>::CHANNEL_COUNT, self.width, self.height)
    }

    pub fn as_flat_samples(&self) -> FlatSamples<&[P::Subpixel]> {
        let layout = self.sample_layout();
        FlatSamples {
            samples: self.data.as_ref(),
            layout,
            color_hint: None,
        }
    }
}
impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + DerefMut,
{
    pub(crate) fn inner_pixels_mut(&mut self) -> &mut [P::Subpixel] {
        let len = Self::image_buffer_len(self.width, self.height).unwrap();
        &mut self.data[..len]
    }
    pub fn pixels_mut(&mut self) -> PixelsMut<'_, P> {
        PixelsMut {
            chunks: self
                .inner_pixels_mut()
                .chunks_exact_mut(<P as Pixel>::CHANNEL_COUNT as usize),
        }
    }
    pub fn rows_mut(&mut self) -> RowsMut<'_, P> {
        RowsMut::with_image(&mut self.data, self.width, self.height)
    }
    pub fn enumerate_pixels_mut(&mut self) -> EnumeratePixelsMut<'_, P> {
        let width = self.width;
        EnumeratePixelsMut {
            pixels: self.pixels_mut(),
            x: 0,
            y: 0,
            width,
        }
    }

    #[inline]
    #[track_caller]
    pub fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut P {
        match self.pixel_indices(x, y) {
            None => panic!(
                "Image index {:?} out of bounds {:?}",
                (x, y),
                (self.width, self.height)
            ),
            Some(pixel_indices) => <P as Pixel>::from_slice_mut(&mut self.data[pixel_indices]),
        }
    }

    #[inline]
    #[track_caller]
    pub fn put_pixel(&mut self, x: u32, y: u32, pixel: P) {
        *self.get_pixel_mut(x, y) = pixel;
    }
}
impl<P: Pixel, Container> ImageBuffer<P, Container> {
    pub fn set_rgb_primaries(&mut self, color: CicpColorPrimaries) {
        self.color.primaries = color;
    }
    pub fn set_transfer_function(&mut self, tf: CicpTransferCharacteristics) {
        self.color.transfer = tf;
    }
    pub fn color_space(&self) -> Cicp {
        self.color.into()
    }
    pub fn set_color_space(&mut self, cicp: Cicp) -> ImageResult<()> {
        self.color = cicp.try_into_rgb()?;
        Ok(())
    }
}
impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    [P::Subpixel]: EncodableLayout,
    Container: Deref<Target = [P::Subpixel]>,
{
    pub fn save<Q>(&self, path: Q) -> ImageResult<()>
    where
        Q: AsRef<Path>,
        P: PixelWithColorType,
    {
        save_buffer(
            path,
            self.inner_pixels().as_bytes(),
            self.width(),
            self.height(),
            <P as PixelWithColorType>::COLOR_TYPE,
        )
    }
}
impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    [P::Subpixel]: EncodableLayout,
    Container: Deref<Target = [P::Subpixel]>,
{
    pub fn save_with_format<Q>(&self, path: Q, format: ImageFormat) -> ImageResult<()>
    where
        Q: AsRef<Path>,
        P: PixelWithColorType,
    {
        save_buffer_with_format(
            path,
            self.inner_pixels().as_bytes(),
            self.width(),
            self.height(),
            <P as PixelWithColorType>::COLOR_TYPE,
            format,
        )
    }
}
impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    [P::Subpixel]: EncodableLayout,
    Container: Deref<Target = [P::Subpixel]>,
{
    pub fn write_to<W>(&self, writer: &mut W, format: ImageFormat) -> ImageResult<()>
    where
        W: std::io::Write + std::io::Seek,
        P: PixelWithColorType,
    {
        write_buffer_with_format(
            writer,
            self.inner_pixels().as_bytes(),
            self.width(),
            self.height(),
            <P as PixelWithColorType>::COLOR_TYPE,
            format,
        )
    }
}
impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    [P::Subpixel]: EncodableLayout,
    Container: Deref<Target = [P::Subpixel]>,
{
    pub fn write_with_encoder<E>(&self, encoder: E) -> ImageResult<()>
    where
        E: ImageEncoder,
        P: PixelWithColorType,
    {
        encoder.write_image(
            self.inner_pixels().as_bytes(),
            self.width(),
            self.height(),
            <P as PixelWithColorType>::COLOR_TYPE,
        )
    }
}
impl<P, Container> Default for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Default,
{
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            _phantom: PhantomData,
            color: Cicp::SRGB_LINEAR.into_rgb(),
            data: Default::default(),
        }
    }
}
impl<P, Container> Deref for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]>,
{
    type Target = [P::Subpixel];
    fn deref(&self) -> &<Self as Deref>::Target {
        &self.data
    }
}
impl<P, Container> DerefMut for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + DerefMut,
{
    fn deref_mut(&mut self) -> &mut <Self as Deref>::Target {
        &mut self.data
    }
}
impl<P, Container> Index<(u32, u32)> for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]>,
{
    type Output = P;
    fn index(&self, (x, y): (u32, u32)) -> &P {
        self.get_pixel(x, y)
    }
}
impl<P, Container> IndexMut<(u32, u32)> for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + DerefMut,
{
    fn index_mut(&mut self, (x, y): (u32, u32)) -> &mut P {
        self.get_pixel_mut(x, y)
    }
}
impl<P, Container> Clone for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + Clone,
{
    fn clone(&self) -> ImageBuffer<P, Container> {
        ImageBuffer {
            data: self.data.clone(),
            width: self.width,
            height: self.height,
            color: self.color,
            _phantom: PhantomData,
        }
    }
    fn clone_from(&mut self, source: &Self) {
        self.data.clone_from(&source.data);
        self.width = source.width;
        self.height = source.height;
        self.color = source.color;
    }
}
impl<P, Container> GenericImageView for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + Deref,
{
    type Pixel = P;
    fn dimensions(&self) -> (u32, u32) {
        self.dimensions()
    }
    fn get_pixel(&self, x: u32, y: u32) -> P {
        *self.get_pixel(x, y)
    }
    fn to_pixel_view(&self) -> Option<ViewOfPixel<'_, Self::Pixel>> {
        self.as_flat_samples().into_view().ok()
    }
    #[inline(always)]
    unsafe fn unsafe_get_pixel(&self, x: u32, y: u32) -> P {
        unsafe {
            let indices = self.pixel_indices_unchecked(x, y);
            *<P as Pixel>::from_slice(self.data.get_unchecked(indices))
        }
    }
    fn buffer_with_dimensions(&self, width: u32, height: u32) -> ImageBuffer<P, Vec<P::Subpixel>> {
        let mut buffer = ImageBuffer::new(width, height);
        buffer.copy_color_space_from(self);
        buffer
    }
}
impl<P, Container> GenericImage for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + DerefMut,
{
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut P {
        self.get_pixel_mut(x, y)
    }
    fn put_pixel(&mut self, x: u32, y: u32, pixel: P) {
        *self.get_pixel_mut(x, y) = pixel;
    }
    #[inline(always)]
    unsafe fn unsafe_put_pixel(&mut self, x: u32, y: u32, pixel: P) {
        unsafe {
            let indices = self.pixel_indices_unchecked(x, y);
            let p = <P as Pixel>::from_slice_mut(self.data.get_unchecked_mut(indices));
            *p = pixel;
        }
    }
    fn blend_pixel(&mut self, x: u32, y: u32, p: P) {
        self.get_pixel_mut(x, y).blend(&p);
    }
    fn copy_from_samples(
        &mut self,
        view: ViewOfPixel<'_, Self::Pixel>,
        x: u32,
        y: u32,
    ) -> ImageResult<()> {
        let (width, height) = view.dimensions();
        let pix_stride = usize::from(<Self::Pixel as Pixel>::CHANNEL_COUNT);
        Rect::from_image_at(&view, x, y).test_in_bounds(self)?;
        if width == 0 || height == 0 || pix_stride == 0 {
            return Ok(());
        }
        let row_len = width as usize * pix_stride;
        let img_sh = self.width as usize;
        let (sw, sh) = view.strides_wh();
        let view_samples: &[_] = view.samples();
        let inner = self.inner_pixels_mut();
        let img_pixel_indices_unchecked =
            |x: u32, y: u32| (y as usize * img_sh + x as usize) * pix_stride;
        if sw == pix_stride {
            for j in 0..height {
                let start = img_pixel_indices_unchecked(x, j + y);
                let img_row = &mut inner[start..][..row_len];
                let view_row = &view_samples[j as usize * sh..][..row_len];
                img_row.copy_from_slice(view_row);
            }
            return Ok(());
        }
        for j in 0..height {
            let img_start = img_pixel_indices_unchecked(x, j + y);
            let img_row = &mut inner[img_start..][..row_len];
            let pixels = img_row.chunks_exact_mut(pix_stride);
            let view_start = j as usize * sh;
            for (i, sp) in pixels.enumerate() {
                let view_pixel = &view_samples[i * sw + view_start..][..pix_stride];
                sp.copy_from_slice(view_pixel);
            }
        }
        Ok(())
    }
    fn copy_within(&mut self, source: Rect, x: u32, y: u32) -> bool {
        let Rect {
            x: sx,
            y: sy,
            width,
            height,
        } = source;
        let dx = x;
        let dy = y;
        assert!(sx < self.width() && dx < self.width());
        assert!(sy < self.height() && dy < self.height());
        if self.width() - dx.max(sx) < width || self.height() - dy.max(sy) < height {
            return false;
        }
        if sy < dy {
            for y in (0..height).rev() {
                let sy = sy + y;
                let dy = dy + y;
                let Range { start, .. } = self.pixel_indices_unchecked(sx, sy);
                let Range { end, .. } = self.pixel_indices_unchecked(sx + width - 1, sy);
                let dst = self.pixel_indices_unchecked(dx, dy).start;
                self.data.copy_within(start..end, dst);
            }
        } else {
            for y in 0..height {
                let sy = sy + y;
                let dy = dy + y;
                let Range { start, .. } = self.pixel_indices_unchecked(sx, sy);
                let Range { end, .. } = self.pixel_indices_unchecked(sx + width - 1, sy);
                let dst = self.pixel_indices_unchecked(dx, dy).start;
                self.data.copy_within(start..end, dst);
            }
        }
        true
    }
}
impl<P: Pixel> ImageBuffer<P, Vec<P::Subpixel>> {
    #[must_use]
    pub fn new(width: u32, height: u32) -> ImageBuffer<P, Vec<P::Subpixel>> {
        let size = Self::image_buffer_len(width, height)
            .expect("Buffer length in `ImageBuffer::new` overflows usize");
        ImageBuffer {
            data: vec![Zero::zero(); size],
            width,
            height,
            color: Cicp::SRGB.into_rgb(),
            _phantom: PhantomData,
        }
    }
    pub fn from_pixel(width: u32, height: u32, pixel: P) -> ImageBuffer<P, Vec<P::Subpixel>> {
        let mut buf = ImageBuffer::new(width, height);
        for p in buf.pixels_mut() {
            *p = pixel;
        }
        buf
    }
    pub fn from_fn<F>(width: u32, height: u32, mut f: F) -> ImageBuffer<P, Vec<P::Subpixel>>
    where
        F: FnMut(u32, u32) -> P,
    {
        let mut buf = ImageBuffer::new(width, height);
        for (x, y, p) in buf.enumerate_pixels_mut() {
            *p = f(x, y);
        }
        buf
    }
    #[must_use]
    pub fn from_vec(
        width: u32,
        height: u32,
        buf: Vec<P::Subpixel>,
    ) -> Option<ImageBuffer<P, Vec<P::Subpixel>>> {
        ImageBuffer::from_raw(width, height, buf)
    }
    #[must_use]
    pub fn into_vec(self) -> Vec<P::Subpixel> {
        self.into_raw()
    }
    pub(crate) fn copy_color_space_from<O: Pixel, C>(&mut self, other: &ImageBuffer<O, C>) {
        self.color = other.color;
    }
}
impl<S, Container> ImageBuffer<Rgb<S>, Container>
where
    Rgb<S>: PixelWithColorType<Subpixel = S>,
    Container: DerefMut<Target = [S]>,
{
}
impl<S, Container> ImageBuffer<Rgba<S>, Container>
where
    Rgba<S>: PixelWithColorType<Subpixel = S>,
    Container: DerefMut<Target = [S]>,
{
}
pub trait ConvertBuffer<T> {
    fn convert(&self) -> T;
}
impl GrayImage {}
impl<Container, FromType: Pixel, ToType: Pixel>
    ConvertBuffer<ImageBuffer<ToType, Vec<ToType::Subpixel>>> for ImageBuffer<FromType, Container>
where
    Container: Deref<Target = [FromType::Subpixel]>,
    ToType: FromColor<FromType>,
{
    fn convert(&self) -> ImageBuffer<ToType, Vec<ToType::Subpixel>> {
        let mut buffer: ImageBuffer<ToType, Vec<ToType::Subpixel>> =
            ImageBuffer::new(self.width, self.height);
        buffer.copy_color_space_from(self);
        for (to, from) in buffer.pixels_mut().zip(self.pixels()) {
            to.from_color(from);
        }
        buffer
    }
}
impl<C, SelfPixel: Pixel> ImageBuffer<SelfPixel, C>
where
    SelfPixel: PixelWithColorType,
    C: Deref<Target = [SelfPixel::Subpixel]> + DerefMut,
{
    pub(crate) fn cast_in_color_space<IntoPixel>(
        &self,
    ) -> ImageBuffer<IntoPixel, Vec<IntoPixel::Subpixel>>
    where
        SelfPixel: Pixel,
        IntoPixel: Pixel,
        IntoPixel: CicpPixelCast<SelfPixel>,
        SelfPixel::Subpixel: ColorComponentForCicp,
        IntoPixel::Subpixel: ColorComponentForCicp + FromPrimitive<SelfPixel::Subpixel>,
    {
        let vec = self
            .color
            .cast_pixels::<SelfPixel, IntoPixel>(self.inner_pixels(), &|| [0.2126, 0.7152, 0.0722]);
        let mut buffer = ImageBuffer::from_vec(self.width, self.height, vec)
            .expect("cast_pixels returned the right number of pixels");
        buffer.copy_color_space_from(self);
        buffer
    }
}
pub type RgbImage = ImageBuffer<Rgb<u8>, Vec<u8>>;
pub type RgbaImage = ImageBuffer<Rgba<u8>, Vec<u8>>;
pub type GrayImage = ImageBuffer<Luma<u8>, Vec<u8>>;
pub type GrayAlphaImage = ImageBuffer<LumaA<u8>, Vec<u8>>;
pub(crate) type Rgb16Image = ImageBuffer<Rgb<u16>, Vec<u16>>;
pub(crate) type Rgba16Image = ImageBuffer<Rgba<u16>, Vec<u16>>;
pub(crate) type Gray16Image = ImageBuffer<Luma<u16>, Vec<u16>>;
pub(crate) type GrayAlpha16Image = ImageBuffer<LumaA<u16>, Vec<u16>>;
pub type Rgb32FImage = ImageBuffer<Rgb<f32>, Vec<f32>>;
pub type Rgba32FImage = ImageBuffer<Rgba<f32>, Vec<f32>>;
impl From<DynamicImage> for RgbImage {
    fn from(value: DynamicImage) -> Self {
        value.into_rgb8()
    }
}
impl From<DynamicImage> for RgbaImage {
    fn from(value: DynamicImage) -> Self {
        value.into_rgba8()
    }
}
impl From<DynamicImage> for GrayImage {
    fn from(value: DynamicImage) -> Self {
        value.into_luma8()
    }
}
impl From<DynamicImage> for GrayAlphaImage {
    fn from(value: DynamicImage) -> Self {
        value.into_luma_alpha8()
    }
}
impl From<DynamicImage> for Rgb16Image {
    fn from(value: DynamicImage) -> Self {
        value.into_rgb16()
    }
}
impl From<DynamicImage> for Rgba16Image {
    fn from(value: DynamicImage) -> Self {
        value.into_rgba16()
    }
}
impl From<DynamicImage> for Gray16Image {
    fn from(value: DynamicImage) -> Self {
        value.into_luma16()
    }
}
impl From<DynamicImage> for GrayAlpha16Image {
    fn from(value: DynamicImage) -> Self {
        value.into_luma_alpha16()
    }
}
impl From<DynamicImage> for Rgba32FImage {
    fn from(value: DynamicImage) -> Self {
        value.into_rgba32f()
    }
}
