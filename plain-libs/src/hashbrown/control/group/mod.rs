cfg_if! { if #[cfg (all (target_feature = "sse2" , any (target_arch = "x86" , target_arch = "x86_64") , not (miri) ,))] { mod sse2 ; use sse2 as imp ; } else if #[cfg (all (target_arch = "aarch64" , target_feature = "neon" , target_endian = "little" , not (miri) ,))] { mod neon ; use neon as imp ; } else { mod generic ; use generic as imp ; } }
pub(crate) use self::imp::Group;
pub(super) use self::imp::{BITMASK_ITER_MASK, BITMASK_STRIDE, BitMaskWord, NonZeroBitMaskWord};
