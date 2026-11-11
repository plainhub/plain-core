use super::super::{BitMask, Tag};
#[cfg(target_arch = "x86")]
use core::arch::x86;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64 as x86;
use core::mem;
use core::num::NonZeroU16;
pub(crate) type BitMaskWord = u16;
pub(crate) type NonZeroBitMaskWord = NonZeroU16;
pub(crate) const BITMASK_STRIDE: usize = 1;
pub(crate) const BITMASK_ITER_MASK: BitMaskWord = !0;
#[derive(Copy, Clone)]
pub(crate) struct Group(x86::__m128i);
impl Group {
    pub(crate) const WIDTH: usize = mem::size_of::<Self>();
    #[inline]
    pub(crate) const fn static_empty() -> &'static [Tag; Group::WIDTH] {
        #[repr(C)]
        struct AlignedTags {
            _align: [Group; 0],
            tags: [Tag; Group::WIDTH],
        }
        const ALIGNED_TAGS: AlignedTags = AlignedTags {
            _align: [],
            tags: [Tag::EMPTY; Group::WIDTH],
        };
        &ALIGNED_TAGS.tags
    }
    #[inline]
    pub(crate) unsafe fn load(ptr: *const Tag) -> Self {
        unsafe { Group(x86::_mm_loadu_si128(ptr.cast())) }
    }
    #[inline]
    pub(crate) unsafe fn load_aligned(ptr: *const Tag) -> Self {
        debug_assert_eq!(ptr.align_offset(mem::align_of::<Self>()), 0);
        unsafe { Group(x86::_mm_load_si128(ptr.cast())) }
    }
    #[inline]
    pub(crate) unsafe fn store_aligned(self, ptr: *mut Tag) {
        debug_assert_eq!(ptr.align_offset(mem::align_of::<Self>()), 0);
        unsafe {
            x86::_mm_store_si128(ptr.cast(), self.0);
        }
    }
    #[inline]
    pub(crate) fn match_tag(self, tag: Tag) -> BitMask {
        #[expect(
            clippy::cast_possible_wrap,
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation
        )]
        unsafe {
            let cmp = x86::_mm_cmpeq_epi8(self.0, x86::_mm_set1_epi8(tag.0 as i8));
            BitMask(x86::_mm_movemask_epi8(cmp) as u16)
        }
    }
    #[inline]
    pub(crate) fn match_empty(self) -> BitMask {
        self.match_tag(Tag::EMPTY)
    }
    #[inline]
    pub(crate) fn match_empty_or_deleted(self) -> BitMask {
        unsafe { BitMask(x86::_mm_movemask_epi8(self.0) as u16) }
    }
    #[inline]
    pub(crate) fn match_full(&self) -> BitMask {
        BitMask(!self.match_empty_or_deleted().0)
    }
    #[inline]
    pub(crate) fn convert_special_to_empty_and_full_to_deleted(self) -> Self {
        unsafe {
            let zero = x86::_mm_setzero_si128();
            let special = x86::_mm_cmpgt_epi8(zero, self.0);
            Group(x86::_mm_or_si128(
                special,
                x86::_mm_set1_epi8(Tag::DELETED.0 as i8),
            ))
        }
    }
}
