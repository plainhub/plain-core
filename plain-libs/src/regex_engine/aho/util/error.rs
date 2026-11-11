use crate::regex_engine::aho::util::{
    primitives::{PatternID, SmallIndex},
    search::MatchKind,
};
#[derive(Clone, Debug)]
pub struct BuildError {
    kind: ErrorKind,
}
#[derive(Clone, Debug)]
enum ErrorKind {
    StateIDOverflow { max: u64, requested_max: u64 },
    PatternIDOverflow { max: u64, requested_max: u64 },
    PatternTooLong { pattern: PatternID, len: usize },
}
impl BuildError {
    pub(crate) fn state_id_overflow(max: u64, requested_max: u64) -> BuildError {
        BuildError {
            kind: ErrorKind::StateIDOverflow { max, requested_max },
        }
    }
    pub(crate) fn pattern_id_overflow(max: u64, requested_max: u64) -> BuildError {
        BuildError {
            kind: ErrorKind::PatternIDOverflow { max, requested_max },
        }
    }
    pub(crate) fn pattern_too_long(pattern: PatternID, len: usize) -> BuildError {
        BuildError {
            kind: ErrorKind::PatternTooLong { pattern, len },
        }
    }
}
impl std::error::Error for BuildError {}
impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.kind {
            ErrorKind::StateIDOverflow { max, requested_max } => {
                write!(
                    f,
                    "state identifier overflow: failed to create state ID \
                     from {}, which exceeds the max of {}",
                    requested_max, max,
                )
            }
            ErrorKind::PatternIDOverflow { max, requested_max } => {
                write!(
                    f,
                    "pattern identifier overflow: failed to create pattern ID \
                     from {}, which exceeds the max of {}",
                    requested_max, max,
                )
            }
            ErrorKind::PatternTooLong { pattern, len } => {
                write!(
                    f,
                    "pattern {} with length {} exceeds \
                     the maximum pattern length of {}",
                    pattern.as_usize(),
                    len,
                    SmallIndex::MAX.as_usize(),
                )
            }
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchError(std::boxed::Box<MatchErrorKind>);
impl MatchError {
    pub fn new(kind: MatchErrorKind) -> MatchError {
        MatchError(std::boxed::Box::new(kind))
    }
    pub fn kind(&self) -> &MatchErrorKind {
        &self.0
    }
    pub fn invalid_input_anchored() -> MatchError {
        MatchError::new(MatchErrorKind::InvalidInputAnchored)
    }
    pub fn invalid_input_unanchored() -> MatchError {
        MatchError::new(MatchErrorKind::InvalidInputUnanchored)
    }
    pub fn unsupported_stream(got: MatchKind) -> MatchError {
        MatchError::new(MatchErrorKind::UnsupportedStream { got })
    }
    pub fn unsupported_overlapping(got: MatchKind) -> MatchError {
        MatchError::new(MatchErrorKind::UnsupportedOverlapping { got })
    }
    pub fn unsupported_empty() -> MatchError {
        MatchError::new(MatchErrorKind::UnsupportedEmpty)
    }
}
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatchErrorKind {
    InvalidInputAnchored,
    InvalidInputUnanchored,
    UnsupportedStream { got: MatchKind },
    UnsupportedOverlapping { got: MatchKind },
    UnsupportedEmpty,
}
impl std::error::Error for MatchError {}
impl core::fmt::Display for MatchError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match *self.kind() {
            MatchErrorKind::InvalidInputAnchored => {
                write!(f, "anchored searches are not supported or enabled")
            }
            MatchErrorKind::InvalidInputUnanchored => {
                write!(f, "unanchored searches are not supported or enabled")
            }
            MatchErrorKind::UnsupportedStream { got } => {
                write!(f, "match kind {:?} does not support stream searching", got,)
            }
            MatchErrorKind::UnsupportedOverlapping { got } => {
                write!(
                    f,
                    "match kind {:?} does not support overlapping searches",
                    got,
                )
            }
            MatchErrorKind::UnsupportedEmpty => {
                write!(
                    f,
                    "matching with an empty pattern string is not \
                     supported for this operation",
                )
            }
        }
    }
}
