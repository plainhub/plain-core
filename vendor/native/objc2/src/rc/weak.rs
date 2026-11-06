use alloc::boxed::Box;
use core::cell::UnsafeCell;
use core::fmt;
use core::marker::PhantomData;
use core::ptr;
use std::panic::{RefUnwindSafe, UnwindSafe};

use super::Retained;
use crate::runtime::AnyObject;
use crate::{ffi, Message};

/// A weak pointer to an Objective-C reference counted object.
///
/// The object is allowed to be deallocated while the weak pointer is alive,
/// though the backing allocation for the object can only be released once all
/// weak pointers are gone.
///
/// Useful for breaking reference cycles and safely checking whether an
/// object has been deallocated.
///
///
/// # Comparison to `std` types
///
/// This is the Objective-C equivalent of [`sync::Weak`] from the standard
/// library, and hence is only usable on types where `Retained<T>` acts like
/// [`sync::Arc`], a.k.a. on non-mutable types.
///
/// [`sync::Weak`]: std::sync::Weak
/// [`sync::Arc`]: std::sync::Arc
#[repr(transparent)] // This is not a public guarantee
#[doc(alias = "WeakId")] // Previous name
pub struct Weak<T: ?Sized> {
    /// We give the runtime the address to this box, so that it can modify it
    /// even if the `Weak` is moved.
    ///
    /// Loading may modify the pointer through a shared reference, so we use
    /// an UnsafeCell to get a *mut without self being mutable.
    ///
    /// Remember that any thread may actually modify the inner value
    /// concurrently, but as long as we only use it through the `objc_XXXWeak`
    /// methods, all access is behind a lock.
    ///
    /// TODO: Verify the need for UnsafeCell?
    /// TODO: Investigate if we can avoid some allocations using `Pin`.
    /// TODO: Add derive(CoercePointee) once this doesn't Box internally.
    inner: Box<UnsafeCell<*mut AnyObject>>,
    /// Weak inherits variance, dropck and various marker traits from
    /// `Retained<T>`.
    item: PhantomData<Retained<T>>,
}

impl<T: Message> Weak<T> {
    /// Construct a new weak pointer that references the given object.
    #[doc(alias = "objc_initWeak")]
    #[inline]
    pub fn new(obj: &T) -> Self {
        // SAFETY: Pointer is valid since it came from a reference.
        unsafe { Self::new_inner(obj) }
    }

    /// Construct a new weak pointer that references the given [`Retained`].
    #[doc(alias = "objc_initWeak")]
    #[deprecated = "use `Weak::from_retained` instead"]
    #[inline]
    pub fn from_id(obj: &Retained<T>) -> Self {
        Self::from_retained(obj)
    }

    /// Construct a new weak pointer that references the given [`Retained`].
    #[doc(alias = "objc_initWeak")]
    #[inline]
    pub fn from_retained(obj: &Retained<T>) -> Self {
        // SAFETY: Pointer is valid since it came from `Retained`.
        unsafe { Self::new_inner(Retained::as_ptr(obj)) }
    }

    /// Raw constructor.
    ///
    ///
    /// # Safety
    ///
    /// The object must be valid or null.
    unsafe fn new_inner(obj: *const T) -> Self {
        let inner = Box::new(UnsafeCell::new(ptr::null_mut()));
        // SAFETY: `ptr` will never move, and the caller verifies `obj`
        let _ = unsafe { ffi::objc_initWeak(inner.get(), (obj as *mut T).cast()) };
        Self {
            inner,
            item: PhantomData,
        }
    }

    /// Load the object into an [`Retained`] if it still exists.
    ///
    /// Returns [`None`] if the object has been deallocated, or the `Weak`
    /// was created with [`Default::default`].
    #[doc(alias = "retain")]
    #[doc(alias = "objc_loadWeak")]
    #[doc(alias = "objc_loadWeakRetained")]
    #[inline]
    pub fn load(&self) -> Option<Retained<T>> {
        let ptr = self.inner.get();
        let obj = unsafe { ffi::objc_loadWeakRetained(ptr) }.cast();
        // SAFETY: The object has +1 retain count
        unsafe { Retained::from_raw(obj) }
    }

    // TODO: Add `autorelease(&self, pool) -> Option<&T>` using `objc_loadWeak`?
}

impl<T: ?Sized> Drop for Weak<T> {
    /// Destroys the weak pointer.
    #[doc(alias = "objc_destroyWeak")]
    #[inline]
    fn drop(&mut self) {
        unsafe { ffi::objc_destroyWeak(self.inner.get()) }
    }
}

// TODO: Add ?Sized
impl<T: Message> Clone for Weak<T> {
    /// Make a clone of the weak pointer that points to the same object.
    #[doc(alias = "objc_copyWeak")]
    fn clone(&self) -> Self {
        let ptr = Box::new(UnsafeCell::new(ptr::null_mut()));
        unsafe { ffi::objc_copyWeak(ptr.get(), self.inner.get()) };
        Self {
            inner: ptr,
            item: PhantomData,
        }
    }
}

// TODO: Add ?Sized
impl<T: Message> Default for Weak<T> {
    /// Constructs a new weak pointer that doesn't reference any object.
    ///
    /// Calling [`Self::load`] on the return value always gives [`None`].
    #[inline]
    fn default() -> Self {
        // SAFETY: The pointer is null
        unsafe { Self::new_inner(ptr::null()) }
    }
}

impl<T: ?Sized> fmt::Debug for Weak<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Note: We intentionally don't try to debug-print the value, since
        // that could lead to cycles. See:
        // https://github.com/rust-lang/rust/pull/90291
        write!(f, "(Weak)")
    }
}

// SAFETY: Same as `std::sync::Weak<T>`.
unsafe impl<T: ?Sized + Sync + Send> Sync for Weak<T> {}

// SAFETY: Same as `std::sync::Weak<T>`.
unsafe impl<T: ?Sized + Sync + Send> Send for Weak<T> {}

// Same as `std::sync::Weak<T>`.
impl<T: ?Sized> Unpin for Weak<T> {}

// Same as `std::sync::Weak<T>`.
impl<T: ?Sized + RefUnwindSafe> RefUnwindSafe for Weak<T> {}

// Same as `std::sync::Weak<T>`.
impl<T: ?Sized + RefUnwindSafe> UnwindSafe for Weak<T> {}

impl<T: Message> From<&T> for Weak<T> {
    #[inline]
    fn from(obj: &T) -> Self {
        Weak::new(obj)
    }
}

impl<T: Message> From<&Retained<T>> for Weak<T> {
    #[inline]
    fn from(obj: &Retained<T>) -> Self {
        Weak::from_retained(obj)
    }
}

impl<T: Message> From<Retained<T>> for Weak<T> {
    #[inline]
    fn from(obj: Retained<T>) -> Self {
        Weak::from_retained(&obj)
    }
}

#[cfg(test)]
#[path = "../../../../../tests/native/objc2/unit/rc/weak.rs"]
mod tests;
