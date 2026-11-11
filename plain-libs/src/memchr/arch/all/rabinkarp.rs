use crate::memchr::ext::Pointer;
#[derive(Clone, Debug)]
pub struct Finder {
    hash: Hash,
    hash_2pow: u32,
}
impl Finder {
    #[inline]
    pub fn new(needle: &[u8]) -> Finder {
        let mut s = Finder {
            hash: Hash::new(),
            hash_2pow: 1,
        };
        let first_byte = match needle.first() {
            None => return s,
            Some(&first_byte) => first_byte,
        };
        s.hash.add(first_byte);
        for b in needle.iter().copied().skip(1) {
            s.hash.add(b);
            s.hash_2pow = s.hash_2pow.wrapping_shl(1);
        }
        s
    }
    #[inline]
    pub fn find(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        unsafe {
            let hstart = haystack.as_ptr();
            let hend = hstart.add(haystack.len());
            let nstart = needle.as_ptr();
            let nend = nstart.add(needle.len());
            let found = self.find_raw(hstart, hend, nstart, nend)?;
            Some(found.distance(hstart))
        }
    }
    #[inline]
    pub unsafe fn find_raw(
        &self,
        hstart: *const u8,
        hend: *const u8,
        nstart: *const u8,
        nend: *const u8,
    ) -> Option<*const u8> {
        unsafe {
            let hlen = hend.distance(hstart);
            let nlen = nend.distance(nstart);
            if nlen > hlen {
                return None;
            }
            let mut cur = hstart;
            let end = hend.sub(nlen);
            let mut hash = Hash::forward(cur, cur.add(nlen));
            loop {
                if self.hash == hash && is_equal_raw(cur, nstart, nlen) {
                    return Some(cur);
                }
                if cur >= end {
                    return None;
                }
                hash.roll(self, cur.read(), cur.add(nlen).read());
                cur = cur.add(1);
            }
        }
    }
}
#[derive(Clone, Debug)]
pub struct FinderRev(Finder);
impl FinderRev {
    #[inline]
    pub fn new(needle: &[u8]) -> FinderRev {
        let mut s = FinderRev(Finder {
            hash: Hash::new(),
            hash_2pow: 1,
        });
        let last_byte = match needle.last() {
            None => return s,
            Some(&last_byte) => last_byte,
        };
        s.0.hash.add(last_byte);
        for b in needle.iter().rev().copied().skip(1) {
            s.0.hash.add(b);
            s.0.hash_2pow = s.0.hash_2pow.wrapping_shl(1);
        }
        s
    }
    #[inline]
    pub fn rfind(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        unsafe {
            let hstart = haystack.as_ptr();
            let hend = hstart.add(haystack.len());
            let nstart = needle.as_ptr();
            let nend = nstart.add(needle.len());
            let found = self.rfind_raw(hstart, hend, nstart, nend)?;
            Some(found.distance(hstart))
        }
    }
    #[inline]
    pub unsafe fn rfind_raw(
        &self,
        hstart: *const u8,
        hend: *const u8,
        nstart: *const u8,
        nend: *const u8,
    ) -> Option<*const u8> {
        unsafe {
            let hlen = hend.distance(hstart);
            let nlen = nend.distance(nstart);
            if nlen > hlen {
                return None;
            }
            let mut cur = hend.sub(nlen);
            let start = hstart;
            let mut hash = Hash::reverse(cur, cur.add(nlen));
            loop {
                if self.0.hash == hash && is_equal_raw(cur, nstart, nlen) {
                    return Some(cur);
                }
                if cur <= start {
                    return None;
                }
                cur = cur.sub(1);
                hash.roll(&self.0, cur.add(nlen).read(), cur.read());
            }
        }
    }
}
#[inline]
pub(crate) fn is_fast(haystack: &[u8], _needle: &[u8]) -> bool {
    haystack.len() < 16
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Hash(u32);
impl Hash {
    #[inline(always)]
    fn new() -> Hash {
        Hash(0)
    }
    #[inline(always)]
    unsafe fn forward(mut start: *const u8, end: *const u8) -> Hash {
        unsafe {
            let mut hash = Hash::new();
            while start < end {
                hash.add(start.read());
                start = start.add(1);
            }
            hash
        }
    }
    #[inline(always)]
    unsafe fn reverse(start: *const u8, mut end: *const u8) -> Hash {
        unsafe {
            let mut hash = Hash::new();
            while start < end {
                end = end.sub(1);
                hash.add(end.read());
            }
            hash
        }
    }
    #[inline(always)]
    fn roll(&mut self, finder: &Finder, old: u8, new: u8) {
        self.del(finder, old);
        self.add(new);
    }
    #[inline(always)]
    fn add(&mut self, byte: u8) {
        self.0 = self.0.wrapping_shl(1).wrapping_add(u32::from(byte));
    }
    #[inline(always)]
    fn del(&mut self, finder: &Finder, byte: u8) {
        let factor = finder.hash_2pow;
        self.0 = self.0.wrapping_sub(u32::from(byte).wrapping_mul(factor));
    }
}
#[cold]
#[inline(never)]
unsafe fn is_equal_raw(x: *const u8, y: *const u8, n: usize) -> bool {
    unsafe { crate::memchr::arch::all::is_equal_raw(x, y, n) }
}
