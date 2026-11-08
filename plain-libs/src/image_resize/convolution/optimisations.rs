use crate::image_resize::convolution::Coefficients;
// This code is based on C-implementation from Pillow-SIMD package for Python
// https://github.com/uploadcare/pillow-simd

// 8 bits for a result. Filter can have negative areas.
// In one case, the sum of the coefficients will be negative,
// in the other it will be more than 1.0. That is why we need
// two extra bits for overflow and i32 type.
const PRECISION_BITS: u8 = 32 - 8 - 2;
// We use i16 type to store coefficients.
const MAX_COEFFS_PRECISION: u8 = 16 - 1;

#[derive(Debug, Clone)]
pub(crate) struct CoefficientsI16Chunk {
    pub start: u32,
    values: Vec<i16>,
}

impl CoefficientsI16Chunk {
    #[inline(always)]
    pub fn values(&self) -> &[i16] {
        &self.values
    }
}

pub(crate) struct Normalizer16 {
    precision: u8,
    chunks: Vec<CoefficientsI16Chunk>,
}

impl Normalizer16 {
    #[inline]
    pub fn new(coefficients: Coefficients) -> Self {
        let max_weight = coefficients
            .values
            .iter()
            .max_by(|&x, &y| x.partial_cmp(y).unwrap())
            .unwrap_or(&0.0)
            .to_owned();

        let mut precision = 0u8;
        for cur_precision in 0..PRECISION_BITS {
            precision = cur_precision;
            let next_value: i32 = (max_weight * (1 << (precision + 1)) as f64).round() as i32;
            if next_value >= (1 << MAX_COEFFS_PRECISION) {
                // The next value will be outside the range, so stop
                break;
            }
        }
        debug_assert!(precision >= 4); // required for some SIMD optimisations

        let mut chunks = Vec::with_capacity(coefficients.bounds.len());
        if coefficients.window_size > 0 {
            let scale = (1 << precision) as f64;
            let coef_chunks = coefficients.values.chunks_exact(coefficients.window_size);
            for (chunk, bound) in coef_chunks.zip(&coefficients.bounds) {
                let chunk_i16: Vec<i16> = chunk
                    .iter()
                    .take(bound.size as usize)
                    .map(|&v| (v * scale).round() as i16)
                    .collect();
                chunks.push(CoefficientsI16Chunk {
                    start: bound.start,
                    values: chunk_i16,
                });
            }
        }

        Self { precision, chunks }
    }

    #[inline(always)]
    pub fn precision(&self) -> u8 {
        self.precision
    }

    #[inline(always)]
    pub fn chunks(&self) -> &[CoefficientsI16Chunk] {
        &self.chunks
    }

    pub fn chunks_len(&self) -> usize {
        self.chunks.len()
    }

    #[inline(always)]
    pub fn clip(&self, value: i32) -> u8 {
        (value >> self.precision).clamp(0, 255) as u8
    }
}

macro_rules! try_process_in_threads_h {
    {$op: ident($src_view: ident, $dst_view: ident, $offset: ident, $($arg: ident),+$(,)?);}  => {

        'block: {
            #[cfg(feature = "thumbnails")]
            {
                use crate::image_resize::threading::split_h_two_images_for_threading;
                use crate::image_resize::threading::for_each_part;

                if let Some(iter) = split_h_two_images_for_threading($src_view, $dst_view, $offset) {
                    for_each_part(iter, |(src, mut dst)| {
                        $op(&src, &mut dst, 0, $($arg),+);
                    });
                    break 'block;
                }
            }
            $op($src_view, $dst_view, $offset, $($arg),+);
        }
    };
}

macro_rules! try_process_in_threads_v {
    {$op: ident($src_view: ident, $dst_view: ident, $offset: ident, $($arg: ident),+$(,)?);}  => {

        'block: {
            #[cfg(feature = "thumbnails")]
            {
                use crate::image_resize::threading::split_v_two_images_for_threading;
                use crate::image_resize::threading::for_each_part;

                if let Some(iter) = split_v_two_images_for_threading($src_view, $dst_view, $offset) {
                    for_each_part(iter, |(src, mut dst)| {
                        $op(&src, &mut dst, 0, $($arg),+);
                    });
                    break 'block;
                }
            }
            $op($src_view, $dst_view, $offset, $($arg),+);
        }
    };
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/image_resize/convolution/optimisations/tests.rs"]
mod tests;
