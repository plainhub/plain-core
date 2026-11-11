use crate::image::error::{ImageError, ParameterError, ParameterErrorKind};
use crate::image::traits::Pixel;
use crate::image::{GenericImage, GenericImageView, ImageBuffer};
pub fn rotate90<I: GenericImageView>(
    image: &I,
) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>
where
    I::Pixel: 'static,
{
    let (width, height) = image.dimensions();
    let mut out = image.buffer_with_dimensions(height, width);
    let _ = rotate90_in(image, &mut out);
    out
}
pub fn rotate180<I: GenericImageView>(
    image: &I,
) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>
where
    I::Pixel: 'static,
{
    let (width, height) = image.dimensions();
    let mut out = image.buffer_with_dimensions(width, height);
    let _ = rotate180_in(image, &mut out);
    out
}
pub fn rotate270<I: GenericImageView>(
    image: &I,
) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>
where
    I::Pixel: 'static,
{
    let (width, height) = image.dimensions();
    let mut out = image.buffer_with_dimensions(height, width);
    let _ = rotate270_in(image, &mut out);
    out
}
pub fn rotate90_in<I, Container>(
    image: &I,
    destination: &mut ImageBuffer<I::Pixel, Container>,
) -> crate::image::ImageResult<()>
where
    I: GenericImageView,
    I::Pixel: 'static,
    Container: std::ops::DerefMut<Target = [<I::Pixel as Pixel>::Subpixel]>,
{
    let ((w0, h0), (w1, h1)) = (image.dimensions(), destination.dimensions());
    if w0 != h1 || h0 != w1 {
        return Err(ImageError::Parameter(ParameterError::from_kind(
            ParameterErrorKind::DimensionMismatch,
        )));
    }
    for y in 0..h0 {
        for x in 0..w0 {
            let p = image.get_pixel(x, y);
            destination.put_pixel(h0 - y - 1, x, p);
        }
    }
    Ok(())
}
pub fn rotate180_in<I, Container>(
    image: &I,
    destination: &mut ImageBuffer<I::Pixel, Container>,
) -> crate::image::ImageResult<()>
where
    I: GenericImageView,
    I::Pixel: 'static,
    Container: std::ops::DerefMut<Target = [<I::Pixel as Pixel>::Subpixel]>,
{
    let ((w0, h0), (w1, h1)) = (image.dimensions(), destination.dimensions());
    if w0 != w1 || h0 != h1 {
        return Err(ImageError::Parameter(ParameterError::from_kind(
            ParameterErrorKind::DimensionMismatch,
        )));
    }
    for y in 0..h0 {
        for x in 0..w0 {
            let p = image.get_pixel(x, y);
            destination.put_pixel(w0 - x - 1, h0 - y - 1, p);
        }
    }
    Ok(())
}
pub fn rotate270_in<I, Container>(
    image: &I,
    destination: &mut ImageBuffer<I::Pixel, Container>,
) -> crate::image::ImageResult<()>
where
    I: GenericImageView,
    I::Pixel: 'static,
    Container: std::ops::DerefMut<Target = [<I::Pixel as Pixel>::Subpixel]>,
{
    let ((w0, h0), (w1, h1)) = (image.dimensions(), destination.dimensions());
    if w0 != h1 || h0 != w1 {
        return Err(ImageError::Parameter(ParameterError::from_kind(
            ParameterErrorKind::DimensionMismatch,
        )));
    }
    for y in 0..h0 {
        for x in 0..w0 {
            let p = image.get_pixel(x, y);
            destination.put_pixel(y, w0 - x - 1, p);
        }
    }
    Ok(())
}
pub fn flip_horizontal<I: GenericImageView>(
    image: &I,
) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>
where
    I::Pixel: 'static,
{
    let mut out = image.buffer_like();
    let _ = flip_horizontal_in(image, &mut out);
    out
}
pub fn flip_vertical<I: GenericImageView>(
    image: &I,
) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>
where
    I::Pixel: 'static,
{
    let mut out = image.buffer_like();
    let _ = flip_vertical_in(image, &mut out);
    out
}
pub fn flip_horizontal_in<I, Container>(
    image: &I,
    destination: &mut ImageBuffer<I::Pixel, Container>,
) -> crate::image::ImageResult<()>
where
    I: GenericImageView,
    I::Pixel: 'static,
    Container: std::ops::DerefMut<Target = [<I::Pixel as Pixel>::Subpixel]>,
{
    let ((w0, h0), (w1, h1)) = (image.dimensions(), destination.dimensions());
    if w0 != w1 || h0 != h1 {
        return Err(ImageError::Parameter(ParameterError::from_kind(
            ParameterErrorKind::DimensionMismatch,
        )));
    }
    for y in 0..h0 {
        for x in 0..w0 {
            let p = image.get_pixel(x, y);
            destination.put_pixel(w0 - x - 1, y, p);
        }
    }
    Ok(())
}
pub fn flip_vertical_in<I, Container>(
    image: &I,
    destination: &mut ImageBuffer<I::Pixel, Container>,
) -> crate::image::ImageResult<()>
where
    I: GenericImageView,
    I::Pixel: 'static,
    Container: std::ops::DerefMut<Target = [<I::Pixel as Pixel>::Subpixel]>,
{
    let ((w0, h0), (w1, h1)) = (image.dimensions(), destination.dimensions());
    if w0 != w1 || h0 != h1 {
        return Err(ImageError::Parameter(ParameterError::from_kind(
            ParameterErrorKind::DimensionMismatch,
        )));
    }
    for y in 0..h0 {
        for x in 0..w0 {
            let p = image.get_pixel(x, y);
            destination.put_pixel(x, h0 - 1 - y, p);
        }
    }
    Ok(())
}
pub fn rotate180_in_place<I: GenericImage>(image: &mut I) {
    let (width, height) = image.dimensions();
    for y in 0..height / 2 {
        for x in 0..width {
            let p = image.get_pixel(x, y);
            let x2 = width - x - 1;
            let y2 = height - y - 1;
            let p2 = image.get_pixel(x2, y2);
            image.put_pixel(x, y, p2);
            image.put_pixel(x2, y2, p);
        }
    }
    if height % 2 != 0 {
        let middle = height / 2;
        for x in 0..width / 2 {
            let p = image.get_pixel(x, middle);
            let x2 = width - x - 1;
            let p2 = image.get_pixel(x2, middle);
            image.put_pixel(x, middle, p2);
            image.put_pixel(x2, middle, p);
        }
    }
}
pub fn flip_horizontal_in_place<I: GenericImage>(image: &mut I) {
    let (width, height) = image.dimensions();
    for y in 0..height {
        for x in 0..width / 2 {
            let x2 = width - x - 1;
            let p2 = image.get_pixel(x2, y);
            let p = image.get_pixel(x, y);
            image.put_pixel(x2, y, p);
            image.put_pixel(x, y, p2);
        }
    }
}
pub fn flip_vertical_in_place<I: GenericImage>(image: &mut I) {
    let (width, height) = image.dimensions();
    for y in 0..height / 2 {
        for x in 0..width {
            let y2 = height - y - 1;
            let p2 = image.get_pixel(x, y2);
            let p = image.get_pixel(x, y);
            image.put_pixel(x, y2, p);
            image.put_pixel(x, y, p2);
        }
    }
}
