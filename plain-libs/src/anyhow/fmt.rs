use crate::anyhow::chain::Chain;
use crate::anyhow::error::ErrorImpl;
use crate::anyhow::ptr::Ref;
use core::fmt::{self, Debug, Write};
impl ErrorImpl {
    pub(crate) unsafe fn display(this: Ref<Self>, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", unsafe { Self::error(this) })?;
        if f.alternate() {
            let chain = unsafe { Self::chain(this) };
            for cause in chain.skip(1) {
                write!(f, ": {}", cause)?;
            }
        }
        Ok(())
    }
    pub(crate) unsafe fn debug(this: Ref<Self>, f: &mut fmt::Formatter) -> fmt::Result {
        let error = unsafe { Self::error(this) };
        if f.alternate() {
            return Debug::fmt(error, f);
        }
        write!(f, "{}", error)?;
        if let Some(cause) = error.source() {
            write!(f, "\n\nCaused by:")?;
            let multiple = cause.source().is_some();
            for (n, error) in Chain::new(cause).enumerate() {
                writeln!(f)?;
                let mut indented = Indented {
                    inner: f,
                    number: if multiple { Some(n) } else { None },
                    started: false,
                };
                write!(indented, "{}", error)?;
            }
        }
        {
            use std::backtrace::BacktraceStatus;
            use std::string::ToString;
            let backtrace = unsafe { Self::backtrace(this) };
            if let BacktraceStatus::Captured = backtrace.status() {
                let mut backtrace = backtrace.to_string();
                write!(f, "\n\n")?;
                if backtrace.starts_with("stack backtrace:") {
                    backtrace.replace_range(0..1, "S");
                } else {
                    writeln!(f, "Stack backtrace:")?;
                }
                backtrace.truncate(backtrace.trim_end().len());
                write!(f, "{}", backtrace)?;
            }
        }
        Ok(())
    }
}
struct Indented<'a, D> {
    inner: &'a mut D,
    number: Option<usize>,
    started: bool,
}
impl<T> Write for Indented<'_, T>
where
    T: Write,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for (i, line) in s.split('\n').enumerate() {
            if !self.started {
                self.started = true;
                match self.number {
                    Some(number) => write!(self.inner, "{: >5}: ", number)?,
                    None => self.inner.write_str("    ")?,
                }
            } else if i > 0 {
                self.inner.write_char('\n')?;
                if self.number.is_some() {
                    self.inner.write_str("       ")?;
                } else {
                    self.inner.write_str("    ")?;
                }
            }
            self.inner.write_str(line)?;
        }
        Ok(())
    }
}
