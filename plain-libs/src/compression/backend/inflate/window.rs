use crate::checksum::adler32::{adler32, adler32_fold_copy};
use crate::compression::backend::{crc32::Crc32Fold, weak_slice::WeakSliceMut};
#[derive(Debug)]
pub struct Window<'a> {
    buf: WeakSliceMut<'a, u8>,
    have: usize,
    next: usize,
}
impl<'a> Window<'a> {
    pub unsafe fn from_raw_parts(ptr: *mut u8, len: usize) -> Self {
        {
            Self {
                buf: unsafe { WeakSliceMut::from_raw_parts_mut(ptr, len) },
                have: 0,
                next: 0,
            }
        }
    }

    pub fn size(&self) -> usize {
        assert!(self.buf.is_empty() || self.buf.len() >= Self::padding());
        self.buf.len().saturating_sub(Self::padding())
    }
    pub fn have(&self) -> usize {
        self.have
    }

    pub fn next(&self) -> usize {
        self.next
    }
    pub fn empty() -> Self {
        Self {
            buf: WeakSliceMut::empty(),
            have: 0,
            next: 0,
        }
    }
    pub fn clear(&mut self) {
        self.have = 0;
        self.next = 0;
    }
    pub fn as_slice(&self) -> &[u8] {
        &self.buf.as_slice()[..self.have]
    }
    pub fn as_ptr(&self) -> *const u8 {
        self.buf.as_ptr()
    }

    pub(crate) fn extend(
        &mut self,
        slice: &[u8],
        flags: i32,
        update_checksum: bool,
        checksum: &mut u32,
        crc_fold: &mut Crc32Fold,
    ) {
        let len = slice.len();
        let wsize = self.size();
        if len >= wsize {
            let pos = len.saturating_sub(self.size());
            let (non_window_slice, window_slice) = slice.split_at(pos);
            if update_checksum {
                if flags != 0 {
                    crc_fold.fold(non_window_slice, 0);
                    crc_fold.fold_copy(&mut self.buf.as_mut_slice()[..wsize], window_slice);
                } else {
                    *checksum = adler32(*checksum, non_window_slice);
                    *checksum = adler32_fold_copy(*checksum, self.buf.as_mut_slice(), window_slice);
                }
            } else {
                self.buf.as_mut_slice()[..wsize].copy_from_slice(window_slice);
            }
            self.next = 0;
            self.have = self.size();
        } else {
            let dist = Ord::min(wsize - self.next, slice.len());
            let (end_part, start_part) = slice.split_at(dist);
            if update_checksum {
                let dst = &mut self.buf.as_mut_slice()[self.next..][..end_part.len()];
                if flags != 0 {
                    crc_fold.fold_copy(dst, end_part);
                } else {
                    *checksum = adler32_fold_copy(*checksum, dst, end_part);
                }
            } else {
                self.buf.as_mut_slice()[self.next..][..end_part.len()].copy_from_slice(end_part);
            }
            if !start_part.is_empty() {
                let dst = &mut self.buf.as_mut_slice()[..start_part.len()];
                if update_checksum {
                    if flags != 0 {
                        crc_fold.fold_copy(dst, start_part);
                    } else {
                        *checksum = adler32_fold_copy(*checksum, dst, start_part);
                    }
                } else {
                    dst.copy_from_slice(start_part);
                }
                self.next = start_part.len();
                self.have = self.size();
            } else {
                self.next += dist;
                if self.next == self.size() {
                    self.next = 0;
                }
                if self.have < self.size() {
                    self.have += dist;
                }
            }
        }
    }

    pub fn padding() -> usize {
        64
    }
}
