use super::super::{BitMask, Tag};
use core::{mem, ptr};
cfg_if! { if #[cfg (any (target_pointer_width = "64" , target_arch = "aarch64" , target_arch = "x86_64" , target_arch = "wasm32" ,))] { type GroupWord = u64 ; type NonZeroGroupWord = core::num::NonZeroU64 ; } else { type GroupWord = u32 ; type NonZeroGroupWord = core::num::NonZeroU32 ; } }
pub(crate) type BitMaskWord = GroupWord;
pub(crate) type NonZeroBitMaskWord = NonZeroGroupWord;
pub(crate) const BITMASK_STRIDE: usize = 8;
const BITMASK_MASK: BitMaskWord = u64::from_ne_bytes([Tag::DELETED.0; 8]) as GroupWord;
pub(crate) const BITMASK_ITER_MASK: BitMaskWord = !0;
#[inline]
fn repeat(tag: Tag) -> GroupWord {
    GroupWord::from_ne_bytes([tag.0; Group::WIDTH])
}
#[derive(Copy, Clone)]
pub(crate) struct Group(GroupWord);
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
        unsafe { Group(ptr::read_unaligned(ptr.cast())) }
    }
    #[inline]
    pub(crate) unsafe fn load_aligned(ptr: *const Tag) -> Self {
        debug_assert_eq!(ptr.align_offset(mem::align_of::<Self>()), 0);
        unsafe { Group(ptr::read(ptr.cast())) }
    }
    #[inline]
    pub(crate) unsafe fn store_aligned(self, ptr: *mut Tag) {
        debug_assert_eq!(ptr.align_offset(mem::align_of::<Self>()), 0);
        unsafe {
            ptr::write(ptr.cast(), self.0);
        }
    }
    #[inline]
    pub(crate) fn match_tag(self, tag: Tag) -> BitMask {
        let cmp = self.0 ^ repeat(tag);
        BitMask((cmp.wrapping_sub(repeat(Tag(0x01))) & !cmp & repeat(Tag::DELETED)).to_le())
    }
    #[inline]
    pub(crate) fn match_empty(self) -> BitMask {
        BitMask((self.0 & (self.0 << 1) & repeat(Tag::DELETED)).to_le())
    }
    #[inline]
    pub(crate) fn match_empty_or_deleted(self) -> BitMask {
        BitMask((self.0 & repeat(Tag::DELETED)).to_le())
    }
    #[inline]
    pub(crate) fn match_full(self) -> BitMask {
        BitMask(self.match_empty_or_deleted().0 ^ BITMASK_MASK)
    }
    #[inline]
    pub(crate) fn convert_special_to_empty_and_full_to_deleted(self) -> Self {
        let full = !self.0 & repeat(Tag::DELETED);
        Group(!full + (full >> 7))
    }
}
