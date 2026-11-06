//! Optimized versions of `objc_retain` and `objc_release`.
//!
//! On macOS 13.0 / iOS 16.0 / tvOS 16.0 / watchOS 9.0, on ARM64, optimized
//! versions of these two functions that use a different calling convention
//! than the usual C calling convention, are available.
//!
//! Specifically, the expected input register is changed. The output register
//! is unchanged.
//!
//! As an example, if the object is stored in the `x19` register and we need
//! to release it, we usually end up emitting an extra `mov` to get the object
//! into the `x0` register first, as expected by the C calling convention:
//!
//! ```asm
//! mov x0, x19
//! bl _objc_release
//! ```
//!
//! With this optimization though, since the expected register is encoded in
//! the name of the function instead, we can avoid the move altogether.
//!
//! ```asm
//! bl _objc_release_x19
//! ```
//!
//!
//!
//! Safety of our two uses of the `asm!` macro:
//!
//! 1. We use the register class `reg`, with the modifier `x`, which on
//!    Aarch64 is defined as `x[0-30]`, see [this][asm-reg-cls].
//!
//!    The functions are only available in the variants `x[0-15]` and
//!    `x[19-28]` though, see [this][objc4-source], so if the register
//!    allocator ends up using `x16`, `x17`, `x18`, `x29` or `x30`, we will
//!    emit a call to e.g. `objc_retain_x29`, which will fail at link time.
//!
//!    Of thesee five registers, `x18` is a reserved register on Apple, so the
//!    register allocator won't use it. `x29` is the frame pointer, so that
//!    will also never be used (at least not for storing a pointer to an
//!    object).
//!
//!    Left are `x16`, `x17` and `x30`, which we prevent from being selected
//!    by specifying `out("x16") _`, `out("x17") _` and `out("x30") _`, which
//!    tricks the register allocator into believing that it cannot use those
//!    registers. This may be slightly worse than optimal, since it also
//!    clobbers the registers, but then again, so does `clobber_abi("C")`, so
//!    it probably won't matter.
//!
//!    (NOTE: I'm not _entirely_ sure that the register allocator won't select
//!    one of the clobbered registers, but luckily, it is not a safety
//!    requirement either way, it will "just" lead to a link error).
//!
//! 2. We use the `clobber_abi("C")` since we're effectively calling a C
//!    C function.
//!
//! [asm-reg-cls]: https://doc.rust-lang.org/nightly/reference/inline-assembly.html#register-operands
//! [objc4-source]: https://github.com/apple-oss-distributions/objc4/blob/objc4-866.9/runtime/objc-abi.h#L442-L498
use crate::runtime::AnyObject;

/// A potentially faster version of `ffi::objc_retain`.
///
///
/// # Safety
///
/// Same as `ffi::objc_retain`.
#[inline]
pub(crate) unsafe fn objc_retain_fast(obj: *mut AnyObject) -> *mut AnyObject {
    #[cfg(all(any(), target_arch = "aarch64"))]
    // SAFETY: See the file header.
    //
    // As per the ARM64 calling convention, the return value is put in `x0`.
    //
    // That the function itself is safe to call is upheld by the caller.
    //
    // TODO: Unwinding.
    unsafe {
        let result;
        core::arch::asm!(
            "bl _objc_retain_{obj:x}",
            obj = in(reg) obj,
            lateout("x0") result,
            out("x16") _,
            out("x17") _,
            out("x30") _,
            clobber_abi("C"),
        );
        result
    }

    #[cfg(not(all(any(), target_arch = "aarch64")))]
    // SAFETY: Upheld by caller.
    unsafe {
        crate::ffi::objc_retain(obj)
    }
}

/// A potentially faster version of `ffi::objc_release`.
///
///
/// # Safety
///
/// Same as `ffi::objc_release`.
#[inline]
pub(crate) unsafe fn objc_release_fast(obj: *mut AnyObject) {
    #[cfg(all(any(), target_arch = "aarch64"))]
    // SAFETY: See the file header.
    //
    // That the function itself is safe to call is upheld by the caller.
    //
    // TODO: Unwinding.
    unsafe {
        core::arch::asm!(
            "bl _objc_release_{obj:x}",
            obj = in(reg) obj,
            out("x16") _,
            out("x17") _,
            out("x30") _,
            clobber_abi("C"),
        )
    }

    #[cfg(not(all(any(), target_arch = "aarch64")))]
    // SAFETY: Upheld by caller.
    unsafe {
        crate::ffi::objc_release(obj)
    }
}

#[cfg(test)]
#[path = "../../../../../tests/native/objc2/unit/runtime/retain_release_fast.rs"]
mod tests;
