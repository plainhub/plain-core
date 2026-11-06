use crate::{dispatch_object_s, DispatchQueue, DispatchQueueAttr, DispatchRetained};
use core::{ffi::c_char, ptr::NonNull};

extern "C" {
    pub fn dispatch_retain(object: NonNull<dispatch_object_s>);
    pub fn dispatch_release(object: NonNull<dispatch_object_s>);
}

impl DispatchQueue {
    #[inline]
    pub(crate) unsafe fn __new(
        label: *const c_char,
        attr: Option<&DispatchQueueAttr>,
    ) -> DispatchRetained<DispatchQueue> {
        extern "C" {
            fn dispatch_queue_create(
                label: *const c_char,
                attr: Option<&DispatchQueueAttr>,
            ) -> Option<NonNull<DispatchQueue>>;
        }
        let ret = unsafe { dispatch_queue_create(label, attr) };
        let ret =
            ret.expect("function was marked as returning non-null, but actually returned NULL");
        unsafe { DispatchRetained::from_raw(ret) }
    }
}
