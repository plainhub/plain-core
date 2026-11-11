use alloc::boxed::Box;
type Mask = u16;
#[derive(Debug)]
pub struct Finder {
    masks: Box<[Mask; 256]>,
    needle_len: usize,
}
impl Finder {
    const MAX_NEEDLE_LEN: usize = (Mask::BITS - 1) as usize;
    #[inline]
    pub fn new(needle: &[u8]) -> Option<Finder> {
        let needle_len = needle.len();
        if needle_len > Finder::MAX_NEEDLE_LEN {
            return None;
        }
        let mut searcher = Finder {
            masks: Box::from([!0; 256]),
            needle_len,
        };
        for (i, &byte) in needle.iter().enumerate() {
            searcher.masks[usize::from(byte)] &= !(1 << i);
        }
        Some(searcher)
    }
    #[inline]
    pub fn find(&self, haystack: &[u8]) -> Option<usize> {
        if self.needle_len == 0 {
            return Some(0);
        }
        let mut result = !1;
        for (i, &byte) in haystack.iter().enumerate() {
            result |= self.masks[usize::from(byte)];
            result <<= 1;
            if result & (1 << self.needle_len) == 0 {
                return Some(i + 1 - self.needle_len);
            }
        }
        None
    }
}
