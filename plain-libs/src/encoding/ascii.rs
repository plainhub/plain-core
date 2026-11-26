cfg_if::cfg_if! { if #[cfg(all())] { # [inline (never)] pub (crate) fn is_ascii (s : & [u8 ; STRIDE]) -> bool { s . iter () . all (| b | * b < 0x80) }   # [inline (never)] pub (crate) fn copy_stride (src_stride : & [u8 ; STRIDE] , dst_stride : & mut [u8 ; STRIDE]) { * dst_stride = * src_stride ; }   # [inline (never)] fn copy_stride_tail (src_stride : & [u8 ; 16] , dst_stride : & mut [u8 ; 16]) -> (u8 , usize) { for (i , (s , d)) in src_stride . iter () . zip (dst_stride . iter_mut ()) . enumerate () { let c = * s ; if c >= 0x80 { return (c , i) ; } * d = c ; } debug_assert ! (false) ; (0 , 0) }   # [inline (never)] fn validate_ascii_stride_tail (stride : & [u8 ; 16]) -> (u8 , usize) { for (i , s) in stride . iter () . enumerate () { let b = * s ; if b >= 0x80 { return (b , i) ; } } debug_assert ! (false) ; (0 , 0) }  # [inline (always)] fn ascii_to_ascii_stride (src_stride : & [u8 ; STRIDE] , dst_stride : & mut [u8 ; STRIDE] ,) -> Option < (u8 , usize) > { if is_ascii (src_stride) { copy_stride (src_stride , dst_stride) ; return None ; } Some (copy_stride_tail (src_stride , dst_stride)) }   # [inline (always)] fn validate_ascii_stride (stride : & [u8 ; STRIDE] ,) -> Option < (u8 , usize) > { if is_ascii (stride) { return None ; } Some (validate_ascii_stride_tail (stride)) }  } }
pub(crate) const STRIDE: usize = 16;

macro_rules! ascii_copy_impl_single {
    ($ name : ident , $ stride : ident , $ double_stride : ident , $ src_unit : ty , $ dst_unit : ty) => {
        #[inline(always)]
        pub fn $name(src: &[$src_unit], dst: &mut [$dst_unit]) -> Option<($src_unit, usize)> {
            let len = core::cmp::min(src.len(), dst.len());
            let mut consumed = 0usize;
            let (src_strides, src_tail) = src[..len].as_chunks::<STRIDE>();
            let (dst_strides, dst_tail) = dst[..len].as_chunks_mut::<STRIDE>();
            for (src_stride, dst_stride) in src_strides.iter().zip(dst_strides.iter_mut()) {
                if let Some((c, pos)) = $stride(src_stride, dst_stride) {
                    return Some((c, consumed + pos));
                }
                consumed += STRIDE;
            }
            for (src_slot, dst_slot) in src_tail.iter().zip(dst_tail.iter_mut()) {
                let c = *src_slot;
                if c >= 0x80 {
                    return Some((c, consumed));
                }
                *dst_slot = c as $dst_unit;
                consumed += 1;
            }
            None
        }
    };
}
#[inline(always)]
fn ascii_valid_impl(bytes: &[u8]) -> Option<(u8, usize)> {
    let (strides, tail) = bytes.as_chunks::<STRIDE>();
    for (index, stride) in strides.iter().enumerate() {
        if let Some((byte, offset)) = validate_ascii_stride(stride) {
            return Some((byte, index * STRIDE + offset));
        }
    }
    let offset = strides.len() * STRIDE;
    tail.iter()
        .position(|byte| !byte.is_ascii())
        .map(|index| (tail[index], offset + index))
}
cfg_if::cfg_if! { if #[cfg(all())] { ascii_copy_impl_single ! (ascii_to_ascii_impl , ascii_to_ascii_stride , ascii_to_ascii_double_stride , u8 , u8) ;  } }
cfg_if::cfg_if! { if #[cfg(all())] {  } }
macro_rules! ascii_copy {
    ($ name : ident , $ impl : ident , $ src_unit : ty , $ dst_unit : ty) => {
        #[inline(always)]
        pub(crate) fn $name(
            src: &[$src_unit],
            dst: &mut [$dst_unit],
        ) -> Option<($src_unit, usize)> {
            $impl(src, dst)
        }
    };
}
ascii_copy!(ascii_to_ascii, ascii_to_ascii_impl, u8, u8);
#[inline(always)]
pub(crate) fn validate_ascii(bytes: &[u8]) -> Option<(u8, usize)> {
    ascii_valid_impl(bytes)
}
#[inline(always)]
pub(crate) fn ascii_valid_up_to(bytes: &[u8]) -> usize {
    ascii_valid_impl(bytes)
        .map(|(_, pos)| pos)
        .unwrap_or(bytes.len())
}
