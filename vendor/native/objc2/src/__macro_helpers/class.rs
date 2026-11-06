use crate::{AnyThread, ClassType, MainThreadOnly, ThreadKind};

/// Helper for ensuring that `ClassType::ThreadKind`, if specified, is set
/// correctly.
pub trait ValidThreadKind<Requested: ?Sized + ThreadKind>
where
    Self: ClassType<ThreadKind = Requested>,
    // Ensure the user did not attempt to declare or define a root class.
    Self::Super: ClassType,
{
    // Required to reference the trait.
    fn check() {}
}

/// Always allow setting `MainThreadOnly`.
impl<'a, Cls> ValidThreadKind<dyn MainThreadOnly + 'a> for Cls
where
    Self: ClassType<ThreadKind = dyn MainThreadOnly + 'a>,
    Self::Super: ClassType,
{
}

/// But restrict `AnyThread` to only if the superclass also sets it.
impl<'a, 'b, Cls> ValidThreadKind<dyn AnyThread + 'a> for Cls
where
    Self: ClassType<ThreadKind = dyn AnyThread + 'a>,
    Self::Super: ClassType<ThreadKind = dyn AnyThread + 'b>,
{
}

/// Check that `MainThreadOnly` types do not implement `Send` and `Sync`.
///
/// Check implemented using type inference:
/// let _ = <MyType as MainThreadOnlyDoesNotImplSendSync<_>>::check
pub trait MainThreadOnlyDoesNotImplSendSync<Inferred> {
    // Required to reference the trait.
    fn check() {}
}

// Type inference will find this blanket impl...
impl<Cls: ?Sized> MainThreadOnlyDoesNotImplSendSync<()> for Cls {}

// ... unless one of these impls also apply, then type inference fails.
struct ImplsSend;
impl<Cls: ?Sized + MainThreadOnly + Send> MainThreadOnlyDoesNotImplSendSync<ImplsSend> for Cls {}

struct ImplsSync;
impl<Cls: ?Sized + MainThreadOnly + Sync> MainThreadOnlyDoesNotImplSendSync<ImplsSync> for Cls {}

/// Check that class does not implement `Drop`.
///
/// This is not needed for soundness, it's just a nice footgun to avoid (since
/// it wouldn't ever get called).
///
/// Check implemented using type inference:
/// let _ = <MyType as DoesNotImplDrop<_>>::check
pub trait DoesNotImplDrop<Inferred> {
    // Required to reference the trait.
    fn check() {}
}

// Type inference will find this blanket impl...
impl<Cls: ?Sized> DoesNotImplDrop<()> for Cls {}

// ... unless this impl also applies, then type inference fails.
struct ImplsDrop;
#[allow(drop_bounds)] // We're intentionally using `Drop` as a bound.
impl<Cls: ?Sized + Drop> DoesNotImplDrop<ImplsDrop> for Cls {}

#[cfg(test)]
#[path = "../../../../../tests/native/objc2/unit/__macro_helpers/class.rs"]
mod tests;
