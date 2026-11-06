use crate::DispatchRetained;
use core::ptr::NonNull;

/// # Safety
/// Implementations must represent objects accepted by dispatch_retain and dispatch_release.
pub unsafe trait DispatchObject {
    #[inline]
    fn retain(&self) -> DispatchRetained<Self> {
        let ptr: NonNull<Self> = NonNull::from(self);
        // SAFETY:
        // - The pointer is valid since it came from `&self`.
        // - The lifetime of the pointer itself is extended, but any lifetime
        //   that the object may carry is still kept within the type itself.
        unsafe { DispatchRetained::retain(ptr) }
    }
    #[inline]
    fn as_raw(&self) -> NonNull<dispatch_object_s> {
        NonNull::from(self).cast()
    }
}

mod private {
    #[allow(non_camel_case_types)]
    #[repr(C)]
    #[derive(Debug)]
    pub struct dispatch_object_s {
        /// opaque value
        _inner: [u8; 0],
        _p: crate::OpaqueData,
    }

    #[cfg(feature = "objc2")]
    // SAFETY: Dispatch types are internally objects.
    unsafe impl objc2::encode::RefEncode for dispatch_object_s {
        const ENCODING_REF: objc2::encode::Encoding = objc2::encode::Encoding::Object;
    }
}

pub(crate) use private::dispatch_object_s;
