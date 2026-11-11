pub use self::affine::{
    flip_horizontal, flip_horizontal_in, flip_horizontal_in_place, flip_vertical, flip_vertical_in,
    flip_vertical_in_place, rotate90, rotate90_in, rotate180, rotate180_in, rotate180_in_place,
    rotate270, rotate270_in,
};
pub use self::sample::FilterType;
pub use self::sample::FilterType::{CatmullRom, Gaussian, Lanczos3, Nearest, Triangle};
pub use self::sample::{resize, thumbnail};
use crate::image::traits::{Lerp, Pixel, Primitive};
use crate::image::{GenericImage, GenericImageView, SubImage};
use std::cmp;
mod affine;
mod sample;
pub fn crop<I: GenericImageView>(
    image: &mut I,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> SubImage<&mut I> {
    let (x, y, width, height) = crop_dimms(image, x, y, width, height);
    SubImage::new(image, x, y, width, height)
}
pub fn crop_imm<I: GenericImageView>(
    image: &I,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> SubImage<&I> {
    let (x, y, width, height) = crop_dimms(image, x, y, width, height);
    SubImage::new(image, x, y, width, height)
}
fn crop_dimms<I: GenericImageView>(
    image: &I,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> (u32, u32, u32, u32) {
    let (iwidth, iheight) = image.dimensions();
    let x = cmp::min(x, iwidth);
    let y = cmp::min(y, iheight);
    let height = cmp::min(height, iheight - y);
    let width = cmp::min(width, iwidth - x);
    (x, y, width, height)
}
