//! Pixel plumbing for the thumbnail engine: the in-memory [`Bitmap`] type,
//! integer box pre-downscaling, EXIF rotation, alpha flattening and the
//! SIMD final resample.

use crate::media::thumbnails::exif::Orientation;

/// A tightly-packed 8-bit bitmap in either RGB or RGBA row-major layout.
#[derive(Debug, Clone)]
pub enum Bitmap {
    Rgb8 { data: Vec<u8>, w: u32, h: u32 },
    Rgba8 { data: Vec<u8>, w: u32, h: u32 },
}

macro_rules! bitmap_accessors {
    ($var:ident) => {
        pub fn $var(&self) -> (&[u8], u32, u32) {
            match self {
                Bitmap::Rgb8 { data, w, h } | Bitmap::Rgba8 { data, w, h } => (data, *w, *h),
            }
        }
    };
}

impl Bitmap {
    bitmap_accessors!(raw);

    pub fn width(&self) -> u32 {
        match self {
            Bitmap::Rgb8 { w, .. } | Bitmap::Rgba8 { w, .. } => *w,
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            Bitmap::Rgb8 { h, .. } | Bitmap::Rgba8 { h, .. } => *h,
        }
    }

    pub fn channels(&self) -> usize {
        match self {
            Bitmap::Rgb8 { .. } => 3,
            Bitmap::Rgba8 { .. } => 4,
        }
    }

    pub fn new_rgb(data: Vec<u8>, w: u32, h: u32) -> Self {
        assert_eq!(data.len(), w as usize * h as usize * 3);
        Bitmap::Rgb8 { data, w, h }
    }

    pub fn new_rgba(data: Vec<u8>, w: u32, h: u32) -> Self {
        assert_eq!(data.len(), w as usize * h as usize * 4);
        Bitmap::Rgba8 { data, w, h }
    }

    /// Convert a decoded `image`-crate image into a Bitmap without extra
    /// copies for the common RGB/RGBA layouts.
    pub fn from_dynamic(img: crate::image::DynamicImage) -> Self {
        use crate::image::DynamicImage;
        match img {
            DynamicImage::ImageRgb8(b) => {
                let (w, h) = (b.width(), b.height());
                Bitmap::new_rgb(b.into_raw(), w, h)
            }
            DynamicImage::ImageRgba8(b) => {
                let (w, h) = (b.width(), b.height());
                Bitmap::new_rgba(b.into_raw(), w, h)
            }
            other => {
                if other.color().has_alpha() {
                    let b = other.to_rgba8();
                    let (w, h) = (b.width(), b.height());
                    Bitmap::new_rgba(b.into_raw(), w, h)
                } else {
                    let b = other.to_rgb8();
                    let (w, h) = (b.width(), b.height());
                    Bitmap::new_rgb(b.into_raw(), w, h)
                }
            }
        }
    }

    /// EXIF orientation applied on the (small, post-resize) thumbnail.
    /// Cheap by construction: it only ever touches thumbnail-sized buffers.
    pub fn apply_orientation(self, o: Orientation) -> Self {
        match o.0 {
            1 | 0 => self,
            2 => self.flip(true),
            3 => self.rotate180(),
            4 => self.flip(false),
            5 => self.transpose(),
            6 => self.rotate90(),
            7 => self.transverse(),
            8 => self.rotate270(),
            _ => self,
        }
    }

    fn rotate90(self) -> Self {
        // 90° CW: dst(x, y) = src(y, H-1-x) — axes swap.
        self.transform_swapping(|x, y, w, h| (y, h - 1 - x, h, w))
    }

    fn rotate270(self) -> Self {
        // 90° CCW: dst(x, y) = src(W-1-y, x) — axes swap.
        self.transform_swapping(|x, y, w, h| (w - 1 - y, x, h, w))
    }

    fn rotate180(self) -> Self {
        self.transform(|x, y, w, h| (w - 1 - x, h - 1 - y, w, h))
    }

    /// Horizontal mirror when `horiz`, else vertical.
    fn flip(self, horiz: bool) -> Self {
        self.transform(|x, y, w, h| {
            if horiz {
                (w - 1 - x, y, w, h)
            } else {
                (x, h - 1 - y, w, h)
            }
        })
    }

    /// Top-left↔bottom-right transpose (EXIF 5).
    fn transpose(self) -> Self {
        self.transform_swapping(|x, y, w, h| (y, x, h, w))
    }

    /// Anti-transpose (EXIF 7): transpose + rotate180.
    fn transverse(self) -> Self {
        self.transpose().rotate180()
    }

    /// Copy pixels through `map(dst_x, dst_y, src_w, src_h) -> (src_x, src_y,
    /// out_w, out_h)` keeping the source dimensions (no axis swap).
    fn transform(self, map: impl Fn(u32, u32, u32, u32) -> (u32, u32, u32, u32)) -> Self {
        let ch = self.channels();
        let (src, w, h) = self.raw_owned();
        let mut dst = vec![0u8; src.len()];
        for y in 0..h {
            for x in 0..w {
                let (sx, sy, ow, oh) = map(x, y, w, h);
                debug_assert_eq!((ow, oh), (w, h));
                let d = px_index(x, y, w, ch);
                let s = px_index(sx, sy, w, ch);
                copy_px(&mut dst[d..], &src[s..], ch);
            }
        }
        rebuild_kind(ch, dst, w, h)
    }

