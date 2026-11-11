use crate::regex_engine::automata::util::{
    captures, look,
    primitives::{PatternID, StateID},
};
#[derive(Clone, Debug)]
pub struct BuildError {
    kind: BuildErrorKind,
}
#[derive(Clone, Debug)]
enum BuildErrorKind {
    Syntax(crate::regex_engine::syntax::Error),
    Captures(captures::GroupInfoError),
    Word(look::UnicodeWordBoundaryError),
    TooManyPatterns { given: usize, limit: usize },
    TooManyStates { given: usize, limit: usize },
    ExceededSizeLimit { limit: usize },
    InvalidCaptureIndex { index: u32 },
    UnsupportedCaptures,
}
impl BuildError {
    pub fn size_limit(&self) -> Option<usize> {
        match self.kind {
            BuildErrorKind::ExceededSizeLimit { limit } => Some(limit),
            _ => None,
        }
    }
    fn kind(&self) -> &BuildErrorKind {
        &self.kind
    }
    pub(crate) fn syntax(err: crate::regex_engine::syntax::Error) -> BuildError {
        BuildError {
            kind: BuildErrorKind::Syntax(err),
        }
    }
    pub(crate) fn captures(err: captures::GroupInfoError) -> BuildError {
        BuildError {
            kind: BuildErrorKind::Captures(err),
        }
    }
    pub(crate) fn word(err: look::UnicodeWordBoundaryError) -> BuildError {
        BuildError {
            kind: BuildErrorKind::Word(err),
        }
    }
    pub(crate) fn too_many_patterns(given: usize) -> BuildError {
        let limit = PatternID::LIMIT;
        BuildError {
            kind: BuildErrorKind::TooManyPatterns { given, limit },
        }
    }
    pub(crate) fn too_many_states(given: usize) -> BuildError {
        let limit = StateID::LIMIT;
        BuildError {
            kind: BuildErrorKind::TooManyStates { given, limit },
        }
    }
    pub(crate) fn exceeded_size_limit(limit: usize) -> BuildError {
        BuildError {
            kind: BuildErrorKind::ExceededSizeLimit { limit },
        }
    }
    pub(crate) fn invalid_capture_index(index: u32) -> BuildError {
        BuildError {
            kind: BuildErrorKind::InvalidCaptureIndex { index },
        }
    }
    pub(crate) fn unsupported_captures() -> BuildError {
        BuildError {
            kind: BuildErrorKind::UnsupportedCaptures,
        }
    }
}
impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind() {
            BuildErrorKind::Syntax(err) => Some(err),
            BuildErrorKind::Captures(err) => Some(err),
            _ => None,
        }
    }
}
impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.kind() {
            BuildErrorKind::Syntax(_) => write!(f, "error parsing regex"),
            BuildErrorKind::Captures(_) => {
                write!(f, "error with capture groups")
            }
            BuildErrorKind::Word(_) => {
                write!(f, "NFA contains Unicode word boundary")
            }
            BuildErrorKind::TooManyPatterns { given, limit } => write!(
                f,
                "attempted to compile {given} patterns, \
                 which exceeds the limit of {limit}",
            ),
            BuildErrorKind::TooManyStates { given, limit } => write!(
                f,
                "attempted to compile {given} NFA states, \
                 which exceeds the limit of {limit}",
            ),
            BuildErrorKind::ExceededSizeLimit { limit } => write!(
                f,
                "heap usage during NFA compilation exceeded limit of {limit}",
            ),
            BuildErrorKind::InvalidCaptureIndex { index } => write!(
                f,
                "capture group index {index} is invalid \
                 (too big or discontinuous)",
            ),
            BuildErrorKind::UnsupportedCaptures => write!(
                f,
                "currently captures must be disabled when compiling \
                 a reverse NFA",
            ),
        }
    }
}
