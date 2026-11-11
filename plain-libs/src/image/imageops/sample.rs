use crate::image::images::buffer::{Gray16Image, GrayAlpha16Image, Rgb16Image, Rgba16Image};
use crate::image::traits::{Enlargeable, Pixel, Primitive};
use crate::image::utils::clamp;
use crate::image::{
    DynamicImage, GenericImage, GenericImageView, GrayAlphaImage, GrayImage, ImageBuffer,
    Rgb32FImage, RgbImage, Rgba32FImage, RgbaImage,
};
use num_traits::{NumCast, ToPrimitive, Zero};
use std::f32;
use std::ops::Mul;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(any(), derive(serde::Serialize, serde::Deserialize))]
pub enum FilterType {
    Nearest,
    Triangle,
    CatmullRom,
    Gaussian,
    Lanczos3,
}
pub(crate) struct Filter<'a> {
    pub(crate) kernel: Box<dyn Fn(f32) -> f32 + 'a>,
    pub(crate) support: f32,
}
struct FloatNearest(f32);
impl ToPrimitive for FloatNearest {
    fn to_i8(&self) -> Option<i8> {
        self.0.round().to_i8()
    }
    fn to_i16(&self) -> Option<i16> {
        self.0.round().to_i16()
    }
    fn to_i64(&self) -> Option<i64> {
        self.0.round().to_i64()
    }
    fn to_u8(&self) -> Option<u8> {
        self.0.round().to_u8()
    }
    fn to_u16(&self) -> Option<u16> {
        self.0.round().to_u16()
    }
    fn to_u64(&self) -> Option<u64> {
        self.0.round().to_u64()
    }
    fn to_f64(&self) -> Option<f64> {
        self.0.to_f64()
    }
}
fn sinc(t: f32) -> f32 {
    let a = t * f32::consts::PI;
    if t == 0.0 { 1.0 } else { a.sin() / a }
}
fn lanczos(x: f32, t: f32) -> f32 {
    if x.abs() < t {
        sinc(x) * sinc(x / t)
    } else {
        0.0
    }
}
fn bc_cubic_spline(x: f32, b: f32, c: f32) -> f32 {
    let a = x.abs();
    let k = if a < 1.0 {
        (12.0 - 9.0 * b - 6.0 * c) * a.powi(3)
            + (-18.0 + 12.0 * b + 6.0 * c) * a.powi(2)
            + (6.0 - 2.0 * b)
    } else if a < 2.0 {
        (-b - 6.0 * c) * a.powi(3)
            + (6.0 * b + 30.0 * c) * a.powi(2)
            + (-12.0 * b - 48.0 * c) * a
            + (8.0 * b + 24.0 * c)
    } else {
        0.0
    };
    k / 6.0
}
pub(crate) fn gaussian(x: f32, r: f32) -> f32 {
    ((2.0 * f32::consts::PI).sqrt() * r).recip() * (-x.powi(2) / (2.0 * r.powi(2))).exp()
}
pub(crate) fn lanczos3_kernel(x: f32) -> f32 {
    lanczos(x, 3.0)
}
pub(crate) fn gaussian_kernel(x: f32) -> f32 {
    gaussian(x, 0.5)
}
pub(crate) fn catmullrom_kernel(x: f32) -> f32 {
    bc_cubic_spline(x, 0.0, 0.5)
}
pub(crate) fn triangle_kernel(x: f32) -> f32 {
    if x.abs() < 1.0 { 1.0 - x.abs() } else { 0.0 }
}
pub(crate) fn box_kernel(_x: f32) -> f32 {
    1.0
}
fn horizontal_sample<P, S>(
    image: &Rgba32FImage,
    new_width: u32,
    filter: &mut Filter,
) -> ImageBuffer<P, Vec<S>>
where
    P: Pixel<Subpixel = S> + 'static,
    S: Primitive + 'static,
{
    let (width, height) = image.dimensions();
    assert!(
        width != 0 || height == 0,
        "Unexpected prior allocation size. This case should have been handled by the caller"
    );
    let mut out = ImageBuffer::new(new_width, height);
    out.copy_color_space_from(image);
    let mut ws = Vec::new();
    let max: f32 = NumCast::from(S::DEFAULT_MAX_VALUE).unwrap();
    let min: f32 = NumCast::from(S::DEFAULT_MIN_VALUE).unwrap();
    let ratio = width as f32 / new_width as f32;
    let sratio = if ratio < 1.0 { 1.0 } else { ratio };
    let src_support = filter.support * sratio;
    for outx in 0..new_width {
        let inputx = (outx as f32 + 0.5) * ratio;
        let left = (inputx - src_support).floor() as i64;
        let left = clamp(left, 0, <i64 as From<_>>::from(width) - 1) as u32;
        let right = (inputx + src_support).ceil() as i64;
        let right = clamp(
            right,
            <i64 as From<_>>::from(left) + 1,
            <i64 as From<_>>::from(width),
        ) as u32;
        let inputx = inputx - 0.5;
        ws.clear();
        let mut sum = 0.0;
        for i in left..right {
            let w = (filter.kernel)((i as f32 - inputx) / sratio);
            ws.push(w);
            sum += w;
        }
        for w in ws.iter_mut() {
            *w /= sum;
        }
        for y in 0..height {
            let mut t = (0.0, 0.0, 0.0, 0.0);
            for (i, w) in ws.iter().enumerate() {
                let p = image.get_pixel(left + i as u32, y);
                let vec = p.channels4();
                t.0 += vec.0 * w;
                t.1 += vec.1 * w;
                t.2 += vec.2 * w;
                t.3 += vec.3 * w;
            }
            let t = Pixel::from_channels(
                NumCast::from(FloatNearest(clamp(t.0, min, max))).unwrap(),
                NumCast::from(FloatNearest(clamp(t.1, min, max))).unwrap(),
                NumCast::from(FloatNearest(clamp(t.2, min, max))).unwrap(),
                NumCast::from(FloatNearest(clamp(t.3, min, max))).unwrap(),
            );
            out.put_pixel(outx, y, t);
        }
    }
    out
}

