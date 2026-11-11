#[macro_use]
mod backtrace;
mod chain;
mod context;
mod error;
mod fmt;
mod kind;
mod macros;
mod ptr;
mod wrapper;
use crate::anyhow::error::ErrorImpl;
use crate::anyhow::ptr::Own;
use core::fmt::Display;
use std::error::Error as StdError;
#[repr(transparent)]
pub struct Error {
    inner: Own<ErrorImpl>,
}
#[derive(Clone)]
pub struct Chain<'a> {
    state: crate::anyhow::chain::ChainState<'a>,
}
pub type Result<T, E = Error> = core::result::Result<T, E>;
pub trait Context<T, E>: context::private::Sealed {
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static;
    fn with_context<C, F>(self, f: F) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C;
}

pub mod __private {
    use crate::anyhow::Error;
    use core::fmt::Arguments;
    pub use core::result::Result::Err;
    pub use core::{concat, format_args, stringify};
    use std::fmt;
    pub use std::format;
    pub mod kind {
        pub use crate::anyhow::kind::BoxedKind;
        pub use crate::anyhow::kind::{AdhocKind, TraitKind};
    }
    #[inline]
    #[cold]
    pub fn format_err(args: Arguments) -> Error {
        if let Some(message) = args.as_str() {
            Error::msg(message)
        } else {
            Error::msg(fmt::format(args))
        }
    }
    #[inline]
    #[cold]
    #[must_use]
    pub fn must_use(error: Error) -> Error {
        error
    }
}

pub use crate::{
    plain_error___anyhow as __anyhow, plain_error_anyhow as anyhow, plain_error_bail as bail,
    plain_error_ensure as ensure,
};

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/anyhow.rs"]
mod tests;
