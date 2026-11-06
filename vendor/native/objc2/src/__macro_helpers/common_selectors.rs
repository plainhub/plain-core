//! Common selectors.
//!
//! These are put here to deduplicate the cached selector, and when using
//! `unstable-static-sel`, the statics.
//!
//! Note that our assembly tests of `unstable-static-sel-inlined` output a GOT
//! entry for such accesses, but that is just a limitation of our tests - the
//! actual assembly is as one would expect.
use crate::__sel_inner;
use crate::runtime::Sel;

#[inline]
pub fn alloc_sel() -> Sel {
    __sel_inner!("alloc", "alloc")
}

#[inline]
pub fn init_sel() -> Sel {
    __sel_inner!("init", "init")
}

#[inline]
pub fn new_sel() -> Sel {
    __sel_inner!("new", "new")
}

#[inline]
pub fn dealloc_sel() -> Sel {
    __sel_inner!("dealloc", "dealloc")
}

/// An undocumented selector called by the Objective-C runtime when
/// initializing instance variables.
#[inline]
#[allow(dead_code)] // May be useful in the future
fn cxx_construct_sel() -> Sel {
    __sel_inner!(".cxx_construct", ".cxx_construct")
}

/// Objective-C runtimes call `.cxx_destruct` as part of the final `dealloc`
/// call inside `NSObject` (and has done so since macOS 10.4).
///
/// While [GCC does document it somewhat][gcc-docs], this is still severely
/// undocumented in clang - but since the selector is emitted into the final
/// binary, it is fine to rely on it being used.
///
/// Unfortunately though, this only works if the class has been defined
/// statically, since in that case a flag is set to inform the runtime that it
/// needs to run destructors. So unfortunately we can't use this on
/// dynamically defined classes.
///
///
/// # ABI
///
/// The ABI of `.cxx_destruct` in Apple's runtime is actually that it does NOT
/// take a selector, unlike every other Objective-C method, see:
/// <https://github.com/apple-oss-distributions/objc4/blob/objc4-906/runtime/objc-class.mm#L457>
///
/// So the signature is `extern "C-unwind" fn(*mut AnyObject)`.
///
/// This is likely because it's not a real Objective-C method that can be
/// called from userspace / objc_msgSend, and it's more efficient to not pass
/// the selector.
///
/// Note that even if Apple decides to suddenly add the selector one day,
/// ignoring it will still be sound, since the function uses the C calling
/// convention, where such an ignored parameter would be allowed on all
/// relevant architectures.
///
/// [gcc-docs]: https://gcc.gnu.org/onlinedocs/gcc/Objective-C-and-Objective-C_002b_002b-Dialect-Options.html#index-fobjc-call-cxx-cdtors
#[inline]
#[allow(dead_code)] // May be useful in the future
fn cxx_destruct_sel() -> Sel {
    __sel_inner!(".cxx_destruct", ".cxx_destruct")
}

#[cfg(test)]
#[path = "../../../../../tests/native/objc2/unit/__macro_helpers/common_selectors.rs"]
mod tests;