fn vertical_sample<I, P, S>(image: &I, new_height: u32, filter: &mut Filter) -> Rgba32FImage
where
    I: GenericImageView<Pixel = P>,
    P: Pixel<Subpixel = S> + 'static,
    S: Primitive + 'static,
{
    let (width, height) = image.dimensions();
    assert!(
        height != 0 || width == 0,
        "Unexpected prior allocation size. This case should have been handled by the caller"
    );
    let mut out = ImageBuffer::new(width, new_height);
    out.copy_color_space_from(&image.buffer_with_dimensions(0, 0));
    let mut ws = Vec::new();
    let ratio = height as f32 / new_height as f32;
    let sratio = if ratio < 1.0 { 1.0 } else { ratio };
    let src_support = filter.support * sratio;
    for outy in 0..new_height {
        let inputy = (outy as f32 + 0.5) * ratio;
        let left = (inputy - src_support).floor() as i64;
        let left = clamp(left, 0, <i64 as From<_>>::from(height) - 1) as u32;
        let right = (inputy + src_support).ceil() as i64;
        let right = clamp(
            right,
            <i64 as From<_>>::from(left) + 1,
            <i64 as From<_>>::from(height),
        ) as u32;
        let inputy = inputy - 0.5;
        ws.clear();
        let mut sum = 0.0;
        for i in left..right {
            let w = (filter.kernel)((i as f32 - inputy) / sratio);
            ws.push(w);
            sum += w;
        }
        for w in ws.iter_mut() {
            *w /= sum;
        }
        for x in 0..width {
            let mut t = (0.0, 0.0, 0.0, 0.0);
            for (i, w) in ws.iter().enumerate() {
                let p = image.get_pixel(x, left + i as u32);
                let (k1, k2, k3, k4) = p.channels4();
                let vec: (f32, f32, f32, f32) = (
                    NumCast::from(k1).unwrap(),
                    NumCast::from(k2).unwrap(),
                    NumCast::from(k3).unwrap(),
                    NumCast::from(k4).unwrap(),
                );
                t.0 += vec.0 * w;
                t.1 += vec.1 * w;
                t.2 += vec.2 * w;
                t.3 += vec.3 * w;
            }
            let t = Pixel::from_channels(t.0, t.1, t.2, t.3);
            out.put_pixel(x, outy, t);
        }
    }
    out
}
struct ThumbnailSum<S: Primitive + Enlargeable>(S::Larger, S::Larger, S::Larger, S::Larger);
impl<S: Primitive + Enlargeable> ThumbnailSum<S> {
    fn zeroed() -> Self {
        ThumbnailSum(
            S::Larger::zero(),
            S::Larger::zero(),
            S::Larger::zero(),
            S::Larger::zero(),
        )
    }
    fn sample_val(val: S) -> S::Larger {
        <S::Larger as NumCast>::from(val).unwrap()
    }
    fn add_pixel<P: Pixel<Subpixel = S>>(&mut self, pixel: P) {
        let pixel = pixel.channels4();
        self.0 += Self::sample_val(pixel.0);
        self.1 += Self::sample_val(pixel.1);
        self.2 += Self::sample_val(pixel.2);
        self.3 += Self::sample_val(pixel.3);
    }
}
pub fn thumbnail<I, P, S>(image: &I, new_width: u32, new_height: u32) -> ImageBuffer<P, Vec<S>>
where
    I: GenericImageView<Pixel = P>,
    P: Pixel<Subpixel = S> + 'static,
    S: Primitive + Enlargeable + 'static,
{
    let (width, height) = image.dimensions();
    let mut out = image.buffer_with_dimensions(new_width, new_height);
    if height == 0 || width == 0 {
        return out;
    }
    let x_ratio = width as f32 / new_width as f32;
    let y_ratio = height as f32 / new_height as f32;
    for outy in 0..new_height {
        let bottomf = outy as f32 * y_ratio;
        let topf = bottomf + y_ratio;
        let bottom = clamp(bottomf.ceil() as u32, 0, height - 1);
        let top = clamp(topf.ceil() as u32, bottom, height);
        for outx in 0..new_width {
            let leftf = outx as f32 * x_ratio;
            let rightf = leftf + x_ratio;
            let left = clamp(leftf.ceil() as u32, 0, width - 1);
            let right = clamp(rightf.ceil() as u32, left, width);
            let avg = if bottom != top && left != right {
                thumbnail_sample_block(image, left, right, bottom, top)
            } else if bottom != top {
                debug_assert!(
                    left > 0 && right > 0,
                    "First output column must have corresponding pixels"
                );
                let fraction_horizontal = (leftf.fract() + rightf.fract()) / 2.;
                thumbnail_sample_fraction_horizontal(
                    image,
                    right - 1,
                    fraction_horizontal,
                    bottom,
                    top,
                )
            } else if left != right {
                debug_assert!(
                    bottom > 0 && top > 0,
                    "First output row must have corresponding pixels"
                );
                let fraction_vertical = (topf.fract() + bottomf.fract()) / 2.;
                thumbnail_sample_fraction_vertical(image, left, right, top - 1, fraction_vertical)
            } else {
                let fraction_horizontal = (topf.fract() + bottomf.fract()) / 2.;
                let fraction_vertical = (leftf.fract() + rightf.fract()) / 2.;
                thumbnail_sample_fraction_both(
                    image,
                    right - 1,
                    fraction_horizontal,
                    top - 1,
                    fraction_vertical,
                )
            };
            let pixel = Pixel::from_channels(avg.0, avg.1, avg.2, avg.3);
            out.put_pixel(outx, outy, pixel);
        }
    }
    out
}
fn thumbnail_sample_block<I, P, S>(
    image: &I,
    left: u32,
    right: u32,
    bottom: u32,
    top: u32,
) -> (S, S, S, S)
where
    I: GenericImageView<Pixel = P>,
    P: Pixel<Subpixel = S>,
    S: Primitive + Enlargeable,
{
    let mut sum = ThumbnailSum::zeroed();
    for y in bottom..top {
        for x in left..right {
            let k = image.get_pixel(x, y);
            sum.add_pixel(k);
        }
    }
    let n = <S::Larger as NumCast>::from((right - left) * (top - bottom)).unwrap();
    let round = <S::Larger as NumCast>::from(n / NumCast::from(2).unwrap()).unwrap();
    (
        S::clamp_from((sum.0 + round) / n),
        S::clamp_from((sum.1 + round) / n),
        S::clamp_from((sum.2 + round) / n),
        S::clamp_from((sum.3 + round) / n),
    )
}
fn thumbnail_sample_fraction_horizontal<I, P, S>(
    image: &I,
    left: u32,
    fraction_horizontal: f32,
    bottom: u32,
    top: u32,
) -> (S, S, S, S)
where
    I: GenericImageView<Pixel = P>,
    P: Pixel<Subpixel = S>,
    S: Primitive + Enlargeable,
{
    let fract = fraction_horizontal;
    let mut sum_left = ThumbnailSum::zeroed();
    let mut sum_right = ThumbnailSum::zeroed();
    for x in bottom..top {
        let k_left = image.get_pixel(left, x);
        sum_left.add_pixel(k_left);
        let k_right = image.get_pixel(left + 1, x);
        sum_right.add_pixel(k_right);
    }
    let fact_right = fract / ((top - bottom) as f32);
    let fact_left = (1. - fract) / ((top - bottom) as f32);
    let mix_left_and_right = |leftv: S::Larger, rightv: S::Larger| {
        <S as NumCast>::from(
            fact_left * leftv.to_f32().unwrap() + fact_right * rightv.to_f32().unwrap(),
        )
        .expect("Average sample value should fit into sample type")
    };
    (
        mix_left_and_right(sum_left.0, sum_right.0),
        mix_left_and_right(sum_left.1, sum_right.1),
        mix_left_and_right(sum_left.2, sum_right.2),
        mix_left_and_right(sum_left.3, sum_right.3),
    )
}
fn thumbnail_sample_fraction_vertical<I, P, S>(
    image: &I,
    left: u32,
    right: u32,
    bottom: u32,
    fraction_vertical: f32,
) -> (S, S, S, S)
where
    I: GenericImageView<Pixel = P>,
    P: Pixel<Subpixel = S>,
    S: Primitive + Enlargeable,
{
    let fract = fraction_vertical;
    let mut sum_bot = ThumbnailSum::zeroed();
    let mut sum_top = ThumbnailSum::zeroed();
    for x in left..right {
        let k_bot = image.get_pixel(x, bottom);
        sum_bot.add_pixel(k_bot);
        let k_top = image.get_pixel(x, bottom + 1);
        sum_top.add_pixel(k_top);
    }
    let fact_top = fract / ((right - left) as f32);
    let fact_bot = (1. - fract) / ((right - left) as f32);
    let mix_bot_and_top = |botv: S::Larger, topv: S::Larger| {
        <S as NumCast>::from(fact_bot * botv.to_f32().unwrap() + fact_top * topv.to_f32().unwrap())
            .expect("Average sample value should fit into sample type")
    };
    (
        mix_bot_and_top(sum_bot.0, sum_top.0),
        mix_bot_and_top(sum_bot.1, sum_top.1),
        mix_bot_and_top(sum_bot.2, sum_top.2),
        mix_bot_and_top(sum_bot.3, sum_top.3),
    )
}
fn thumbnail_sample_fraction_both<I, P, S>(
    image: &I,
    left: u32,
    fraction_vertical: f32,
    bottom: u32,
    fraction_horizontal: f32,
) -> (S, S, S, S)
where
    I: GenericImageView<Pixel = P>,
    P: Pixel<Subpixel = S>,
    S: Primitive + Enlargeable,
{
    let k_bl = image.get_pixel(left, bottom).channels4();
    let k_tl = image.get_pixel(left, bottom + 1).channels4();
    let k_br = image.get_pixel(left + 1, bottom).channels4();
    let k_tr = image.get_pixel(left + 1, bottom + 1).channels4();
    let frac_v = fraction_vertical;
    let frac_h = fraction_horizontal;
    let fact_tr = frac_v * frac_h;
    let fact_tl = frac_v * (1. - frac_h);
    let fact_br = (1. - frac_v) * frac_h;
    let fact_bl = (1. - frac_v) * (1. - frac_h);
    let mix = |br: S, tr: S, bl: S, tl: S| {
        <S as NumCast>::from(
            fact_br * br.to_f32().unwrap()
                + fact_tr * tr.to_f32().unwrap()
                + fact_bl * bl.to_f32().unwrap()
                + fact_tl * tl.to_f32().unwrap(),
        )
        .expect("Average sample value should fit into sample type")
    };
    (
        mix(k_br.0, k_tr.0, k_bl.0, k_tl.0),
        mix(k_br.1, k_tr.1, k_bl.1, k_tl.1),
        mix(k_br.2, k_tr.2, k_bl.2, k_tl.2),
        mix(k_br.3, k_tr.3, k_bl.3, k_tl.3),
    )
}

