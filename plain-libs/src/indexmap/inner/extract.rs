use super::{Bucket, Core};
use crate::indexmap::util::simplify_range;
use core::ops::RangeBounds;
impl<K, V> Core<K, V> {
    #[track_caller]
    pub(crate) fn extract<R>(&mut self, range: R) -> ExtractCore<'_, K, V>
    where
        R: RangeBounds<usize>,
    {
        let range = simplify_range(range, self.entries.len());
        assert_eq!(self.entries.len(), self.indices.len());
        unsafe {
            self.entries.set_len(range.start);
        }
        ExtractCore {
            map: self,
            new_len: range.start,
            current: range.start,
            end: range.end,
        }
    }
}
pub(crate) struct ExtractCore<'a, K, V> {
    map: &'a mut Core<K, V>,
    new_len: usize,
    current: usize,
    end: usize,
}
impl<K, V> Drop for ExtractCore<'_, K, V> {
    fn drop(&mut self) {
        let old_len = self.map.indices.len();
        let mut new_len = self.new_len;
        debug_assert!(new_len <= self.current);
        debug_assert!(self.current <= self.end);
        debug_assert!(self.current <= old_len);
        debug_assert!(old_len <= self.map.entries.capacity());
        unsafe {
            if new_len == self.current {
                new_len = old_len;
            } else if self.current < old_len {
                let tail_len = old_len - self.current;
                let base = self.map.entries.as_mut_ptr();
                let src = base.add(self.current);
                let dest = base.add(new_len);
                src.copy_to(dest, tail_len);
                new_len += tail_len;
            }
            self.map.entries.set_len(new_len);
        }
        if new_len != old_len {
            self.map.rebuild_hash_table();
        }
    }
}
impl<K, V> ExtractCore<'_, K, V> {
    pub(crate) fn extract_if<F>(&mut self, mut pred: F) -> Option<Bucket<K, V>>
    where
        F: FnMut(&mut Bucket<K, V>) -> bool,
    {
        debug_assert!(self.end <= self.map.entries.capacity());
        let base = self.map.entries.as_mut_ptr();
        while self.current < self.end {
            unsafe {
                let item = base.add(self.current);
                if pred(&mut *item) {
                    self.current += 1;
                    return Some(item.read());
                } else {
                    if self.new_len != self.current {
                        debug_assert!(self.new_len < self.current);
                        let dest = base.add(self.new_len);
                        item.copy_to_nonoverlapping(dest, 1);
                    }
                    self.current += 1;
                    self.new_len += 1;
                }
            }
        }
        None
    }
    pub(crate) fn remaining(&self) -> usize {
        self.end - self.current
    }
}
