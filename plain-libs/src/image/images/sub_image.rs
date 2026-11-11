use crate::image::{
    GenericImage, GenericImageView, ImageBuffer, Pixel, flat::ViewOfPixel, math::Rect,
};
use std::ops::{Deref, DerefMut};
#[derive(Copy, Clone)]
pub struct SubImage<I> {
    inner: SubImageInner<I>,
}
#[derive(Copy, Clone)]
pub struct SubImageInner<I> {
    image: I,
    xoffset: u32,
    yoffset: u32,
    xstride: u32,
    ystride: u32,
}
type DerefPixel<I> = <<I as Deref>::Target as GenericImageView>::Pixel;
type DerefSubpixel<I> = <DerefPixel<I> as Pixel>::Subpixel;
impl<I> SubImage<I> {
    pub fn new(image: I, x: u32, y: u32, width: u32, height: u32) -> SubImage<I> {
        SubImage {
            inner: SubImageInner {
                image,
                xoffset: x,
                yoffset: y,
                xstride: width,
                ystride: height,
            },
        }
    }

    pub fn offsets(&self) -> (u32, u32) {
        (self.inner.xoffset, self.inner.yoffset)
    }
    pub fn to_image(&self) -> ImageBuffer<DerefPixel<I>, Vec<DerefSubpixel<I>>>
    where
        I: Deref,
        I::Target: GenericImageView + 'static,
    {
        let borrowed = &*self.inner.image;
        let mut out = borrowed.buffer_with_dimensions(self.inner.xstride, self.inner.ystride);
        for y in 0..self.inner.ystride {
            for x in 0..self.inner.xstride {
                let p = borrowed.get_pixel(x + self.inner.xoffset, y + self.inner.yoffset);
                out.put_pixel(x, y, p);
            }
        }
        out
    }
}
impl<I> SubImage<I>
where
    I: Deref,
    I::Target: GenericImageView,
{
    pub fn view(&self, x: u32, y: u32, width: u32, height: u32) -> SubImage<&I::Target> {
        use crate::image::GenericImageView as _;
        assert!(u64::from(x) + u64::from(width) <= u64::from(self.inner.width()));
        assert!(u64::from(y) + u64::from(height) <= u64::from(self.inner.height()));
        let x = self.inner.xoffset.saturating_add(x);
        let y = self.inner.yoffset.saturating_add(y);
        SubImage::new(&*self.inner.image, x, y, width, height)
    }
    pub fn inner(&self) -> &I::Target {
        &self.inner.image
    }
}
impl<I> SubImage<I>
where
    I: DerefMut,
    I::Target: GenericImage,
{
    pub fn sub_image(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> SubImage<&mut I::Target> {
        assert!(u64::from(x) + u64::from(width) <= u64::from(self.inner.width()));
        assert!(u64::from(y) + u64::from(height) <= u64::from(self.inner.height()));
        let x = self.inner.xoffset.saturating_add(x);
        let y = self.inner.yoffset.saturating_add(y);
        SubImage::new(&mut *self.inner.image, x, y, width, height)
    }
}
impl<I> Deref for SubImage<I>
where
    I: Deref,
{
    type Target = SubImageInner<I>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<I> DerefMut for SubImage<I>
where
    I: DerefMut,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
impl<I> GenericImageView for SubImageInner<I>
where
    I: Deref,
    I::Target: GenericImageView,
{
    type Pixel = DerefPixel<I>;
    fn dimensions(&self) -> (u32, u32) {
        (self.xstride, self.ystride)
    }
    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        self.image.get_pixel(x + self.xoffset, y + self.yoffset)
    }
    fn buffer_with_dimensions(
        &self,
        width: u32,
        height: u32,
    ) -> ImageBuffer<
        <I::Target as GenericImageView>::Pixel,
        Vec<<<I::Target as GenericImageView>::Pixel as Pixel>::Subpixel>,
    > {
        self.image.buffer_with_dimensions(width, height)
    }
    fn to_pixel_view(&self) -> Option<ViewOfPixel<'_, Self::Pixel>> {
        let inner = self.image.to_pixel_view()?;
        let mut descriptor = inner.into_inner();
        let offset = descriptor.index(0, self.xoffset, self.yoffset)?;
        descriptor.samples = descriptor.samples.get(offset..)?;
        descriptor.layout.width = self.xstride;
        descriptor.layout.height = self.ystride;
        descriptor.into_view().ok()
    }
}
impl<I> GenericImage for SubImageInner<I>
where
    I: DerefMut,
    I::Target: GenericImage + Sized,
{
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Self::Pixel {
        self.image.get_pixel_mut(x + self.xoffset, y + self.yoffset)
    }
    fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        self.image
            .put_pixel(x + self.xoffset, y + self.yoffset, pixel);
    }
    fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        self.image
            .blend_pixel(x + self.xoffset, y + self.yoffset, pixel);
    }
    fn copy_from<O>(&mut self, other: &O, x: u32, y: u32) -> Result<(), crate::image::ImageError>
    where
        O: GenericImageView<Pixel = Self::Pixel>,
    {
        Rect::from_image_at(other, x, y).test_in_bounds(self)?;
        self.image
            .copy_from(other, x + self.xoffset, y + self.yoffset)
    }
}
