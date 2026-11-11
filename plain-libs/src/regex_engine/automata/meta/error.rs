use crate::regex_engine::automata::{PatternID, nfa, util::search::MatchError};
use crate::regex_engine::syntax::{ast, hir};
#[derive(Clone, Debug)]
pub struct BuildError {
    kind: BuildErrorKind,
}
#[derive(Clone, Debug)]
enum BuildErrorKind {
    Syntax {
        pid: PatternID,
        err: crate::regex_engine::syntax::Error,
    },
    NFA(nfa::thompson::BuildError),
}
impl BuildError {
    pub fn pattern(&self) -> Option<PatternID> {
        match self.kind {
            BuildErrorKind::Syntax { pid, .. } => Some(pid),
            _ => None,
        }
    }
    pub fn size_limit(&self) -> Option<usize> {
        match self.kind {
            BuildErrorKind::NFA(ref err) => err.size_limit(),
            _ => None,
        }
    }

    pub(crate) fn ast(pid: PatternID, err: ast::Error) -> BuildError {
        let err = crate::regex_engine::syntax::Error::from(err);
        BuildError {
            kind: BuildErrorKind::Syntax { pid, err },
        }
    }
    pub(crate) fn hir(pid: PatternID, err: hir::Error) -> BuildError {
        let err = crate::regex_engine::syntax::Error::from(err);
        BuildError {
            kind: BuildErrorKind::Syntax { pid, err },
        }
    }
    pub(crate) fn nfa(err: nfa::thompson::BuildError) -> BuildError {
        BuildError {
            kind: BuildErrorKind::NFA(err),
        }
    }
}
impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind {
            BuildErrorKind::Syntax { ref err, .. } => Some(err),
            BuildErrorKind::NFA(ref err) => Some(err),
        }
    }
}
impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.kind {
            BuildErrorKind::Syntax { pid, .. } => {
                write!(f, "error parsing pattern {}", pid.as_usize())
            }
            BuildErrorKind::NFA(_) => write!(f, "error building NFA"),
        }
    }
}
#[derive(Debug)]
pub(crate) enum RetryError {
    Quadratic(RetryQuadraticError),
    Fail(RetryFailError),
}
impl std::error::Error for RetryError {}
impl core::fmt::Display for RetryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            RetryError::Quadratic(ref err) => err.fmt(f),
            RetryError::Fail(ref err) => err.fmt(f),
        }
    }
}
impl From<MatchError> for RetryError {
    fn from(merr: MatchError) -> RetryError {
        RetryError::Fail(RetryFailError::from(merr))
    }
}
#[derive(Debug)]
pub(crate) struct RetryQuadraticError(());
impl RetryQuadraticError {
    pub(crate) fn new() -> RetryQuadraticError {
        RetryQuadraticError(())
    }
}
impl std::error::Error for RetryQuadraticError {}
impl core::fmt::Display for RetryQuadraticError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "regex engine gave up to avoid quadratic behavior")
    }
}
impl From<RetryQuadraticError> for RetryError {
    fn from(err: RetryQuadraticError) -> RetryError {
        RetryError::Quadratic(err)
    }
}
#[derive(Debug)]
pub(crate) struct RetryFailError {
    offset: usize,
}
impl RetryFailError {
    pub(crate) fn from_offset(offset: usize) -> RetryFailError {
        RetryFailError { offset }
    }
}
impl std::error::Error for RetryFailError {}
impl core::fmt::Display for RetryFailError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "regex engine failed at offset {:?}", self.offset)
    }
}
impl From<RetryFailError> for RetryError {
    fn from(err: RetryFailError) -> RetryError {
        RetryError::Fail(err)
    }
}
impl From<MatchError> for RetryFailError {
    fn from(merr: MatchError) -> RetryFailError {
        use crate::regex_engine::automata::util::search::MatchErrorKind::*;
        match *merr.kind() {
            Quit { offset, .. } => RetryFailError::from_offset(offset),
            GaveUp { offset } => RetryFailError::from_offset(offset),
            HaystackTooLong { .. } | UnsupportedAnchored { .. } => {
                unreachable!("found impossible error in meta engine: {merr}")
            }
        }
    }
}
