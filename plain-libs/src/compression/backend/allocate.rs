use core::{
    ffi::{c_uint, c_void},
    marker::PhantomData,
    mem,
    ptr::NonNull,
};
use std::alloc::Layout;
const ALIGN: u8 = 64;
const _: () = assert!(ALIGN.count_ones() == 1);
const _: () = assert!(ALIGN as usize % mem::size_of::<*mut c_void>() == 0);
unsafe extern "C" fn zalloc_rust(_opaque: *mut c_void, count: c_uint, size: c_uint) -> *mut c_void {
    {
        let size = count as usize * size as usize;
        if size == 0 {
            return core::ptr::null_mut();
        }
        let layout = Layout::from_size_align(size, ALIGN.into()).unwrap();
        let ptr = unsafe { std::alloc::alloc(layout) };
        ptr as *mut c_void
    }
}
unsafe extern "C" fn zfree_rust(opaque: *mut c_void, ptr: *mut c_void) {
    {
        if ptr.is_null() {
            return;
        }
        debug_assert!(!opaque.is_null());
        if opaque.is_null() {
            return;
        }
        let size = unsafe { *(opaque as *mut usize) };
        if size == 0 {
            return;
        }
        let layout = Layout::from_size_align(size, ALIGN.into());
        let layout = layout.unwrap();
        unsafe { std::alloc::dealloc(ptr.cast(), layout) };
    }
}
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Allocator<'a> {
    pub zalloc: crate::compression::backend::c_api::AllocFn,
    pub zfree: crate::compression::backend::c_api::FreeFn,
    pub opaque: crate::compression::backend::c_api::VoidPtr,
    pub _marker: PhantomData<&'a ()>,
}
unsafe impl Sync for Allocator<'static> {}
pub static RUST: Allocator<'static> = Allocator {
    zalloc: zalloc_rust,
    zfree: zfree_rust,
    opaque: core::ptr::null_mut(),
    _marker: PhantomData,
};
impl Allocator<'_> {
    pub fn allocate_slice_raw<T>(&self, len: usize) -> Option<NonNull<T>> {
        let layout = Layout::array::<T>(len).ok()?;
        assert!(layout.align() <= ALIGN.into());
        if layout.size() == 0 {
            return None;
        }
        let layout = Layout::from_size_align(layout.size(), ALIGN.into()).ok()?;
        NonNull::new(unsafe { std::alloc::alloc(layout) }.cast())
    }

    pub unsafe fn deallocate<T>(&self, ptr: *mut T, len: usize) {
        unsafe {
            if !ptr.is_null() {
                assert_ne!(len, 0, "invalid allocation length");
                let layout =
                    Layout::from_size_align(core::mem::size_of::<T>() * len, ALIGN.into()).unwrap();
                std::alloc::dealloc(ptr.cast(), layout);
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/compression/backend/allocate.rs"]
mod tests;
