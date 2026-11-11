use crate::anyhow::Error;
use crate::anyhow::StdError;
use core::fmt::{Debug, Display};
use std::boxed::Box;
pub struct Adhoc;
pub trait AdhocKind: Sized {
    #[inline]
    fn anyhow_kind(&self) -> Adhoc {
        Adhoc
    }
}
impl<T> AdhocKind for &T where T: ?Sized + Display + Debug + Send + Sync + 'static {}
impl Adhoc {
    #[cold]
    pub fn new<M>(self, message: M) -> Error
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        Error::construct_from_adhoc(message, backtrace!())
    }
}
pub struct Trait;
pub trait TraitKind: Sized {
    #[inline]
    fn anyhow_kind(&self) -> Trait {
        Trait
    }
}
impl<E> TraitKind for E where E: Into<Error> {}
impl Trait {
    #[cold]
    pub fn new<E>(self, error: E) -> Error
    where
        E: Into<Error>,
    {
        error.into()
    }
}
pub struct Boxed;
pub trait BoxedKind: Sized {
    #[inline]
    fn anyhow_kind(&self) -> Boxed {
        Boxed
    }
}
impl BoxedKind for Box<dyn StdError + Send + Sync> {}
impl Boxed {
    #[cold]
    pub fn new(self, error: Box<dyn StdError + Send + Sync>) -> Error {
        let backtrace = backtrace_if_absent!(&*error);
        Error::construct_from_boxed(error, backtrace)
    }
}
