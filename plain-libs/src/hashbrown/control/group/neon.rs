use super::super::{BitMask, Tag};
use core::arch::aarch64 as neon;
use core::mem;
use core::num::NonZeroU64;
pub(crate) type BitMaskWord = u64;
pub(crate) type NonZeroBitMaskWord = NonZeroU64;
pub(crate) const BITMASK_STRIDE: usize = 8;
pub(crate) const BITMASK_ITER_MASK: BitMaskWord = 0x8080_8080_8080_8080;
#[derive(Copy, Clone)]
pub(crate) struct Group(neon::uint8x8_t);
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
        unsafe { Group(neon::vld1_u8(ptr.cast())) }
    }
    #[inline]
    pub(crate) unsafe fn load_aligned(ptr: *const Tag) -> Self {
        debug_assert_eq!(ptr.align_offset(mem::align_of::<Self>()), 0);
        unsafe { Group(neon::vld1_u8(ptr.cast())) }
    }
    #[inline]
    pub(crate) unsafe fn store_aligned(self, ptr: *mut Tag) {
        debug_assert_eq!(ptr.align_offset(mem::align_of::<Self>()), 0);
        unsafe {
            neon::vst1_u8(ptr.cast(), self.0);
        }
    }
    #[inline]
    pub(crate) fn match_tag(self, tag: Tag) -> BitMask {
        unsafe {
            let cmp = neon::vceq_u8(self.0, neon::vdup_n_u8(tag.0));
            BitMask(neon::vget_lane_u64(neon::vreinterpret_u64_u8(cmp), 0))
        }
    }
    #[inline]
    pub(crate) fn match_empty(self) -> BitMask {
        self.match_tag(Tag::EMPTY)
    }
    #[inline]
    pub(crate) fn match_empty_or_deleted(self) -> BitMask {
        unsafe {
            let cmp = neon::vcltz_s8(neon::vreinterpret_s8_u8(self.0));
            BitMask(neon::vget_lane_u64(neon::vreinterpret_u64_u8(cmp), 0))
        }
    }
    #[inline]
    pub(crate) fn match_full(self) -> BitMask {
        unsafe {
            let cmp = neon::vcgez_s8(neon::vreinterpret_s8_u8(self.0));
            BitMask(neon::vget_lane_u64(neon::vreinterpret_u64_u8(cmp), 0))
        }
    }
    #[inline]
    pub(crate) fn convert_special_to_empty_and_full_to_deleted(self) -> Self {
        unsafe {
            let special = neon::vcltz_s8(neon::vreinterpret_s8_u8(self.0));
            Group(neon::vorr_u8(special, neon::vdup_n_u8(0x80)))
        }
    }
}
