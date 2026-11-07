#[inline(always)]
pub(crate) fn mul_div_255(a: u8, b: u8) -> u8 {
    let tmp = a as u32 * b as u32 + 128;
    (((tmp >> 8) + tmp) >> 8) as u8
}

const fn recip_alpha_array(precision: u32) -> [u32; 256] {
    let mut res = [0; 256];
    let scale = 1 << (precision + 1);
    let scaled_max = 255 * scale;
    let mut i: usize = 1;
    while i < 256 {
        res[i] = ((scaled_max / i as u32) + 1) >> 1;
        i += 1;
    }
    res
}

const PRECISION: u32 = 8;
const ROUND_CORRECTION: u32 = 1 << (PRECISION - 1);

#[inline(always)]
pub(crate) fn div_and_clip(v: u8, recip_alpha: u32) -> u8 {
    ((v as u32 * recip_alpha + ROUND_CORRECTION) >> PRECISION).min(0xff) as u8
}

pub(crate) const RECIP_ALPHA: [u32; 256] = recip_alpha_array(PRECISION);

macro_rules! process_two_images {
    {$op: ident($src_view: ident, $dst_view: ident, $($arg: ident),+);}  => {

        'block: {
            #[cfg(feature = "thumbnails")]
            {
                use crate::image_resize::threading::split_h_two_images_for_threading;
                use rayon::prelude::*;

                if let Some(iter) = split_h_two_images_for_threading($src_view, $dst_view, 0) {
                    iter.for_each(|(src, mut dst)| {
                        $op(&src, &mut dst, $($arg),+);
                    });
                    break 'block;
                }
            }
            $op($src_view, $dst_view, $($arg),+);
        }
    };
}

macro_rules! process_one_images {
    {$op: ident($image_view: ident, $($arg: ident),+);} => {

        'block: {
            #[cfg(feature = "thumbnails")]
            {
                use crate::image_resize::threading::split_h_one_image_for_threading;
                use rayon::prelude::*;

                if let Some(iter) = split_h_one_image_for_threading($image_view) {
                    iter.for_each(|mut img| {
                        $op(&mut img, $($arg),+);
                    });
                    break 'block;
                }
            }
            $op($image_view, $($arg),+);
        }
    };
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/image_resize/alpha/common/tests.rs"]
mod tests;
