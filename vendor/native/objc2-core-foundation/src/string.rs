use core::cmp::Ordering;
use core::ffi::c_char;
use core::fmt::Write;
use core::ptr::NonNull;
use core::{fmt, slice, str};

use crate::{
    kCFAllocatorNull, CFIndex, CFRange, CFRetained, CFString, CFStringBuiltInEncodings,
    CFStringCompareFlags,
};

#[track_caller]
unsafe fn debug_checked_utf8_unchecked(bytes: &[u8]) -> &str {
    if cfg!(debug_assertions) {
        match str::from_utf8(bytes) {
            Ok(s) => s,
            Err(err) => panic!(
                "unsafe precondition violated: CF function did not return valid UTF-8: {err}"
            ),
        }
    } else {
        // SAFETY: Checked by caller
        unsafe { str::from_utf8_unchecked(bytes) }
    }
}

impl CFString {
    /// Creates a new `CFString` from a [`str`][prim@str].
    #[inline]
    #[doc(alias = "CFStringCreateWithBytes")]
    #[allow(clippy::should_implement_trait)] // Not really sure of a better name
    pub fn from_str(string: &str) -> CFRetained<Self> {
        // Can never happen, allocations in Rust cannot be this large.
        debug_assert!(string.len() < CFIndex::MAX as usize);
        let len = string.len() as CFIndex;
        let s = unsafe {
            Self::with_bytes(
                None,
                string.as_ptr(),
                len,
                CFStringBuiltInEncodings::EncodingUTF8.0,
                false,
            )
        };
        // Should only fail if the string is not UTF-8 (which we know it is)
        // or perhaps on allocation error.
        s.expect("failed creating CFString")
    }

    /// Alias for easier transition from the `core-foundation` crate.
    #[inline]
    #[deprecated = "renamed to CFString::from_str"]
    pub fn new(string: &str) -> CFRetained<Self> {
        Self::from_str(string)
    }

    /// Creates a new `CFString` from a `'static` [`str`][prim@str].
    ///
    /// This may be slightly more efficient than [`CFString::from_str`], as it
    /// may be able to re-use the existing buffer (since we know it won't be
    /// deallocated).
    #[inline]
    #[doc(alias = "CFStringCreateWithBytesNoCopy")]
    pub fn from_static_str(string: &'static str) -> CFRetained<Self> {
        debug_assert!(string.len() < CFIndex::MAX as usize);
        let len = string.len() as CFIndex;
        // SAFETY: The string is used as a backing store, and thus must
        // potentially live forever, since we don't know how long the returned
        // CFString will be alive for. This is ensured by the `'static`
        // requirement.
        let s = unsafe {
            Self::with_bytes_no_copy(
                None,
                string.as_ptr(),
                len,
                CFStringBuiltInEncodings::EncodingUTF8.0,
                false,
                kCFAllocatorNull,
            )
        };
        s.expect("failed creating CFString")
    }

    /// Get the [`str`](`prim@str`) representation of this string if it can be
    /// done efficiently.
    ///
    /// Returns [`None`] if the internal storage does not allow this to be
    /// done efficiently. Use `CFString::to_string` if performance is not an
    /// issue.
    ///
    /// # Safety
    ///
    /// The `CFString` must not be mutated for the lifetime of the returned
    /// string.
    ///
    /// Warning: This is very difficult to ensure in generic contexts, e.g. it
    /// cannot even be used inside `Debug::fmt`, since `Formatter` uses `dyn`
    /// internally, and could thus mutate the string inside there.
    #[doc(alias = "CFStringGetCStringPtr")]
    pub unsafe fn as_str_unchecked(&self) -> Option<&str> {
        // NOTE: The encoding is an 8-bit encoding.
        let bytes = self.c_string_ptr(CFStringBuiltInEncodings::EncodingASCII.0);
        NonNull::new(bytes as *mut c_char).map(|bytes| {
            // NOTE: The returned string may contain interior NUL bytes:
            // https://github.com/swiftlang/swift-corelibs-foundation/issues/5200
            //
            // So we have to check the length of the string too. We do that
            // using `CFStringGetLength`; Since `CFStringGetCStringPtr`
            // returned a pointer, and we picked the encoding to be ASCII
            // (which has 1 codepoint per byte), this means that the number of
            // codepoints is the same as the number of bytes in the string.
            //
            // This is also what Swift does:
            // https://github.com/swiftlang/swift-corelibs-foundation/commit/8422c1a5e63913613a93523b3b398cb982df6205
            let len = self.length() as usize;

            // SAFETY: The pointer is valid for as long as the CFString is not
            // mutated (which the caller ensures it isn't for the lifetime of
            // the reference), and the length is correct (see above).
            let bytes = unsafe { slice::from_raw_parts(bytes.as_ptr().cast(), len) };

            // SAFETY: `CFStringGetCStringPtr` is (very likely) implemented
            // correctly, and we picked the encoding to be ASCII (which is a
            // subset of UTF-8).
            unsafe { debug_checked_utf8_unchecked(bytes) }
        })
    }
}

impl fmt::Display for CFString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Copy UTF-8 bytes from the CFString to the formatter in a loop, to
        // avoid allocating.
        //
        // We have to do this instead of using `CFStringGetCStringPtr`, as
        // that will be invalidated if the string is mutated while in use, and
        // `fmt::Formatter` contains `dyn Write` which may very theoretically
        // do exactly that.

        // Somewhat reasonably sized stack buffer.
        // TODO: Do performance testing, and tweak this value.
        //
        // Should be at least 4 (as that's the minimum size of `char`).
        let mut buf = [0u8; 32];

        let mut location_utf16 = 0;

        loop {
            let len_utf16 = self.length();
            let mut read_utf8 = 0;
            let read_utf16 = unsafe {
                self.bytes(
                    CFRange {
                        location: location_utf16,
                        length: len_utf16 - location_utf16,
                    },
                    CFStringBuiltInEncodings::EncodingUTF8.0,
                    0, // No conversion character
                    false,
                    buf.as_mut_ptr(),
                    buf.len() as _,
                    &mut read_utf8,
                )
            };
            if read_utf16 <= 0 {
                if location_utf16 < len_utf16 {
                    // We're not done reading the entire string yet; emit
                    // replacement character, advance one character, and try again.
                    f.write_char(char::REPLACEMENT_CHARACTER)?;
                    location_utf16 += 1;
                    continue;
                }
                break;
            }
            location_utf16 += read_utf16;

            // SAFETY: `CFStringGetBytes` is (very likely) implemented
            // correctly, and won't return non-UTF8 strings.
            //
            // Even if a string contains an UTF-8 char on a boundary, it won't
            // split it up when returning UTF-8.
            let s = unsafe { debug_checked_utf8_unchecked(&buf[0..read_utf8 as usize]) };

            // NOTE: May unwind, and may invalidate the string contents.
            f.write_str(s)?;
        }

        Ok(())
    }
}

impl PartialOrd for CFString {
    #[inline]
    #[doc(alias = "CFStringCompare")]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CFString {
    #[inline]
    #[doc(alias = "CFStringCompare")]
    fn cmp(&self, other: &Self) -> Ordering {
        // Request standard lexiographical ordering.
        let flags = CFStringCompareFlags::empty();
        self.compare(Some(other), flags).into()
    }
}

#[cfg(test)]
#[path = "../../../../tests/native/objc2-core-foundation/unit/string.rs"]
mod tests;
