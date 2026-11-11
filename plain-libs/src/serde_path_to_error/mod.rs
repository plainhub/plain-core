mod de;
mod path;
mod wrap;
pub use crate::serde_path_to_error::de::{Deserializer, deserialize};
pub use crate::serde_path_to_error::path::{Path, Segment, Segments};
use alloc::string::String;
use core::cell::Cell;
use core::fmt::{self, Display};
use serde::ser::StdError;
#[derive(Clone, Debug)]
pub struct Error<E> {
    path: Path,
    original: E,
}
impl<E> Error<E> {
    pub fn new(path: Path, inner: E) -> Self {
        Error {
            path,
            original: inner,
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn into_inner(self) -> E {
        self.original
    }
    pub fn inner(&self) -> &E {
        &self.original
    }
}
impl<E: Display> Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if !self.path.is_only_unknown() {
            write!(f, "{}: ", self.path)?;
        }
        write!(f, "{}", self.original)
    }
}
impl<E: StdError> StdError for Error<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.original.source()
    }
}
pub struct Track {
    path: Cell<Option<Path>>,
}
impl Track {
    pub const fn new() -> Self {
        Track {
            path: Cell::new(None),
        }
    }
    pub fn path(self) -> Path {
        self.path.into_inner().unwrap_or_else(Path::empty)
    }
    #[inline]
    fn trigger<E>(&self, chain: &Chain, err: E) -> E {
        self.trigger_impl(chain);
        err
    }
    fn trigger_impl(&self, chain: &Chain) {
        self.path.set(Some(match self.path.take() {
            Some(already_set) => already_set,
            None => Path::from_chain(chain),
        }));
    }
}
#[derive(Clone)]
enum Chain<'a> {
    Root,
    Seq {
        parent: &'a Chain<'a>,
        index: usize,
    },
    Map {
        parent: &'a Chain<'a>,
        key: String,
    },
    Struct {
        parent: &'a Chain<'a>,
        key: &'static str,
    },
    Enum {
        parent: &'a Chain<'a>,
        variant: String,
    },
    Some {
        parent: &'a Chain<'a>,
    },
    NewtypeStruct {
        parent: &'a Chain<'a>,
    },
    NewtypeVariant {
        parent: &'a Chain<'a>,
    },
    NonStringKey {
        parent: &'a Chain<'a>,
    },
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/serde_path_to_error.rs"]
mod tests;
