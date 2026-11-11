use crate::image::color::ColorType;
use crate::image::error::{
    DecodingError, ImageError, ImageFormatHint, ParameterError, ParameterErrorKind,
    UnsupportedError, UnsupportedErrorKind,
};
use crate::image::traits::Pixel;
use crate::image::{GenericImage, GenericImageView, ImageBuffer};
use num_traits::Zero;
use std::marker::PhantomData;
use std::ops::{Deref, Index, IndexMut};
use std::{cmp, error, fmt};
#[derive(Clone, Debug)]
pub struct FlatSamples<Buffer> {
    pub samples: Buffer,
    pub layout: SampleLayout,
    pub color_hint: Option<ColorType>,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SampleLayout {
    pub channels: u8,
    pub channel_stride: usize,
    pub width: u32,
    pub width_stride: usize,
    pub height: u32,
    pub height_stride: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Dim(usize, usize);
impl SampleLayout {
    #[must_use]
    pub fn row_major_packed(channels: u8, width: u32, height: u32) -> Self {
        let height_stride = (channels as usize).checked_mul(width as usize).expect(
            "Row major packed image can not be described because it does not fit into memory",
        );
        SampleLayout {
            channels,
            channel_stride: 1,
            width,
            width_stride: channels as usize,
            height,
            height_stride,
        }
    }

    #[must_use]
    pub fn strides_cwh(&self) -> (usize, usize, usize) {
        (self.channel_stride, self.width_stride, self.height_stride)
    }
    #[must_use]
    pub fn extents(&self) -> (usize, usize, usize) {
        (
            self.channels as usize,
            self.width as usize,
            self.height as usize,
        )
    }
    #[must_use]
    pub fn bounds(&self) -> (u8, u32, u32) {
        (self.channels, self.width, self.height)
    }
    #[must_use]
    pub fn min_length(&self) -> Option<usize> {
        if self.width == 0 || self.height == 0 || self.channels == 0 {
            return Some(0);
        }
        self.index(self.channels - 1, self.width - 1, self.height - 1)
            .and_then(|idx| idx.checked_add(1))
    }
    #[must_use]
    pub fn fits(&self, len: usize) -> bool {
        self.min_length().is_some_and(|min| len >= min)
    }
    fn increasing_stride_dims(&self) -> [Dim; 3] {
        let mut grouped: [Dim; 3] = [
            Dim(self.channel_stride, self.channels as usize),
            Dim(self.width_stride, self.width as usize),
            Dim(self.height_stride, self.height as usize),
        ];
        grouped.sort();
        let (min_dim, mid_dim, max_dim) = (grouped[0], grouped[1], grouped[2]);
        assert!(min_dim.stride() <= mid_dim.stride() && mid_dim.stride() <= max_dim.stride());
        grouped
    }
    #[must_use]
    pub fn has_aliased_samples(&self) -> bool {
        let grouped = self.increasing_stride_dims();
        let (min_dim, mid_dim, max_dim) = (grouped[0], grouped[1], grouped[2]);
        let min_size = match min_dim.checked_len() {
            None => return true,
            Some(size) => size,
        };
        let mid_size = match mid_dim.checked_len() {
            None => return true,
            Some(size) => size,
        };
        if max_dim.checked_len().is_none() {
            return true;
        }
        min_size > mid_dim.stride() || mid_size > max_dim.stride()
    }
    #[must_use]
    pub fn is_normal(&self, form: NormalForm) -> bool {
        if self.has_aliased_samples() {
            return false;
        }
        if form >= NormalForm::PixelPacked && self.channel_stride != 1 {
            return false;
        }
        if form >= NormalForm::ImagePacked {
            let grouped = self.increasing_stride_dims();
            let (min_dim, mid_dim, max_dim) = (grouped[0], grouped[1], grouped[2]);
            if 1 != min_dim.stride() {
                return false;
            }
            if min_dim.len() != mid_dim.stride() {
                return false;
            }
            if mid_dim.len() != max_dim.stride() {
                return false;
            }
        }
        if form >= NormalForm::RowMajorPacked {
            if self.width_stride != self.channels as usize {
                return false;
            }
            if self.width as usize * self.width_stride != self.height_stride {
                return false;
            }
        }
        if form >= NormalForm::ColumnMajorPacked {
            if self.height_stride != self.channels as usize {
                return false;
            }
            if self.height as usize * self.height_stride != self.width_stride {
                return false;
            }
        }
        true
    }
    #[must_use]
    pub fn in_bounds(&self, channel: u8, x: u32, y: u32) -> bool {
        channel < self.channels && x < self.width && y < self.height
    }
    #[must_use]
    pub fn index(&self, channel: u8, x: u32, y: u32) -> Option<usize> {
        if !self.in_bounds(channel, x, y) {
            return None;
        }
        self.index_ignoring_bounds(channel as usize, x as usize, y as usize)
    }
    #[must_use]
    pub fn index_ignoring_bounds(&self, channel: usize, x: usize, y: usize) -> Option<usize> {
        let idx_c = channel.checked_mul(self.channel_stride);
        let idx_x = x.checked_mul(self.width_stride);
        let idx_y = y.checked_mul(self.height_stride);
        let (Some(idx_c), Some(idx_x), Some(idx_y)) = (idx_c, idx_x, idx_y) else {
            return None;
        };
        Some(0usize)
            .and_then(|b| b.checked_add(idx_c))
            .and_then(|b| b.checked_add(idx_x))
            .and_then(|b| b.checked_add(idx_y))
    }
    #[must_use]
    pub fn in_bounds_index(&self, c: u8, x: u32, y: u32) -> usize {
        let (c_stride, x_stride, y_stride) = self.strides_cwh();
        (y as usize * y_stride) + (x as usize * x_stride) + (c as usize * c_stride)
    }
    pub fn shrink_to(&mut self, channels: u8, width: u32, height: u32) {
        self.channels = self.channels.min(channels);
        self.width = self.width.min(width);
        self.height = self.height.min(height);
    }
}
impl Dim {
    fn stride(self) -> usize {
        self.0
    }
    fn checked_len(self) -> Option<usize> {
        self.0.checked_mul(self.1)
    }
    fn len(self) -> usize {
        self.0 * self.1
    }
}
impl<Buffer> FlatSamples<Buffer> {
    pub fn strides_cwh(&self) -> (usize, usize, usize) {
        self.layout.strides_cwh()
    }
    pub fn extents(&self) -> (usize, usize, usize) {
        self.layout.extents()
    }
    pub fn bounds(&self) -> (u8, u32, u32) {
        self.layout.bounds()
    }
    pub fn as_ref<T>(&self) -> FlatSamples<&[T]>
    where
        Buffer: AsRef<[T]>,
    {
        FlatSamples {
            samples: self.samples.as_ref(),
            layout: self.layout,
            color_hint: self.color_hint,
        }
    }
    pub fn as_mut<T>(&mut self) -> FlatSamples<&mut [T]>
    where
        Buffer: AsMut<[T]>,
    {
        FlatSamples {
            samples: self.samples.as_mut(),
            layout: self.layout,
            color_hint: self.color_hint,
        }
    }
    pub fn to_vec<T>(&self) -> FlatSamples<Vec<T>>
    where
        T: Clone,
        Buffer: AsRef<[T]>,
    {
        FlatSamples {
            samples: self.samples.as_ref().to_vec(),
            layout: self.layout,
            color_hint: self.color_hint,
        }
    }
    pub fn get_sample<T>(&self, channel: u8, x: u32, y: u32) -> Option<&T>
    where
        Buffer: AsRef<[T]>,
    {
        self.index(channel, x, y)
            .and_then(|idx| self.samples.as_ref().get(idx))
    }
    pub fn get_mut_sample<T>(&mut self, channel: u8, x: u32, y: u32) -> Option<&mut T>
    where
        Buffer: AsMut<[T]>,
    {
        match self.index(channel, x, y) {
            None => None,
            Some(idx) => self.samples.as_mut().get_mut(idx),
        }
    }

    pub(crate) fn into_view<P>(self) -> Result<View<Buffer, P>, Error>
    where
        P: Pixel,
        Buffer: AsRef<[P::Subpixel]>,
    {
        if self.layout.channels != P::CHANNEL_COUNT {
            return Err(Error::ChannelCountMismatch(
                self.layout.channels,
                P::CHANNEL_COUNT,
            ));
        }
        if !self.layout.fits(self.samples.as_ref().len()) {
            return Err(Error::TooLarge);
        }
        Ok(View {
            inner: self,
            phantom: PhantomData,
        })
    }

    pub fn as_slice<T>(&self) -> &[T]
    where
        Buffer: AsRef<[T]>,
    {
        self.samples.as_ref()
    }
    pub fn as_mut_slice<T>(&mut self) -> &mut [T]
    where
        Buffer: AsMut<[T]>,
    {
        self.samples.as_mut()
    }
    pub fn image_slice<T>(&self) -> Option<&[T]>
    where
        Buffer: AsRef<[T]>,
    {
        let min_length = self.min_length()?;
        let slice = self.samples.as_ref();
        if slice.len() < min_length {
            return None;
        }
        Some(&slice[..min_length])
    }
    pub fn image_mut_slice<T>(&mut self) -> Option<&mut [T]>
    where
        Buffer: AsMut<[T]>,
    {
        let min_length = self.min_length()?;
        let slice = self.samples.as_mut();
        if slice.len() < min_length {
            return None;
        }
        Some(&mut slice[..min_length])
    }

    pub fn min_length(&self) -> Option<usize> {
        self.layout.min_length()
    }
    pub fn fits(&self, len: usize) -> bool {
        self.layout.fits(len)
    }
    pub fn has_aliased_samples(&self) -> bool {
        self.layout.has_aliased_samples()
    }
    pub fn is_normal(&self, form: NormalForm) -> bool {
        self.layout.is_normal(form)
    }
    pub fn in_bounds(&self, channel: u8, x: u32, y: u32) -> bool {
        self.layout.in_bounds(channel, x, y)
    }
    pub fn index(&self, channel: u8, x: u32, y: u32) -> Option<usize> {
        self.layout.index(channel, x, y)
    }
    pub fn index_ignoring_bounds(&self, channel: usize, x: usize, y: usize) -> Option<usize> {
        self.layout.index_ignoring_bounds(channel, x, y)
    }
    pub fn in_bounds_index(&self, channel: u8, x: u32, y: u32) -> usize {
        self.layout.in_bounds_index(channel, x, y)
    }
    pub fn shrink_to(&mut self, channels: u8, width: u32, height: u32) {
        self.layout.shrink_to(channels, width, height);
    }
}
impl<'buf, Subpixel> FlatSamples<&'buf [Subpixel]> {}
#[derive(Clone, Debug)]
pub struct View<Buffer, P: Pixel>
where
    Buffer: AsRef<[P::Subpixel]>,
{
    inner: FlatSamples<Buffer>,
    phantom: PhantomData<P>,
}
pub type ViewOfPixel<'lt, P> = View<&'lt [<P as Pixel>::Subpixel], P>;
#[derive(Clone, Debug)]
pub struct ViewMut<Buffer, P: Pixel>
where
    Buffer: AsMut<[P::Subpixel]>,
{
    inner: FlatSamples<Buffer>,
    phantom: PhantomData<P>,
}
pub type ViewMutOfPixel<'lt, P> = ViewMut<&'lt mut [<P as Pixel>::Subpixel], P>;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Error {
    TooLarge,
    NormalFormRequired(NormalForm),
    ChannelCountMismatch(u8, u8),
    WrongColor(ColorType),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NormalForm {
    Unaliased,
    PixelPacked,
    ImagePacked,
    RowMajorPacked,
    ColumnMajorPacked,
}
impl<Buffer, P: Pixel> View<Buffer, P>
where
    Buffer: AsRef<[P::Subpixel]>,
{
    pub fn into_inner(self) -> FlatSamples<Buffer> {
        self.inner
    }
    pub fn flat(&self) -> &FlatSamples<Buffer> {
        &self.inner
    }
    pub fn samples(&self) -> &Buffer {
        &self.inner.samples
    }
    pub fn get_sample(&self, channel: u8, x: u32, y: u32) -> Option<&P::Subpixel> {
        if !self.inner.in_bounds(channel, x, y) {
            return None;
        }
        let index = self.inner.in_bounds_index(channel, x, y);
        self.samples().as_ref().get(index)
    }
    pub fn get_mut_sample(&mut self, channel: u8, x: u32, y: u32) -> Option<&mut P::Subpixel>
    where
        Buffer: AsMut<[P::Subpixel]>,
    {
        if !self.inner.in_bounds(channel, x, y) {
            return None;
        }
        let index = self.inner.in_bounds_index(channel, x, y);
        self.inner.samples.as_mut().get_mut(index)
    }
    pub fn min_length(&self) -> usize {
        self.inner.min_length().unwrap()
    }
    pub fn image_slice(&self) -> &[P::Subpixel] {
        &self.samples().as_ref()[..self.min_length()]
    }
    pub(crate) fn strides_wh(&self) -> (usize, usize) {
        let (_, w, h) = self.inner.layout.strides_cwh();
        (w, h)
    }
    pub fn image_mut_slice(&mut self) -> &mut [P::Subpixel]
    where
        Buffer: AsMut<[P::Subpixel]>,
    {
        let min_length = self.min_length();
        &mut self.inner.samples.as_mut()[..min_length]
    }
    pub fn shrink_to(&mut self, width: u32, height: u32) {
        let channels = self.inner.layout.channels;
        self.inner.shrink_to(channels, width, height);
    }
}
impl<Buffer, P: Pixel> ViewMut<Buffer, P>
where
    Buffer: AsMut<[P::Subpixel]>,
{
    pub fn into_inner(self) -> FlatSamples<Buffer> {
        self.inner
    }
    pub fn flat(&self) -> &FlatSamples<Buffer> {
        &self.inner
    }
    pub fn samples(&self) -> &Buffer {
        &self.inner.samples
    }
    pub fn min_length(&self) -> usize {
        self.inner.min_length().unwrap()
    }
    pub fn get_sample(&self, channel: u8, x: u32, y: u32) -> Option<&P::Subpixel>
    where
        Buffer: AsRef<[P::Subpixel]>,
    {
        if !self.inner.in_bounds(channel, x, y) {
            return None;
        }
        let index = self.inner.in_bounds_index(channel, x, y);
        self.samples().as_ref().get(index)
    }
    pub fn get_mut_sample(&mut self, channel: u8, x: u32, y: u32) -> Option<&mut P::Subpixel> {
        if !self.inner.in_bounds(channel, x, y) {
            return None;
        }
        let index = self.inner.in_bounds_index(channel, x, y);
        self.inner.samples.as_mut().get_mut(index)
    }
    pub fn image_slice(&self) -> &[P::Subpixel]
    where
        Buffer: AsRef<[P::Subpixel]>,
    {
        &self.inner.samples.as_ref()[..self.min_length()]
    }
    pub fn image_mut_slice(&mut self) -> &mut [P::Subpixel] {
        let length = self.min_length();
        &mut self.inner.samples.as_mut()[..length]
    }
    pub fn shrink_to(&mut self, width: u32, height: u32) {
        let channels = self.inner.layout.channels;
        self.inner.shrink_to(channels, width, height);
    }
}
#[inline(never)]
#[cold]
fn panic_cwh_out_of_bounds(
    (c, x, y): (u8, u32, u32),
    bounds: (u8, u32, u32),
    strides: (usize, usize, usize),
) -> ! {
    panic!(
        "Sample coordinates {:?} out of sample matrix bounds {:?} with strides {:?}",
        (c, x, y),
        bounds,
        strides
    )
}
#[inline(never)]
#[cold]
fn panic_pixel_out_of_bounds((x, y): (u32, u32), bounds: (u32, u32)) -> ! {
    panic!("Image index {:?} out of bounds {:?}", (x, y), bounds)
}
impl<Buffer> Index<(u8, u32, u32)> for FlatSamples<Buffer>
where
    Buffer: Index<usize>,
{
    type Output = Buffer::Output;
    fn index(&self, (c, x, y): (u8, u32, u32)) -> &Self::Output {
        let bounds = self.bounds();
        let strides = self.strides_cwh();
        let index = self
            .index(c, x, y)
            .unwrap_or_else(|| panic_cwh_out_of_bounds((c, x, y), bounds, strides));
        &self.samples[index]
    }
}
impl<Buffer> IndexMut<(u8, u32, u32)> for FlatSamples<Buffer>
where
    Buffer: IndexMut<usize>,
{
    fn index_mut(&mut self, (c, x, y): (u8, u32, u32)) -> &mut Self::Output {
        let bounds = self.bounds();
        let strides = self.strides_cwh();
        let index = self
            .index(c, x, y)
            .unwrap_or_else(|| panic_cwh_out_of_bounds((c, x, y), bounds, strides));
        &mut self.samples[index]
    }
}
impl<Buffer, P: Pixel> GenericImageView for View<Buffer, P>
where
    Buffer: AsRef<[P::Subpixel]>,
{
    type Pixel = P;
    fn dimensions(&self) -> (u32, u32) {
        (self.inner.layout.width, self.inner.layout.height)
    }
    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        if !self.inner.in_bounds(0, x, y) {
            panic_pixel_out_of_bounds((x, y), self.dimensions())
        }
        let image = self.inner.samples.as_ref();
        let base_index = self.inner.in_bounds_index(0, x, y);
        let channels = P::CHANNEL_COUNT as usize;
        let mut buffer = [Zero::zero(); 256];
        buffer
            .iter_mut()
            .enumerate()
            .take(channels)
            .for_each(|(c, to)| {
                let index = base_index + c * self.inner.layout.channel_stride;
                *to = image[index];
            });
        *P::from_slice(&buffer[..channels])
    }
    fn to_pixel_view(&self) -> Option<ViewOfPixel<'_, Self::Pixel>> {
        Some(View {
            inner: FlatSamples {
                samples: self.inner.samples.as_ref(),
                layout: self.inner.layout,
                color_hint: None,
            },
            phantom: PhantomData,
        })
    }
}
impl<Buffer, P: Pixel> GenericImageView for ViewMut<Buffer, P>
where
    Buffer: AsMut<[P::Subpixel]> + AsRef<[P::Subpixel]>,
{
    type Pixel = P;
    fn dimensions(&self) -> (u32, u32) {
        (self.inner.layout.width, self.inner.layout.height)
    }
    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        if !self.inner.in_bounds(0, x, y) {
            panic_pixel_out_of_bounds((x, y), self.dimensions())
        }
        let image = self.inner.samples.as_ref();
        let base_index = self.inner.in_bounds_index(0, x, y);
        let channels = P::CHANNEL_COUNT as usize;
        let mut buffer = [Zero::zero(); 256];
        buffer
            .iter_mut()
            .enumerate()
            .take(channels)
            .for_each(|(c, to)| {
                let index = base_index + c * self.inner.layout.channel_stride;
                *to = image[index];
            });
        *P::from_slice(&buffer[..channels])
    }
    fn to_pixel_view(&self) -> Option<ViewOfPixel<'_, Self::Pixel>> {
        Some(View {
            inner: FlatSamples {
                samples: self.inner.samples.as_ref(),
                layout: self.inner.layout,
                color_hint: None,
            },
            phantom: PhantomData,
        })
    }
}
impl<Buffer, P: Pixel> GenericImage for ViewMut<Buffer, P>
where
    Buffer: AsMut<[P::Subpixel]> + AsRef<[P::Subpixel]>,
{
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Self::Pixel {
        if !self.inner.in_bounds(0, x, y) {
            panic_pixel_out_of_bounds((x, y), self.dimensions())
        }
        let base_index = self.inner.in_bounds_index(0, x, y);
        let channel_count = <P as Pixel>::CHANNEL_COUNT as usize;
        let pixel_range = base_index..base_index + channel_count;
        P::from_slice_mut(&mut self.inner.samples.as_mut()[pixel_range])
    }
    fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        *self.get_pixel_mut(x, y) = pixel;
    }
    fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        self.get_pixel_mut(x, y).blend(&pixel);
    }
}
impl From<Error> for ImageError {
    fn from(error: Error) -> ImageError {
        #[derive(Debug)]
        struct NormalFormRequiredError(NormalForm);
        impl fmt::Display for NormalFormRequiredError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "Required sample buffer in normal form {:?}", self.0)
            }
        }
        impl error::Error for NormalFormRequiredError {}
        match error {
            Error::TooLarge => ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::DimensionMismatch,
            )),
            Error::NormalFormRequired(form) => ImageError::Decoding(DecodingError::new(
                ImageFormatHint::Unknown,
                NormalFormRequiredError(form),
            )),
            Error::ChannelCountMismatch(_lc, _pc) => ImageError::Parameter(
                ParameterError::from_kind(ParameterErrorKind::DimensionMismatch),
            ),
            Error::WrongColor(color) => {
                ImageError::Unsupported(UnsupportedError::from_format_and_kind(
                    ImageFormatHint::Unknown,
                    UnsupportedErrorKind::Color(color.into()),
                ))
            }
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::TooLarge => write!(f, "The layout is too large"),
            Error::NormalFormRequired(form) => write!(
                f,
                "The layout needs to {}",
                match form {
                    NormalForm::ColumnMajorPacked => "be packed and in column major form",
                    NormalForm::ImagePacked => "be fully packed",
                    NormalForm::PixelPacked => "have packed pixels",
                    NormalForm::RowMajorPacked => "be packed and in row major form",
                    NormalForm::Unaliased => "not have any aliasing channels",
                }
            ),
            Error::ChannelCountMismatch(layout_channels, pixel_channels) => {
                write!(
                    f,
                    "The channel count of the chosen pixel (={pixel_channels}) does agree with the layout (={layout_channels})"
                )
            }
            Error::WrongColor(color) => {
                write!(f, "The chosen color type does not match the hint {color:?}")
            }
        }
    }
}
impl error::Error for Error {}
impl PartialOrd for NormalForm {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        match (*self, *other) {
            (NormalForm::Unaliased, NormalForm::Unaliased) => Some(cmp::Ordering::Equal),
            (NormalForm::PixelPacked, NormalForm::PixelPacked) => Some(cmp::Ordering::Equal),
            (NormalForm::ImagePacked, NormalForm::ImagePacked) => Some(cmp::Ordering::Equal),
            (NormalForm::RowMajorPacked, NormalForm::RowMajorPacked) => Some(cmp::Ordering::Equal),
            (NormalForm::ColumnMajorPacked, NormalForm::ColumnMajorPacked) => {
                Some(cmp::Ordering::Equal)
            }
            (NormalForm::Unaliased, _) => Some(cmp::Ordering::Less),
            (_, NormalForm::Unaliased) => Some(cmp::Ordering::Greater),
            (NormalForm::PixelPacked, NormalForm::ColumnMajorPacked) => Some(cmp::Ordering::Less),
            (NormalForm::PixelPacked, NormalForm::RowMajorPacked) => Some(cmp::Ordering::Less),
            (NormalForm::RowMajorPacked, NormalForm::PixelPacked) => Some(cmp::Ordering::Greater),
            (NormalForm::ColumnMajorPacked, NormalForm::PixelPacked) => {
                Some(cmp::Ordering::Greater)
            }
            (NormalForm::ImagePacked, NormalForm::ColumnMajorPacked) => Some(cmp::Ordering::Less),
            (NormalForm::ImagePacked, NormalForm::RowMajorPacked) => Some(cmp::Ordering::Less),
            (NormalForm::RowMajorPacked, NormalForm::ImagePacked) => Some(cmp::Ordering::Greater),
            (NormalForm::ColumnMajorPacked, NormalForm::ImagePacked) => {
                Some(cmp::Ordering::Greater)
            }
            (NormalForm::ImagePacked, NormalForm::PixelPacked) => None,
            (NormalForm::PixelPacked, NormalForm::ImagePacked) => None,
            (NormalForm::RowMajorPacked, NormalForm::ColumnMajorPacked) => None,
            (NormalForm::ColumnMajorPacked, NormalForm::RowMajorPacked) => None,
        }
    }
}
