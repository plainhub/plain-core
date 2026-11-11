use crate::image_codecs::jpeg::components::UpSampler;
use crate::image_codecs::zune::options::DecoderOptions;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod avx2;
#[cfg(target_arch = "aarch64")]
mod neon;
mod scalar;
pub fn choose_horizontal_samp_function(options: &DecoderOptions) -> UpSampler {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if options.use_avx2() {
        return |a: &[i16], b: &[i16], c: &[i16], d: &mut [i16], e: &mut [i16]| unsafe {
            avx2::upsample_horizontal_avx2(a, b, c, d, e)
        };
    }
    #[cfg(target_arch = "aarch64")]
    if options.use_neon() {
        return |a: &[i16], b: &[i16], c: &[i16], d: &mut [i16], e: &mut [i16]| unsafe {
            neon::upsample_horizontal_neon(a, b, c, d, e)
        };
    }
    return scalar::upsample_horizontal;
}
pub fn choose_hv_samp_function(options: &DecoderOptions) -> UpSampler {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if options.use_avx2() {
        return |a: &[i16], b: &[i16], c: &[i16], d: &mut [i16], e: &mut [i16]| unsafe {
            avx2::upsample_hv_avx2(a, b, c, d, e)
        };
    }
    #[cfg(target_arch = "aarch64")]
    if options.use_neon() {
        return |a: &[i16], b: &[i16], c: &[i16], d: &mut [i16], e: &mut [i16]| unsafe {
            neon::upsample_hv_neon(a, b, c, d, e)
        };
    }
    return scalar::upsample_hv;
}
pub fn choose_v_samp_function(options: &DecoderOptions) -> UpSampler {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if options.use_avx2() {
        return |a: &[i16], b: &[i16], c: &[i16], d: &mut [i16], e: &mut [i16]| unsafe {
            avx2::upsample_vertical_avx2(a, b, c, d, e)
        };
    }
    #[cfg(target_arch = "aarch64")]
    if options.use_neon() {
        return |a: &[i16], b: &[i16], c: &[i16], d: &mut [i16], e: &mut [i16]| unsafe {
            neon::upsample_vertical_neon(a, b, c, d, e)
        };
    }
    return scalar::upsample_vertical;
}
pub fn upsample_no_op(
    _input: &[i16],
    _in_ref: &[i16],
    _in_near: &[i16],
    _scratch_space: &mut [i16],
    _output: &mut [i16],
) {
}
pub fn generic_sampler() -> UpSampler {
    scalar::upsample_generic
}
