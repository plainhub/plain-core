use crate::DispatchRetained;
use alloc::ffi::CString;

dispatch_object!(
    pub struct DispatchQueue;
);
dispatch_object_not_data!(unsafe DispatchQueue);

impl DispatchQueue {
    #[inline]
    pub fn new(label: &str, queue_attribute: Option<&DispatchQueueAttr>) -> DispatchRetained<Self> {
        let label = CString::new(label).expect("Invalid label!");

        // SAFETY: The label is a valid C string.
        unsafe { Self::__new(label.as_ptr(), queue_attribute) }
    }
}

dispatch_object!(
    pub struct DispatchQueueAttr;
);
dispatch_object_not_data!(unsafe DispatchQueueAttr);

impl DispatchQueueAttr {
    pub const SERIAL: Option<&Self> = None;
}
