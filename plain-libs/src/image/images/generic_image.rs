use crate::image::error::{ImageError, ImageResult};
use crate::image::flat::ViewOfPixel;
use crate::image::math::Rect;
use crate::image::traits::Pixel;
use crate::image::{ImageBuffer, SubImage};
pub trait GenericImageView {
    type Pixel: Pixel;
    fn dimensions(&self) -> (u32, u32);
    fn width(&self) -> u32 {
        let (w, _) = self.dimensions();
        w
    }
    fn height(&self) -> u32 {
        let (_, h) = self.dimensions();
        h
    }
    fn in_bounds(&self, x: u32, y: u32) -> bool {
        let (width, height) = self.dimensions();
        x < width && y < height
    }
    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel;
    unsafe fn unsafe_get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        self.get_pixel(x, y)
    }
    fn pixels(&self) -> Pixels<'_, Self>
    where
        Self: Sized,
    {
        let (width, height) = self.dimensions();
        Pixels {
            image: self,
            x: 0,
            y: 0,
            width,
            height,
        }
    }
    fn view(&self, x: u32, y: u32, width: u32, height: u32) -> SubImage<&Self>
    where
        Self: Sized,
    {
        assert!(u64::from(x) + u64::from(width) <= u64::from(self.width()));
        assert!(u64::from(y) + u64::from(height) <= u64::from(self.height()));
        SubImage::new(self, x, y, width, height)
    }
    fn try_view(
        &self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<SubImage<&Self>, ImageError>
    where
        Self: Sized,
    {
        let rect = Rect {
            x,
            y,
            width,
            height,
        };
        rect.test_in_bounds(self)?;
        Ok(SubImage::new(self, x, y, width, height))
    }
    fn buffer_like(&self) -> ImageBuffer<Self::Pixel, Vec<<Self::Pixel as Pixel>::Subpixel>> {
        let (w, h) = self.dimensions();
        self.buffer_with_dimensions(w, h)
    }
    fn buffer_with_dimensions(
        &self,
        width: u32,
        height: u32,
    ) -> ImageBuffer<Self::Pixel, Vec<<Self::Pixel as Pixel>::Subpixel>> {
        ImageBuffer::new(width, height)
    }
    fn to_pixel_view(&self) -> Option<ViewOfPixel<'_, Self::Pixel>> {
        None
    }
}
#[derive(Debug)]
pub struct Pixels<'a, I: ?Sized + 'a> {
    image: &'a I,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}
impl<I: GenericImageView> Iterator for Pixels<'_, I> {
    type Item = (u32, u32, I::Pixel);
    fn next(&mut self) -> Option<(u32, u32, I::Pixel)> {
        if self.x >= self.width {
            self.x = 0;
            self.y += 1;
        }
        if self.y >= self.height {
            None
        } else {
            let pixel = self.image.get_pixel(self.x, self.y);
            let p = (self.x, self.y, pixel);
            self.x += 1;
            Some(p)
        }
    }
}
impl<I: ?Sized> Clone for Pixels<'_, I> {
    fn clone(&self) -> Self {
        Pixels { ..*self }
    }
}
pub trait GenericImage: GenericImageView {
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Self::Pixel;
    fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel);
    unsafe fn unsafe_put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        self.put_pixel(x, y, pixel);
    }
    fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel);
    fn copy_from<O>(&mut self, other: &O, x: u32, y: u32) -> ImageResult<()>
    where
        O: GenericImageView<Pixel = Self::Pixel>,
    {
        if let Some(flat) = other.to_pixel_view() {
            return self.copy_from_samples(flat, x, y);
        }
        Rect::from_image_at(other, x, y).test_in_bounds(self)?;
        for k in 0..other.height() {
            for i in 0..other.width() {
                let p = other.get_pixel(i, k);
                self.put_pixel(i + x, k + y, p);
            }
        }
        Ok(())
    }
    fn copy_from_samples(
        &mut self,
        samples: ViewOfPixel<'_, Self::Pixel>,
        x: u32,
        y: u32,
    ) -> ImageResult<()> {
        Rect::from_image_at(&samples, x, y).test_in_bounds(self)?;
        for k in 0..samples.height() {
            for i in 0..samples.width() {
                let p = samples.get_pixel(i, k);
                self.put_pixel(i + x, k + y, p);
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
        macro_rules! copy_within_impl_ {
            ($ xiter : expr , $ yiter : expr) => {
                for y in $yiter {
                    let sy = sy + y;
                    let dy = dy + y;
                    for x in $xiter {
                        let sx = sx + x;
                        let dx = dx + x;
                        let pixel = self.get_pixel(sx, sy);
                        self.put_pixel(dx, dy, pixel);
                    }
                }
            };
        }
        match (sx < dx, sy < dy) {
            (true, true) => copy_within_impl_!((0..width).rev(), (0..height).rev()),
            (true, false) => copy_within_impl_!((0..width).rev(), 0..height),
            (false, true) => copy_within_impl_!(0..width, (0..height).rev()),
            (false, false) => copy_within_impl_!(0..width, 0..height),
        }
        true
    }
    fn sub_image(&mut self, x: u32, y: u32, width: u32, height: u32) -> SubImage<&mut Self>
    where
        Self: Sized,
    {
        assert!(u64::from(x) + u64::from(width) <= u64::from(self.width()));
        assert!(u64::from(y) + u64::from(height) <= u64::from(self.height()));
        SubImage::new(self, x, y, width, height)
    }
}
