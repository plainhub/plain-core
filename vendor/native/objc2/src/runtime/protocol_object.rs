use core::fmt;
use core::hash;
use core::marker::PhantomData;
use core::ptr::NonNull;

use crate::encode::{Encoding, RefEncode};
use crate::rc::{autoreleasepool_leaking, Retained};
use crate::runtime::__nsstring::nsstring_to_str;
use crate::runtime::{AnyObject, NSObjectProtocol};
use crate::Message;

/// An internal helper trait for [`ProtocolObject`].
///
///
/// # Safety
///
/// This is meant to be a sealed trait, and should not be implemented outside
/// of the [`extern_protocol!`] macro.
///
/// [`extern_protocol!`]: crate::extern_protocol
pub unsafe trait ImplementedBy<T: ?Sized + Message> {
    #[doc(hidden)]
    const __INNER: ();
}

/// An object representing any object that implements a specified protocol.
///
/// Objective-C has [a feature][protocol-type-checking] where you can write
/// `id<MyProtocol>`, and then work with the protocol as-if it was an object;
/// this is very similar to `dyn` traits in Rust!
///
/// If we could customize how `dyn Trait` works, then this struct would not
/// have been necessary; however, `dyn Trait` is a wide pointer with overhead,
/// which this struct helps avoid.
///
/// If the trait `T` inherits [`NSObjectProtocol`], this will implement common
/// traits like `Debug`, `PartialEq`, `Eq` and `Hash`.
///
/// [protocol-type-checking]: https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/ObjectiveC/Chapters/ocProtocols.html#//apple_ref/doc/uid/TP30001163-CH15-TPXREF151
///
///
/// # Example
///
/// Convert an object `MyObject` that implements the a protocol `MyProtocol`
/// into a [`ProtocolObject`] for working with the protocol in a type-erased
/// way.
///
/// ```
/// use objc2::runtime::ProtocolObject;
/// use objc2::rc::Retained;
/// # use objc2::runtime::NSObject as MyObject;
/// # use objc2::runtime::NSObjectProtocol as MyProtocol;
///
/// let obj: Retained<MyObject> = MyObject::new();
/// let proto: &ProtocolObject<dyn MyProtocol> = ProtocolObject::from_ref(&*obj);
/// let proto: Retained<ProtocolObject<dyn MyProtocol>> = ProtocolObject::from_retained(obj);
/// ```
#[doc(alias = "id")]
#[repr(C)]
pub struct ProtocolObject<P: ?Sized> {
    inner: AnyObject,
    p: PhantomData<P>,
}

// SAFETY: `Send` if the underlying trait promises `Send`.
//
// E.g. `ProtocolObject<dyn NSObjectProtocol + Send>` is naturally `Send`.
unsafe impl<P: ?Sized + Send> Send for ProtocolObject<P> {}

// SAFETY: `Sync` if the underlying trait promises `Sync`.
//
// E.g. `ProtocolObject<dyn NSObjectProtocol + Sync>` is naturally `Sync`.
unsafe impl<P: ?Sized + Sync> Sync for ProtocolObject<P> {}

// SAFETY: The type is `#[repr(C)]` and `AnyObject` internally
unsafe impl<P: ?Sized> RefEncode for ProtocolObject<P> {
    const ENCODING_REF: Encoding = Encoding::Object;
}

// SAFETY: The type is `AnyObject` internally, and is mean to be messaged
// as-if it's an object.
unsafe impl<P: ?Sized> Message for ProtocolObject<P> {}

impl<P: ?Sized> ProtocolObject<P> {
    /// Get an immutable type-erased reference from a type implementing a
    /// protocol.
    #[inline]
    pub fn from_ref<T: ?Sized + Message>(obj: &T) -> &Self
    where
        P: ImplementedBy<T>,
    {
        let ptr: NonNull<T> = NonNull::from(obj);
        let ptr: NonNull<Self> = ptr.cast();
        // SAFETY: Implementer ensures that the object conforms to the
        // protocol; so converting the reference here is safe.
        unsafe { ptr.as_ref() }
    }