    /// Same as [`Bitmap::transform`] but for maps that swap the axes.
    fn transform_swapping(self, map: impl Fn(u32, u32, u32, u32) -> (u32, u32, u32, u32)) -> Self {
        let ch = self.channels();
        let (src, w, h) = self.raw_owned();
        let (nw, nh) = (h, w);
        let mut dst = vec![0u8; src.len()];
        for y in 0..nh {
            for x in 0..nw {
                let (sx, sy, ow, oh) = map(x, y, w, h);
                debug_assert_eq!((ow, oh), (nw, nh));
                let d = px_index(x, y, nw, ch);
                let s = px_index(sx, sy, w, ch);
                copy_px(&mut dst[d..], &src[s..], ch);
            }
        }
        rebuild_kind(ch, dst, nw, nh)
    }

    fn raw_owned(self) -> (Vec<u8>, u32, u32) {
        let (w, h) = (self.width(), self.height());
        let data = match self {
            Bitmap::Rgb8 { data, .. } | Bitmap::Rgba8 { data, .. } => data,
        };
        (data, w, h)
    }

    /// Flatten alpha onto a white background, yielding RGB.
    pub fn flatten_to_rgb(self) -> Bitmap {
        match self {
            Bitmap::Rgb8 { .. } => self,
            Bitmap::Rgba8 { data, w, h } => {
                let mut out = Vec::with_capacity(data.len() / 4 * 3);
                for px in data.chunks_exact(4) {
                    let a = px[3] as u32;
                    out.push(((px[0] as u32 * a + (255 - a) * 255) / 255) as u8);
                    out.push(((px[1] as u32 * a + (255 - a) * 255) / 255) as u8);
                    out.push(((px[2] as u32 * a + (255 - a) * 255) / 255) as u8);
                }
                Bitmap::new_rgb(out, w, h)
            }
        }
    }
}

fn rebuild_kind(ch: usize, data: Vec<u8>, w: u32, h: u32) -> Bitmap {
    if ch == 4 {
        Bitmap::new_rgba(data, w, h)
    } else {
        Bitmap::new_rgb(data, w, h)
    }
}

/// Copy one pixel between flat buffers whose indices the caller computed.
#[inline]
fn copy_px(dst: &mut [u8], src: &[u8], ch: usize) {
    dst[..ch].copy_from_slice(&src[..ch]);
}

/// Flat-buffer pixel index helper for transform maps.
#[inline]
fn px_index(x: u32, y: u32, stride: u32, ch: usize) -> usize {
    (y as usize * stride as usize + x as usize) * ch
}

/// Two-pass integer box downscale by `f` (≥ 2). Edge boxes average over the
/// pixels they actually cover, so the result is exactly a box filter.
pub fn box_downscale(bm: &Bitmap, f: u32) -> Bitmap {
    let (src, w, h) = bm.raw();
    debug_assert!(f >= 2);
    let (ow, oh) = (w.div_ceil(f), h.div_ceil(f));
    match bm {
        Bitmap::Rgb8 { .. } => Bitmap::new_rgb(box_rows::<3>(src, w, h, f), ow, oh),
        Bitmap::Rgba8 { .. } => Bitmap::new_rgba(box_rows::<4>(src, w, h, f), ow, oh),
    }
}

struct RoundedMean {
    count: u32,
    reciprocal: u64,
}

impl RoundedMean {
    fn new(count: u32) -> Self {
        Self {
            count,
            reciprocal: (1u64 << 32) / u64::from(count),
        }
    }

    fn of(&self, sum: u32) -> u32 {
        let value = sum + self.count / 2;
        let quotient = (u64::from(value) * self.reciprocal >> 32) as u32;
        // The floor reciprocal can underestimate the quotient by one.
        quotient + u32::from(value - quotient * self.count >= self.count)
    }
}

fn box_rows<const CHANNELS: usize>(src: &[u8], w: u32, h: u32, f: u32) -> Vec<u8> {
    if src.is_empty() {
        return Vec::new();
    }
    let stride = w as usize * CHANNELS;
    let mut vertical = vec![0u32; stride];
    let mut output = Vec::with_capacity(w.div_ceil(f) as usize * h.div_ceil(f) as usize * CHANNELS);
    let filter = BoxRowFilter::new(w, f);
    for group in src.chunks(stride * f.min(h) as usize) {
        vertical.fill(0);
        for row in group.chunks_exact(stride) {
            for (sum, &byte) in vertical.iter_mut().zip(row) {
                *sum += u32::from(byte);
            }
        }
        filter.append::<CHANNELS>(&mut vertical, (group.len() / stride) as u32, &mut output);
    }
    output
}

pub(super) struct BoxRowFilter {
    full: RoundedMean,
    right: RoundedMean,
    factor: u32,
}

