//! Support for passing "out"-parameters to `msg_send!` and family.
//!
//! See clang's documentation:
//! <https://clang.llvm.org/docs/AutomaticReferenceCounting.html#passing-to-an-out-parameter-by-writeback>
//!
//! Note: We differ from that in that we do not create a temporary, whose
//! address we then work on; instead, we directly reuse the pointer that the
//! user provides (since, if it's a mutable pointer, we know that it's not
//! shared elsewhere in the program, and hence it is safe to modify directly).
//!
//! Another important consideration is unwinding; I haven't researched how
//! Clang handles that, but the correct thing is to do the writeback
//! retain/release dance regardless of whether the function unwinded or not.
//! We ensure this by doing it in `Drop`.
use core::mem::ManuallyDrop;
use core::ptr::NonNull;

use super::ConvertArgument;
use crate::rc::Retained;
use crate::Message;

// Note the `'static` bound here - this may not be necessary, but I'm unsure
// of the exact requirements, so we better keep it for now.
impl<T: Message + 'static> ConvertArgument for &mut Retained<T> {
    // We use `*mut T` as the inner value instead of `NonNull<T>`, since we
    // want to do debug checking that the value hasn't unexpectedly been
    // overwritten to contain NULL (which is clear UB, but the user might have
    // made a mistake).
    type __Inner = NonNull<*mut T>;

    type __WritebackOnDrop = WritebackOnDrop<T>;

    #[inline]
    fn __from_defined_param(_inner: Self::__Inner) -> Self {
        todo!("`&mut Retained<_>` is not supported in `define_class!` yet")
    }

    #[inline]
    unsafe fn __into_argument(self) -> (Self::__Inner, Self::__WritebackOnDrop) {
        let ptr: NonNull<Retained<T>> = NonNull::from(self);
        // `Retained` is `#[repr(transparent)]` over `NonNull`.
        let ptr: NonNull<NonNull<T>> = ptr.cast();

        // SAFETY: The value came from `&mut _`, and we only read a pointer.
        let old: NonNull<T> = unsafe { *ptr.as_ptr() };

        // `NonNull<T>` has the same layout as `*mut T`.
        let ptr: NonNull<*mut T> = ptr.cast();

        (ptr, WritebackOnDrop { ptr, old })
    }
}

#[derive(Debug)]
pub struct WritebackOnDrop<T: Message> {
    /// A copy of the argument, so that we can retain it after the message
    /// send.
    ///
    /// Ideally, we'd work with e.g. `&mut *mut T`, but we can't do that
    /// inside the generic context of `MessageArguments::__invoke`, while
    /// working within Rust's aliasing rules.
    ptr: NonNull<*mut T>,
    /// The old value, stored so that we can release if after the message
    /// send.
    old: NonNull<T>,
}

impl<T: Message> Drop for WritebackOnDrop<T> {
    #[inline]
    fn drop(&mut self) {
        // In terms of provenance, we roughly want to do the following:
        // ```
        // fn do(value: &mut Retained<T>) {
        //     let old = value.clone();
        //     msg_send![... value ...];
        //     let _ = value.clone();
        //     drop(old);
        // }
        // ```
        //
        // Which is definitely valid under stacked borrows! See also this
        // playground link for testing something equivalent in Miri:
        // <https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=5ad8fcff1f870819081aa534ec754b86>
        //
        //
        //
        // In Objective-C terms, we want to retain the new value and release
        // the old, and importantly, in that order (such that we don't dealloc
        // the value if it didn't change). So something like this:
        // ```
        // fn do(value: &mut Retained) {
        //     let old = *value;
        //     msg_send![... value ...];
        //     objc_retain(*value);
        //     objc_release(old);
        // }
        // ```

        // SAFETY: Caller ensures that the pointer is either left as-is, or is
        // safe to retain at this point.
        let new: Option<Retained<T>> = unsafe { Retained::retain(*self.ptr.as_ptr()) };
        // We ignore the result of `retain`, since it always returns the same
        // value as was given (and it would be unnecessary work to write that
        // value back into `ptr` again).
        let _new = ManuallyDrop::new(new);
        #[cfg(debug_assertions)]
        if _new.is_none() {
            panic!("found that NULL was written to `&mut Retained<_>`, which is UB! You should handle this with `&mut Option<Retained<_>>` instead");
        }

        // SAFETY: The old pointer was valid when it was constructed.
        //
        // If the message send modified the argument, they would have left a
        // +1 retain count on the old pointer; so either we have +1 from that,
        // or the message send didn't modify the pointer and we instead have
        // +1 retain count from the `retain` above.
        let _: Retained<T> = unsafe { Retained::new_nonnull(self.old) };
    }
}

