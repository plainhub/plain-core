use crate::image_codecs::jpeg::decoder::IDCTPtr;
use crate::image_codecs::jpeg::idct::scalar::{idct_int, idct_int_1x1};
use crate::image_codecs::zune::log::debug;
use crate::image_codecs::zune::options::DecoderOptions;
pub mod avx2;
pub mod neon;
pub mod scalar;
pub fn choose_idct_func(options: &DecoderOptions) -> IDCTPtr {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if options.use_avx2() {
            debug!("Using vector integer IDCT");
            return |a: &mut [i32; 64], b: &mut [i16], c: usize| unsafe {
                avx2::idct_avx2(a, b, c)
            };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if options.use_neon() {
            debug!("Using vector integer IDCT");
            return |a: &mut [i32; 64], b: &mut [i16], c: usize| unsafe {
                neon::idct_neon(a, b, c)
            };
        }
    }
    debug!("Using scalar integer IDCT");
    return idct_int;
}
pub fn choose_idct_4x4_func(_options: &DecoderOptions) -> IDCTPtr {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if _options.use_avx2() {
            debug!("Using vector integer IDCT");
            return |a: &mut [i32; 64], b: &mut [i16], c: usize| unsafe {
                avx2::idct_avx2_4x4(a, b, c)
            };
        }
    }
    scalar::idct4x4
}
pub fn choose_idct_1x1_func(_: &DecoderOptions) -> IDCTPtr {
    idct_int_1x1
}
