#![forbid(unsafe_code)]
use crate::parser::types::OperationType;
use crate::value::Name;
pub use parse::parse_query;
use pest::{RuleType, error::LineColLocation};
pub use pos::{Pos, Positioned};
use serde::{Serialize, Serializer};
use std::fmt::{self, Display, Formatter};
mod parse;
mod pos;
pub mod types;
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    Syntax {
        message: String,
        start: Pos,
        end: Option<Pos>,
    },
    MultipleRoots {
        root: OperationType,
        schema: Pos,
        pos: Pos,
    },
    MissingQueryRoot {
        pos: Pos,
    },
    MultipleOperations {
        anonymous: Pos,
        operation: Pos,
    },
    OperationDuplicated {
        operation: Name,
        first: Pos,
        second: Pos,
    },
    FragmentDuplicated {
        fragment: Name,
        first: Pos,
        second: Pos,
    },
    MissingOperation,
    RecursionLimitExceeded,
}
impl Error {
    #[must_use]
    pub fn positions(&self) -> ErrorPositions {
        match self {
            Self::Syntax {
                start,
                end: Some(end),
                ..
            } => ErrorPositions::new_2(*start, *end),
            Self::Syntax { start, .. } => ErrorPositions::new_1(*start),
            Self::MultipleRoots { schema, pos, .. } => ErrorPositions::new_2(*pos, *schema),
            Self::MissingQueryRoot { pos } => ErrorPositions::new_1(*pos),
            Self::MultipleOperations {
                anonymous,
                operation,
            } => ErrorPositions::new_2(*anonymous, *operation),
            Self::OperationDuplicated { first, second, .. } => {
                ErrorPositions::new_2(*second, *first)
            }
            Self::FragmentDuplicated { first, second, .. } => {
                ErrorPositions::new_2(*second, *first)
            }
            Self::MissingOperation => ErrorPositions::new_0(),
            Self::RecursionLimitExceeded => ErrorPositions::new_0(),
        }
    }
}
impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { message, .. } => f.write_str(message),
            Self::MissingQueryRoot { .. } => f.write_str("schema definition is missing query root"),
            Self::MultipleRoots { root, .. } => {
                write!(f, "multiple {} roots in schema definition", root)
            }
            Self::MultipleOperations { .. } => f.write_str("document contains multiple operations"),
            Self::OperationDuplicated { operation, .. } => {
                write!(f, "operation {} is defined twice", operation)
            }
            Self::FragmentDuplicated { fragment, .. } => {
                write!(f, "fragment {} is defined twice", fragment)
            }
            Self::MissingOperation => f.write_str("document does not contain an operation"),
            Self::RecursionLimitExceeded => f.write_str("recursion limit exceeded."),
        }
    }
}
impl std::error::Error for Error {}
impl<R: RuleType> From<pest::error::Error<R>> for Error {
    fn from(err: pest::error::Error<R>) -> Self {
        let (start, end) = match err.line_col {
            LineColLocation::Pos(at) => (at, None),
            LineColLocation::Span(start, end) => (start, Some(end)),
        };
        Error::Syntax {
            message: err.to_string(),
            start: Pos::from(start),
            end: end.map(Pos::from),
        }
    }
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone)]
pub struct ErrorPositions(ErrorPositionsInner);
impl ErrorPositions {
    fn new_0() -> Self {
        Self(ErrorPositionsInner::None)
    }
    fn new_1(a: Pos) -> Self {
        Self(ErrorPositionsInner::One(a))
    }
    fn new_2(a: Pos, b: Pos) -> Self {
        Self(ErrorPositionsInner::Two(a, b))
    }
}
impl Iterator for ErrorPositions {
    type Item = Pos;
    fn next(&mut self) -> Option<Self::Item> {
        match self.0 {
            ErrorPositionsInner::Two(a, b) => {
                self.0 = ErrorPositionsInner::One(b);
                Some(a)
            }
            ErrorPositionsInner::One(a) => {
                self.0 = ErrorPositionsInner::None;
                Some(a)
            }
            ErrorPositionsInner::None => None,
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}
impl DoubleEndedIterator for ErrorPositions {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self.0 {
            ErrorPositionsInner::Two(a, b) => {
                self.0 = ErrorPositionsInner::One(a);
                Some(b)
            }
            ErrorPositionsInner::One(a) => {
                self.0 = ErrorPositionsInner::None;
                Some(a)
            }
            ErrorPositionsInner::None => None,
        }
    }
}
impl std::iter::FusedIterator for ErrorPositions {}
impl ExactSizeIterator for ErrorPositions {
    fn len(&self) -> usize {
        match self.0 {
            ErrorPositionsInner::Two(_, _) => 2,
            ErrorPositionsInner::One(_) => 1,
            ErrorPositionsInner::None => 0,
        }
    }
}
impl Serialize for ErrorPositions {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_seq(self.clone())
    }
}
#[derive(Debug, Clone, Copy)]
enum ErrorPositionsInner {
    Two(Pos, Pos),
    One(Pos),
    None,
}

#[cfg(test)]
#[path = "../../../../tests/plain-graphql/unit/parser/recursion_limit.rs"]
mod recursion_limit_tests;
