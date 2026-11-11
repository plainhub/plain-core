use crate::compression::backend::weak_slice::WeakSliceMut;
use crate::cpu_features::CpuFeatures;
use core::fmt;
use core::mem::MaybeUninit;
use core::ops::Range;
pub struct Writer<'a> {
    buf: WeakSliceMut<'a, MaybeUninit<u8>>,
    filled: usize,
}
impl<'a> Writer<'a> {
    #[inline]
    pub fn new(buf: &'a mut [u8]) -> Writer<'a> {
        unsafe { Self::new_uninit(buf.as_mut_ptr(), buf.len()) }
    }
    #[inline]
    pub unsafe fn new_uninit(ptr: *mut u8, len: usize) -> Writer<'a> {
        {
            let buf = unsafe { WeakSliceMut::from_raw_parts_mut(ptr as *mut MaybeUninit<u8>, len) };
            Writer { buf, filled: 0 }
        }
    }

    #[inline]
    pub fn next_out(&mut self) -> *mut MaybeUninit<u8> {
        self.buf.as_mut_ptr().wrapping_add(self.filled).cast()
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buf.len()
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.filled
    }
    #[inline]
    pub fn filled(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.buf.as_ptr().cast(), self.filled) }
    }
    #[inline]
    pub fn remaining(&self) -> usize {
        self.capacity() - self.filled
    }
    #[inline]
    pub fn is_full(&self) -> bool {
        self.filled == self.buf.len()
    }
    pub fn push(&mut self, byte: u8) {
        self.buf.as_mut_slice()[self.filled] = MaybeUninit::new(byte);
        self.filled += 1;
    }
    #[inline(always)]
    pub fn extend(&mut self, buf: &[u8]) {
        self.buf.as_mut_slice()[self.filled..][..buf.len()].copy_from_slice(slice_to_uninit(buf));
        self.filled += buf.len();
    }
    #[inline(always)]
    pub fn extend_from_window(&mut self, window: &super::window::Window, range: Range<usize>) {
        self.extend_from_window_with_features::<{ CpuFeatures::NONE }>(window, range)
    }
    pub fn extend_from_window_with_features<const FEATURES: usize>(
        &mut self,
        window: &super::window::Window,
        range: Range<usize>,
    ) {
        match FEATURES {
            #[cfg(target_arch = "x86_64")]
            CpuFeatures::AVX2 => self.extend_from_window_help::<32>(window, range),
            _ => self.extend_from_window_runtime_dispatch(window, range),
        }
    }
    fn extend_from_window_runtime_dispatch(
        &mut self,
        window: &super::window::Window,
        range: Range<usize>,
    ) {
        #[cfg(target_arch = "x86_64")]
        if crate::cpu_features::is_enabled_avx2_and_bmi2() {
            return self.extend_from_window_help::<32>(window, range);
        }
        #[cfg(target_arch = "x86_64")]
        if crate::cpu_features::is_enabled_sse() {
            return self.extend_from_window_help::<16>(window, range);
        }
        #[cfg(target_arch = "aarch64")]
        if crate::cpu_features::is_enabled_neon() {
            return self.extend_from_window_help::<16>(window, range);
        }
        #[cfg(target_arch = "wasm32")]
        if crate::cpu_features::is_enabled_simd128() {
            return self.extend_from_window_help::<16>(window, range);
        }
        self.extend_from_window_help::<8>(window, range)
    }
    #[inline(always)]
    fn extend_from_window_help<const N: usize>(
        &mut self,
        window: &super::window::Window,
        range: Range<usize>,
    ) {
        let len = range.end - range.start;
        if self.remaining() >= len + N {
            unsafe {
                let src = window.as_ptr();
                Self::copy_chunk_unchecked::<N>(
                    src.wrapping_add(range.start).cast(),
                    self.next_out(),
                    len,
                )
            }
        } else {
            let buf = &window.as_slice()[range];
            self.buf.as_mut_slice()[self.filled..][..buf.len()]
                .copy_from_slice(slice_to_uninit(buf));
        }
        self.filled += len;
    }

    #[inline(always)]
    pub fn copy_match(&mut self, offset_from_end: usize, length: usize) {
        self.copy_match_with_features::<{ CpuFeatures::NONE }>(offset_from_end, length)
    }
    #[inline(always)]
    pub fn copy_match_with_features<const FEATURES: usize>(
        &mut self,
        offset_from_end: usize,
        length: usize,
    ) {
        match FEATURES {
            #[cfg(target_arch = "x86_64")]
            CpuFeatures::AVX2 => self.copy_match_help::<32>(offset_from_end, length),
            _ => self.copy_match_runtime_dispatch(offset_from_end, length),
        }
    }
    fn copy_match_runtime_dispatch(&mut self, offset_from_end: usize, length: usize) {
        #[cfg(target_arch = "x86_64")]
        if crate::cpu_features::is_enabled_avx2_and_bmi2() {
            return self.copy_match_help::<32>(offset_from_end, length);
        }
        #[cfg(target_arch = "x86_64")]
        if crate::cpu_features::is_enabled_sse() {
            return self.copy_match_help::<16>(offset_from_end, length);
        }
        #[cfg(target_arch = "aarch64")]
        if crate::cpu_features::is_enabled_neon() {
            return self.copy_match_help::<16>(offset_from_end, length);
        }
        #[cfg(target_arch = "wasm32")]
        if crate::cpu_features::is_enabled_simd128() {
            return self.copy_match_help::<16>(offset_from_end, length);
        }
        self.copy_match_help::<8>(offset_from_end, length)
    }
    #[inline(always)]
    fn copy_match_help<const N: usize>(&mut self, offset_from_end: usize, length: usize) {
        let capacity = self.buf.len();
        let len = Ord::min(self.filled + length + N, capacity);
        let buf = &mut self.buf.as_mut_slice()[..len];
        let current = self.filled;
        self.filled += length;
        if length > offset_from_end {
            match offset_from_end {
                1 => {
                    let element = buf[current - 1];
                    buf[current..][..length].fill(element);
                }
                _ => {
                    for i in 0..length {
                        buf[current + i] = buf[current - offset_from_end + i];
                    }
                }
            }
        } else {
            Self::copy_chunked_within::<N>(buf, capacity, current, offset_from_end, length);
        }
    }

    #[inline(always)]
    fn copy_chunked_within<const N: usize>(
        buf: &mut [MaybeUninit<u8>],
        capacity: usize,
        current: usize,
        offset_from_end: usize,
        length: usize,
    ) {
        let start = current.checked_sub(offset_from_end).expect("in bounds");
        if current + length + N < capacity {
            let ptr = buf.as_mut_ptr();
            unsafe { Self::copy_chunk_unchecked::<N>(ptr.add(start), ptr.add(current), length) }
        } else {
            buf.copy_within(start..start + length, current);
        }
    }
    #[inline(always)]
    unsafe fn copy_chunk_unchecked<const N: usize>(
        mut src: *const MaybeUninit<u8>,
        mut dst: *mut MaybeUninit<u8>,
        length: usize,
    ) {
        {
            if length == 0 {
                return;
            }
            let end = unsafe { src.add(length) };
            let chunk = unsafe { load_chunk::<N>(src) };
            unsafe { store_chunk::<N>(dst, chunk) };
            src = unsafe { src.add(N) };
            dst = unsafe { dst.add(N) };
            while src < end {
                let chunk = unsafe { load_chunk::<N>(src) };
                unsafe { store_chunk::<N>(dst, chunk) };
                src = unsafe { src.add(N) };
                dst = unsafe { dst.add(N) };
            }
        }
    }
}
#[inline(always)]
unsafe fn load_chunk<const N: usize>(from: *const MaybeUninit<u8>) -> [MaybeUninit<u8>; N] {
    { unsafe { core::ptr::read_unaligned(from.cast::<[MaybeUninit<u8>; N]>()) } }
}
#[inline(always)]
unsafe fn store_chunk<const N: usize>(out: *mut MaybeUninit<u8>, chunk: [MaybeUninit<u8>; N]) {
    { unsafe { core::ptr::write_unaligned(out.cast(), chunk) } }
}
impl fmt::Debug for Writer<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Writer")
            .field("ptr", &self.buf.as_ptr())
            .field("filled", &self.filled)
            .field("capacity", &self.capacity())
            .finish()
    }
}
fn slice_to_uninit(slice: &[u8]) -> &[MaybeUninit<u8>] {
    unsafe { &*(slice as *const [u8] as *const [MaybeUninit<u8>]) }
}