impl BoxRowFilter {
    pub(super) fn new(width: u32, factor: u32) -> Self {
        Self {
            full: RoundedMean::new(factor),
            right: RoundedMean::new(if width % factor == 0 {
                factor
            } else {
                width % factor
            }),
            factor,
        }
    }

    pub(super) fn append<const CHANNELS: usize>(
        &self,
        vertical: &mut [u32],
        rows: u32,
        output: &mut Vec<u8>,
    ) {
        let mean = RoundedMean::new(rows);
        for sum in vertical.iter_mut() {
            *sum = mean.of(*sum);
        }
        for block in vertical.chunks(self.factor as usize * CHANNELS) {
            let mut sums = [0u32; CHANNELS];
            for pixel in block.chunks_exact(CHANNELS) {
                for c in 0..CHANNELS {
                    sums[c] += pixel[c];
                }
            }
            let mean = if block.len() / CHANNELS == self.factor as usize {
                &self.full
            } else {
                &self.right
            };
            output.extend(sums.map(|sum| mean.of(sum) as u8));
        }
    }
}

/// Full resize strategy: optional single box pre-pass when the downscale
/// ratio is large, then the SIMD convolution resample to the exact target.
pub fn resize_to(bm: Bitmap, tw: u32, th: u32) -> Bitmap {
    let (w, h) = (bm.width(), bm.height());
    if tw == 0 || th == 0 || (tw == w && th == h) {
        return bm;
    }

    // Choose the largest integer factor that keeps the intermediate at or
    // above the target in both axes (so the final pass still downsamples),
    // capped so one box pass is enough.
    let f = w
        .checked_div(tw)
        .unwrap_or(0)
        .min(h.checked_div(th).unwrap_or(0))
        .min(64);
    let mid = if f >= 2 && (w / f >= tw && h / f >= th) {
        box_downscale(&bm, f)
    } else {
        bm
    };

    fir_resize(mid, tw, th)
}

/// Final resample via `fast_image_resize` (pure Rust, SIMD convolution).
/// Bilinear for small thumbnails, Lanczos3 when quality matters more.
pub(super) fn fir_resize(bm: Bitmap, tw: u32, th: u32) -> Bitmap {
    use crate::image_resize::{PixelType, Resizer};

    let (w, h) = (bm.width(), bm.height());
    if tw == w && th == h {
        return bm;
    }
    let ch = bm.channels();
    let (data, _, _) = bm.raw_owned();
    let pixel_type = if ch == 4 {
        PixelType::U8x4
    } else {
        PixelType::U8x3
    };
    let src = crate::image_resize::images::Image::from_vec_u8(w, h, data, pixel_type)
        .expect("bitmap buffer matches dimensions");
    let mut dst = crate::image_resize::images::Image::new(tw, th, pixel_type);
    let mut resizer = Resizer::new();
    let filter = if tw.max(th) < 200 {
        crate::image_resize::FilterType::Bilinear
    } else {
        crate::image_resize::FilterType::Lanczos3
    };
    resizer
        .resize(&src, &mut dst, filter)
        .expect("fast_image_resize never fails on valid buffers");
    let out = dst.into_vec();
    if ch == 4 {
        Bitmap::new_rgba(out, tw, th)
    } else {
        Bitmap::new_rgb(out, tw, th)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/media/thumbnails/scale.rs"]
mod tests;

pub fn center_crop(bitmap: Bitmap, width: u32, height: u32) -> Bitmap {
    let (bytes, source_width, source_height) = bitmap.raw();
    let width = width.min(source_width);
    let height = height.min(source_height);
    if width == source_width && height == source_height {
        return bitmap;
    }
    let channels = bitmap.channels();
    let left = (source_width - width) / 2;
    let top = (source_height - height) / 2;
    let mut cropped = Vec::with_capacity(width as usize * height as usize * channels);
    for row in top..top + height {
        let start = (row as usize * source_width as usize + left as usize) * channels;
        cropped.extend_from_slice(&bytes[start..start + width as usize * channels]);
    }
    if channels == 3 {
        Bitmap::new_rgb(cropped, width, height)
    } else {
        Bitmap::new_rgba(cropped, width, height)
    }
}

pub(super) fn crop_size(sw: u32, sh: u32, target: Option<(u32, u32)>) -> (u32, u32) {
    let Some((width, height)) = target else {
        return (sw, sh);
    };
    if u64::from(sw) * u64::from(height) > u64::from(sh) * u64::from(width) {
        (
            (u64::from(sh) * u64::from(width) / u64::from(height)).max(1) as u32,
            sh,
        )
    } else {
        (
            sw,
            (u64::from(sw) * u64::from(height) / u64::from(width)).max(1) as u32,
        )
    }
}

pub(super) fn crop_for_fill(bitmap: Bitmap, target: Option<(u32, u32)>) -> Bitmap {
    let (w, h) = crop_size(bitmap.width(), bitmap.height(), target);
    center_crop(bitmap, w, h)
}

pub(super) fn resize_to_fill(bitmap: Bitmap, width: u32, height: u32) -> Bitmap {
    resize_to(crop_for_fill(bitmap, Some((width, height))), width, height)
}
