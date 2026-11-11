use crate::anyhow::error::ContextError;
use crate::anyhow::{Context, Error, StdError};
use core::convert::Infallible;
use core::fmt::{self, Debug, Display, Write};
mod ext {
    use super::*;
    pub trait StdError {
        fn ext_context<C>(self, context: C) -> Error
        where
            C: Display + Send + Sync + 'static;
    }
    impl<E> StdError for E
    where
        E: crate::anyhow::StdError + Send + Sync + 'static,
    {
        fn ext_context<C>(self, context: C) -> Error
        where
            C: Display + Send + Sync + 'static,
        {
            let backtrace = backtrace_if_absent!(&self);
            Error::construct_from_context(context, self, backtrace)
        }
    }
    impl StdError for Error {
        fn ext_context<C>(self, context: C) -> Error
        where
            C: Display + Send + Sync + 'static,
        {
            self.context(context)
        }
    }
}
impl<T, E> Context<T, E> for Result<T, E>
where
    E: ext::StdError + Send + Sync + 'static,
{
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
    {
        match self {
            Ok(ok) => Ok(ok),
            Err(error) => Err(error.ext_context(context)),
        }
    }
    fn with_context<C, F>(self, context: F) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        match self {
            Ok(ok) => Ok(ok),
            Err(error) => Err(error.ext_context(context())),
        }
    }
}
impl<T> Context<T, Infallible> for Option<T> {
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
    {
        match self {
            Some(ok) => Ok(ok),
            None => Err(Error::construct_from_display(context, backtrace!())),
        }
    }
    fn with_context<C, F>(self, context: F) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        match self {
            Some(ok) => Ok(ok),
            None => Err(Error::construct_from_display(context(), backtrace!())),
        }
    }
}
impl<C, E> Debug for ContextError<C, E>
where
    C: Display,
    E: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Error")
            .field("context", &Quoted(&self.context))
            .field("source", &self.error)
            .finish()
    }
}
impl<C, E> Display for ContextError<C, E>
where
    C: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.context, f)
    }
}
impl<C, E> StdError for ContextError<C, E>
where
    C: Display,
    E: StdError + 'static,
{
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.error)
    }
}
impl<C> StdError for ContextError<C, Error>
where
    C: Display,
{
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(unsafe { crate::anyhow::ErrorImpl::error(self.error.inner.by_ref()) })
    }
}
struct Quoted<C>(C);
impl<C> Debug for Quoted<C>
where
    C: Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_char('"')?;
        Quoted(&mut *formatter).write_fmt(format_args!("{}", self.0))?;
        formatter.write_char('"')?;
        Ok(())
    }
}
impl Write for Quoted<&mut fmt::Formatter<'_>> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Display::fmt(&s.escape_debug(), self.0)
    }
}
pub(crate) mod private {
    use super::*;
    pub trait Sealed {}
    impl<T, E> Sealed for Result<T, E> where E: ext::StdError {}
    impl<T> Sealed for Option<T> {}
}