impl<T: Message + 'static> ConvertArgument for &mut Option<Retained<T>> {
    type __Inner = NonNull<*mut T>;

    type __WritebackOnDrop = WritebackOnDropNullable<T>;

    #[inline]
    fn __from_defined_param(_inner: Self::__Inner) -> Self {
        todo!("`&mut Option<Retained<_>>` is not supported in `define_class!` yet")
    }

    #[inline]
    unsafe fn __into_argument(self) -> (Self::__Inner, Self::__WritebackOnDrop) {
        let ptr: NonNull<Option<Retained<T>>> = NonNull::from(self);
        // `Option<Retained<T>>` has the same memory layout as `*mut T`.
        let ptr: NonNull<*mut T> = ptr.cast();
        // SAFETY: Same as for `&mut Retained`
        let old: *mut T = unsafe { *ptr.as_ptr() };

        (ptr, WritebackOnDropNullable { ptr, old })
    }
}

/// Mostly the same as `WritebackOnDrop`, except that the old value is
/// nullable.
#[derive(Debug)]
pub struct WritebackOnDropNullable<T: Message> {
    ptr: NonNull<*mut T>,
    old: *mut T,
}

impl<T: Message> Drop for WritebackOnDropNullable<T> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: Same as for `&mut Retained`
        let new: Option<Retained<T>> = unsafe { Retained::retain(*self.ptr.as_ptr()) };
        let _ = ManuallyDrop::new(new);

        // SAFETY: Same as for `&mut Retained`
        //
        // Note: We explicitly keep the `if old == nil { objc_release(old) }`
        // check, since we expect that the user would often do:
        //
        // ```
        // let mut value = None
        // do(&mut value);
        // ```
        //
        // And in that case, we can elide the `objc_release`!
        let _: Option<Retained<T>> = unsafe { Retained::from_raw(self.old) };
    }
}

// Note: For `Option<&mut ...>` we explicitly want to do the `if Some` checks
// before anything else, since whether `None` or `Some` was passed is often
// known at compile-time, and for the `None` case it would be detrimental to
// have extra `retain/release` calls here.

impl<T: Message + 'static> ConvertArgument for Option<&mut Retained<T>> {
    type __Inner = Option<NonNull<*mut T>>;

    type __WritebackOnDrop = Option<WritebackOnDrop<T>>;

    #[inline]
    fn __from_defined_param(_inner: Self::__Inner) -> Self {
        todo!("`Option<&mut Retained<_>>` is not supported in `define_class!` yet")
    }

    #[inline]
    unsafe fn __into_argument(self) -> (Self::__Inner, Self::__WritebackOnDrop) {
        if let Some(this) = self {
            // SAFETY: Upheld by caller.
            let (ptr, helper) = unsafe { this.__into_argument() };
            (Some(ptr), Some(helper))
        } else {
            (None, None)
        }
    }
}

impl<T: Message + 'static> ConvertArgument for Option<&mut Option<Retained<T>>> {
    type __Inner = Option<NonNull<*mut T>>;

    type __WritebackOnDrop = Option<WritebackOnDropNullable<T>>;

    #[inline]
    fn __from_defined_param(_inner: Self::__Inner) -> Self {
        todo!("`Option<&mut Option<Retained<_>>>` is not supported in `define_class!` yet")
    }

    #[inline]
    unsafe fn __into_argument(self) -> (Self::__Inner, Self::__WritebackOnDrop) {
        if let Some(this) = self {
            // SAFETY: Upheld by caller.
            let (ptr, stored) = unsafe { this.__into_argument() };
            (Some(ptr), Some(stored))
        } else {
            (None, None)
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/native/objc2/unit/__macro_helpers/writeback.rs"]
mod tests;
