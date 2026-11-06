use core::fmt;
use core::panic::{RefUnwindSafe, UnwindSafe};

#[cfg(any())]
use crate::encode::Encode;
use crate::encode::{Encoding, RefEncode};
use crate::ffi;

/// A type used to identify and manage memory zones.
///
/// Zones are ignored on all newer platforms, you should very rarely need to
/// use this, but may be useful if you need to implement `copyWithZone:` or
/// `allocWithZone:`.
///
/// See [Apple's documentation](https://developer.apple.com/documentation/foundation/nszone?language=objc).
#[repr(C)]
pub struct NSZone {
    _priv: [u8; 0],
    // Use `OpaqueData` to mark the types as !Send, !Sync and UnsafeCell.
    _inner: ffi::OpaqueData,
}

impl fmt::Debug for NSZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<NSZone {self:p}>")
    }
}

// Note: We don't know anything about the internals of `NSZone`, so best not
// to make it `Send` and `Sync` for now.

impl UnwindSafe for NSZone {}
impl RefUnwindSafe for NSZone {}

unsafe impl RefEncode for NSZone {
    #[cfg(not(any()))]
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("_NSZone", &[]));
    #[cfg(any())]
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct(
        "_NSZone",
        &[
            // Functions
            Encoding::Pointer(&Encoding::Unknown),
            Encoding::Pointer(&Encoding::Unknown),
            Encoding::Pointer(&Encoding::Unknown),
            Encoding::Pointer(&Encoding::Unknown),
            Encoding::Pointer(&Encoding::Unknown),
            Encoding::Pointer(&Encoding::Unknown),
            // Stats
            Encoding::Pointer(&Encoding::Unknown),
            // Zone granularity
            usize::ENCODING,
            // Name of zone
            Encoding::Object,
            // Next zone - note that the contents of this doesn't matter,
            // since this is nested far enough that the encoding string ends
            // up ignoring it.
            Encoding::Pointer(&Encoding::Struct("_NSZone", &[])),
        ],
    ));
}

#[cfg(test)]
#[path = "../../../../../tests/native/objc2/unit/runtime/nszone.rs"]
mod tests;