pub fn resize<I: GenericImageView>(
    image: &I,
    nwidth: u32,
    nheight: u32,
    filter: FilterType,
) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>
where
    I::Pixel: 'static,
    <I::Pixel as Pixel>::Subpixel: 'static,
{
    let is_empty = {
        let (width, height) = image.dimensions();
        width == 0 || height == 0
    };
    if is_empty {
        return image.buffer_with_dimensions(nwidth, nheight);
    }
    if (nwidth, nheight) == image.dimensions() {
        let mut tmp = image.buffer_like();
        tmp.copy_from(image, 0, 0).unwrap();
        return tmp;
    }
    let mut method = match filter {
        FilterType::Nearest => Filter {
            kernel: Box::new(box_kernel),
            support: 0.0,
        },
        FilterType::Triangle => Filter {
            kernel: Box::new(triangle_kernel),
            support: 1.0,
        },
        FilterType::CatmullRom => Filter {
            kernel: Box::new(catmullrom_kernel),
            support: 2.0,
        },
        FilterType::Gaussian => Filter {
            kernel: Box::new(gaussian_kernel),
            support: 3.0,
        },
        FilterType::Lanczos3 => Filter {
            kernel: Box::new(lanczos3_kernel),
            support: 3.0,
        },
    };
    let tmp: Rgba32FImage = vertical_sample(image, nheight, &mut method);
    horizontal_sample(&tmp, nwidth, &mut method)
}
