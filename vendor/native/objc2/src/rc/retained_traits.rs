//! Helper traits for Retained.

use super::Retained;

/// Helper trait to implement [`Default`] on [`Retained`].
#[doc(alias = "DefaultId")] // Previous name
pub trait DefaultRetained {
    /// The default [`Retained`] for a type.
    ///
    /// On most objects the implementation would be sending a message to the
    /// `new` selector.
    fn default_retained() -> Retained<Self>;
}

impl<T: ?Sized + DefaultRetained> Default for Retained<T> {
    #[inline]
    fn default() -> Self {
        T::default_retained()
    }
}

/// Helper trait to implement [`IntoIterator`] on [`Retained`].
///
/// This should be implemented in exactly the same fashion as if you were
/// implementing `IntoIterator` for your type normally.
//
// Note that [`Box<T>` gets to cheat with regards moves][box-move], so
// `boxed.into_iter()` is possible, while `obj.into_iter()` is not possible
// without this helper trait.
//
// [box-move]: https://doc.rust-lang.org/reference/expressions.html#moved-and-copied-types
#[doc(alias = "IdIntoIterator")] // Previous name
pub trait RetainedIntoIterator {
    /// The type of the elements being iterated over.
    type Item;

    /// Which kind of iterator are we turning this into?
    type IntoIter: Iterator<Item = Self::Item>;

    /// Creates an iterator from an [`Retained`].
    ///
    /// You would normally not call this function directly; instead, you'd
    /// call [`into_iter`](IntoIterator::into_iter) on an [`Retained`].
    fn retained_into_iter(this: Retained<Self>) -> Self::IntoIter;
}

// Note: These `IntoIterator` implementations conflict with an `Iterator`
// implementation for `Retained`.
//
// For our case however (in contrast with `Box`), that is the better tradeoff,
// which I will show with an example:
//
// ```
// let xs = Box::new(vec![]);
// for x in &xs { // Doesn't compile, `&Box` doesn't implement `IntoIterator`
//     // ...
// }
// ```
//
// Here, you're expected to write `xs.iter()` or `&**xs` instead, which is
// fairly acceptable, since usually people don't wrap things in boxes so much;
// but in Objective-C, _everything_ is wrapped in an `Retained`, and hence we should
// attempt to make that common case easier:
//
// ```
// let obj = NSArray::new(); // `Retained<NSArray<_>>`
// for item in &obj { // Should compile
//     // ...
// }
// ```
//
// The loss of the `Iterator` impl is a bit unfortunate, but not a big deal,
// since there is only one iterator in Objective-C anyhow, `NSEnumerator`, and
// for that we can make other abstractions instead.
impl<T: ?Sized + RetainedIntoIterator> IntoIterator for Retained<T> {
    type Item = <T as RetainedIntoIterator>::Item;
    type IntoIter = <T as RetainedIntoIterator>::IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        T::retained_into_iter(self)
    }
}

impl<'a, T: ?Sized> IntoIterator for &'a Retained<T>
where
    &'a T: IntoIterator,
{
    type Item = <&'a T as IntoIterator>::Item;
    type IntoIter = <&'a T as IntoIterator>::IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        (&**self).into_iter()
    }
}

/// Helper trait to implement [`FromIterator`] on [`Retained`].
///
/// This should be implemented in exactly the same fashion as if you were
/// implementing `FromIterator` for your type normally.
#[doc(alias = "IdFromIterator")] // Previous name
pub trait RetainedFromIterator<T>: Sized {
    /// Creates an `Retained` from an iterator.
    fn retained_from_iter<I>(iter: I) -> Retained<Self>
    where
        I: IntoIterator<Item = T>;
}

impl<T, U: RetainedFromIterator<T>> FromIterator<T> for Retained<U> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        U::retained_from_iter(iter)
    }
}

#[cfg(test)]
#[path = "../../../../../tests/native/objc2/unit/rc/retained_traits.rs"]
mod tests;
