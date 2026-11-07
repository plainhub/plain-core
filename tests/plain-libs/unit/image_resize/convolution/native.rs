use super::*;
use crate::image_resize::images::{TypedImage, TypedImageRef};
use crate::image_resize::pixels::{U8x3, U8x4};

fn coefficients() -> Coefficients {
    Coefficients {
        values: vec![0.5, 0.5, -1.0, 2.0],
        window_size: 2,
        bounds: vec![Bound { start: 0, size: 2 }; 2],
    }
}

#[test]
fn scalar_rgb_convolution_preserves_offset_rounding_and_clipping() {
    let pixels = [
        U8x3::new([255; 3]),
        U8x3::new([255; 3]),
        U8x3::new([20, 40, 60]),
        U8x3::new([40, 20, 100]),
    ];
    let src = TypedImageRef::new(2, 2, &pixels).unwrap();
    let mut output = [U8x3::default(); 2];
    let mut dst = TypedImage::from_pixels_slice(2, 1, &mut output).unwrap();
    U8x3::horiz_convolution(&src, &mut dst, 1, coefficients(), CpuExtensions::None);
    assert_eq!(output.map(|p| p.0), [[30, 30, 80], [60, 0, 140]]);
}

#[test]
fn scalar_rgba_convolution_applies_the_same_kernel_to_alpha() {
    let pixels = [
        U8x4::new([255; 4]),
        U8x4::new([255; 4]),
        U8x4::new([20, 40, 60, 80]),
        U8x4::new([40, 20, 100, 240]),
    ];
    let src = TypedImageRef::new(2, 2, &pixels).unwrap();
    let mut output = [U8x4::default(); 2];
    let mut dst = TypedImage::from_pixels_slice(2, 1, &mut output).unwrap();
    U8x4::horiz_convolution(&src, &mut dst, 1, coefficients(), CpuExtensions::None);
    assert_eq!(output.map(|p| p.0), [[30, 30, 80, 160], [60, 0, 140, 255]]);
}