    /// Get a type-erased object from a type implementing a protocol.
    #[deprecated = "use `ProtocolObject::from_retained` instead"]
    #[inline]
    pub fn from_id<T>(obj: Retained<T>) -> Retained<Self>
    where
        P: ImplementedBy<T> + 'static,
        T: Message + 'static,
    {
        Self::from_retained(obj)
    }

    /// Get a type-erased object from a type implementing a protocol.
    #[inline]
    pub fn from_retained<T>(obj: Retained<T>) -> Retained<Self>
    where
        P: ImplementedBy<T> + 'static,
        T: Message + 'static,
    {
        // SAFETY:
        // - The type can be represented as the casted-to type.
        // - Both types are `'static` (this could maybe be relaxed a bit, but
        //   let's be on the safe side)!
        unsafe { Retained::cast_unchecked::<Self>(obj) }
    }
}

impl<P: ?Sized + NSObjectProtocol> PartialEq for ProtocolObject<P> {
    #[inline]
    #[doc(alias = "isEqual:")]
    fn eq(&self, other: &Self) -> bool {
        self.isEqual(Some(&other.inner))
    }
}

impl<P: ?Sized + NSObjectProtocol> Eq for ProtocolObject<P> {}

impl<P: ?Sized + NSObjectProtocol> hash::Hash for ProtocolObject<P> {
    #[inline]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        <Self as NSObjectProtocol>::hash(self).hash(state);
    }
}

impl<P: ?Sized + NSObjectProtocol> fmt::Debug for ProtocolObject<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let description = self.description();

        // `NSString`s in return types, such as the one in `description`, are
        // in general _supposed_ to be immutable:
        // <https://developer.apple.com/library/archive/documentation/General/Conceptual/CocoaEncyclopedia/ObjectMutability/ObjectMutability.html#//apple_ref/doc/uid/TP40010810-CH5-SW65>
        //
        // In reality, that isn't actually the case for `NSMutableString`,
        // the `description` of that just returns itself instead of copying.
        //
        // Luckily though, the `UTF8String` returned by mutable objects do not
        // return a reference to internal data, and instead always allocate
        // a new autoreleased object (see details in `nsstring_to_str`), so
        // we do not have to worry about the string being mutated in e.g. a
        // malicious `Write` implementation in `fmt::Formatter` while we hold
        // the `&str`.

        // We use a leaking autorelease pool since often the string will be
        // UTF-8, and in that case the pool will be irrelevant. Also, it
        // allows us to pass the formatter into the pool (since it may contain
        // a pool internally that it assumes is current when writing).
        autoreleasepool_leaking(|pool| {
            // SAFETY:
            // - The `description` selector is guaranteed to always return an
            //   instance of `NSString`.
            // - We control the scope in which the string is alive, so we know
            //   it is not moved outside the current autorelease pool
            //   (`autoreleasepool_leaking` is greatly helping with this,
            //   though by itself does not fully ensure it).
            let s = unsafe { nsstring_to_str(&description, pool) };
            fmt::Display::fmt(s, f)
        })
    }
}

impl<P: ?Sized, T> AsRef<ProtocolObject<T>> for ProtocolObject<P>
where
    T: ?Sized + ImplementedBy<ProtocolObject<P>>,
{
    #[inline]
    fn as_ref(&self) -> &ProtocolObject<T> {
        ProtocolObject::from_ref(self)
    }
}

// TODO: Maybe implement Borrow?

impl<P: ?Sized + 'static> AsRef<AnyObject> for ProtocolObject<P> {
    #[inline]
    fn as_ref(&self) -> &AnyObject {
        let ptr: NonNull<ProtocolObject<P>> = NonNull::from(self);
        let ptr: NonNull<AnyObject> = ptr.cast();
        // SAFETY: All protocol objects are Objective-C objects too.
        unsafe { ptr.as_ref() }
    }
}

#[cfg(test)]
#[allow(clippy::missing_safety_doc)]
#[allow(dead_code)]
#[path = "../../../../../tests/native/objc2/unit/runtime/protocol_object.rs"]
mod tests;
