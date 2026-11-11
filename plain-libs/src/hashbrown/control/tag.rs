use core::{fmt, mem};
#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub(crate) struct Tag(pub(super) u8);
impl Tag {
    pub(crate) const EMPTY: Tag = Tag(0b1111_1111);
    pub(crate) const DELETED: Tag = Tag(0b1000_0000);
    #[inline]
    pub(crate) const fn is_full(self) -> bool {
        self.0 & 0x80 == 0
    }
    #[inline]
    pub(crate) const fn is_special(self) -> bool {
        self.0 & 0x80 != 0
    }
    #[inline]
    pub(crate) const fn special_is_empty(self) -> bool {
        debug_assert!(self.is_special());
        self.0 & 0x01 != 0
    }
    #[inline]
    pub(crate) const fn full(hash: u64) -> Tag {
        const MIN_HASH_LEN: usize = if mem::size_of::<usize>() < mem::size_of::<u64>() {
            mem::size_of::<usize>()
        } else {
            mem::size_of::<u64>()
        };
        let top7 = hash >> (MIN_HASH_LEN * 8 - 7);
        Tag((top7 & 0x7f) as u8)
    }
}
impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_special() {
            if self.special_is_empty() {
                f.pad("EMPTY")
            } else {
                f.pad("DELETED")
            }
        } else {
            f.debug_tuple("full").field(&(self.0 & 0x7F)).finish()
        }
    }
}
pub(crate) trait TagSliceExt {
    fn fill_tag(&mut self, tag: Tag);
    #[inline]
    fn fill_empty(&mut self) {
        self.fill_tag(Tag::EMPTY);
    }
}
impl TagSliceExt for [mem::MaybeUninit<Tag>] {
    #[inline]
    fn fill_tag(&mut self, tag: Tag) {
        unsafe { self.as_mut_ptr().write_bytes(tag.0, self.len()) }
    }
}
