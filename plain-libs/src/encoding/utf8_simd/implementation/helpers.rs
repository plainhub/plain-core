type Utf8ErrorCompat = crate::encoding::utf8_simd::compat::Utf8Error;
#[inline]
pub(crate) fn validate_utf8_at_offset(input: &[u8], offset: usize) -> Result<(), Utf8ErrorCompat> {
    match core::str::from_utf8(&input[offset..]) {
        Ok(_) => Ok(()),
        Err(err) => Err(Utf8ErrorCompat {
            valid_up_to: err.valid_up_to() + offset,
            error_len: err.error_len().map(|len| len as u8),
        }),
    }
}
#[cold]
pub(crate) fn get_compat_error(input: &[u8], failing_block_pos: usize) -> Utf8ErrorCompat {
    let offset = if failing_block_pos == 0 {
        0
    } else {
        (1..=3)
            .find(|i| input[failing_block_pos - i] >> 6 != 0b10)
            .map_or(failing_block_pos, |i| failing_block_pos - i)
    };
    validate_utf8_at_offset(input, offset).unwrap_err()
}
pub(crate) unsafe fn memcpy_unaligned_nonoverlapping_inline_opt_lt_64(
    mut src: *const u8,
    mut dest: *mut u8,
    mut len: usize,
) {
    unsafe {
        #[inline]
        unsafe fn memcpy_u64(src: &mut *const u8, dest: &mut *mut u8) {
            unsafe {
                dest.cast::<u64>()
                    .write_unaligned(src.cast::<u64>().read_unaligned());
                *src = src.offset(8);
                *dest = dest.offset(8);
            }
        }
        if len >= 32 {
            memcpy_u64(&mut src, &mut dest);
            memcpy_u64(&mut src, &mut dest);
            memcpy_u64(&mut src, &mut dest);
            memcpy_u64(&mut src, &mut dest);
            len -= 32;
        }
        if len >= 16 {
            memcpy_u64(&mut src, &mut dest);
            memcpy_u64(&mut src, &mut dest);
            len -= 16;
        }
        if len >= 8 {
            memcpy_u64(&mut src, &mut dest);
            len -= 8;
        }
        while len > 0 {
            *dest = *src;
            src = src.offset(1);
            dest = dest.offset(1);
            len -= 1;
        }
    }
}
pub(crate) const SIMD_CHUNK_SIZE: usize = 64;
#[repr(C, align(32))]
pub(crate) struct Utf8CheckAlgorithm<T> {
    pub(crate) prev: T,
    pub(crate) incomplete: T,
    pub(crate) error: T,
}
#[repr(C, align(16))]
pub(crate) struct TempSimdChunkA16(pub(crate) [u8; SIMD_CHUNK_SIZE]);
impl TempSimdChunkA16 {
    #[inline]
    pub(crate) const fn new() -> Self {
        Self([0; SIMD_CHUNK_SIZE])
    }
}
#[repr(C, align(32))]
pub(crate) struct TempSimdChunkA32(pub(crate) [u8; SIMD_CHUNK_SIZE]);
impl TempSimdChunkA32 {
    #[inline]
    pub(crate) const fn new() -> Self {
        Self([0; SIMD_CHUNK_SIZE])
    }
}
#[derive(Clone, Copy)]
pub(crate) struct SimdU8Value<T>(pub(crate) T)
where
    T: Copy;
