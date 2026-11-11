use crate::regex_engine::automata::{hybrid::id::LazyStateIDError, nfa, util::search::Anchored};
#[derive(Clone, Debug)]
pub struct BuildError {
    kind: BuildErrorKind,
}
#[derive(Clone, Debug)]
enum BuildErrorKind {
    NFA(nfa::thompson::BuildError),
    InsufficientCacheCapacity { minimum: usize, given: usize },
    InsufficientStateIDCapacity { err: LazyStateIDError },
    Unsupported(&'static str),
}
impl BuildError {
    pub(crate) fn nfa(err: nfa::thompson::BuildError) -> BuildError {
        BuildError {
            kind: BuildErrorKind::NFA(err),
        }
    }
    pub(crate) fn insufficient_cache_capacity(minimum: usize, given: usize) -> BuildError {
        BuildError {
            kind: BuildErrorKind::InsufficientCacheCapacity { minimum, given },
        }
    }
    pub(crate) fn insufficient_state_id_capacity(err: LazyStateIDError) -> BuildError {
        BuildError {
            kind: BuildErrorKind::InsufficientStateIDCapacity { err },
        }
    }
    pub(crate) fn unsupported_dfa_word_boundary_unicode() -> BuildError {
        let msg = "cannot build lazy DFAs for regexes with Unicode word \
                   boundaries; switch to ASCII word boundaries, or \
                   heuristically enable Unicode word boundaries or use a \
                   different regex engine";
        BuildError {
            kind: BuildErrorKind::Unsupported(msg),
        }
    }
}
impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind {
            BuildErrorKind::NFA(ref err) => Some(err),
            _ => None,
        }
    }
}
impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.kind {
            BuildErrorKind::NFA(_) => write!(f, "error building NFA"),
            BuildErrorKind::InsufficientCacheCapacity { minimum, given } => {
                write!(
                    f,
                    "given cache capacity ({given}) is smaller than \
                     minimum required ({minimum})",
                )
            }
            BuildErrorKind::InsufficientStateIDCapacity { ref err } => err.fmt(f),
            BuildErrorKind::Unsupported(ref msg) => {
                write!(f, "unsupported regex feature for DFAs: {msg}")
            }
        }
    }
}
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum StartError {
    Cache { err: CacheError },
    Quit { byte: u8 },
    UnsupportedAnchored { mode: Anchored },
}
impl StartError {
    pub(crate) fn cache(err: CacheError) -> StartError {
        StartError::Cache { err }
    }
    pub(crate) fn quit(byte: u8) -> StartError {
        StartError::Quit { byte }
    }
    pub(crate) fn unsupported_anchored(mode: Anchored) -> StartError {
        StartError::UnsupportedAnchored { mode }
    }
}
impl std::error::Error for StartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            StartError::Cache { ref err } => Some(err),
            _ => None,
        }
    }
}
impl core::fmt::Display for StartError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            StartError::Cache { .. } => write!(
                f,
                "error computing start state because of cache inefficiency"
            ),
            StartError::Quit { byte } => write!(
                f,
                "error computing start state because the look-behind byte \
                 {:?} triggered a quit state",
                crate::regex_engine::automata::util::escape::DebugByte(byte),
            ),
            StartError::UnsupportedAnchored {
                mode: Anchored::Yes,
            } => {
                write!(
                    f,
                    "error computing start state because \
                     anchored searches are not supported or enabled"
                )
            }
            StartError::UnsupportedAnchored { mode: Anchored::No } => {
                write!(
                    f,
                    "error computing start state because \
                     unanchored searches are not supported or enabled"
                )
            }
            StartError::UnsupportedAnchored {
                mode: Anchored::Pattern(pid),
            } => {
                write!(
                    f,
                    "error computing start state because \
                     anchored searches for a specific pattern ({}) \
                     are not supported or enabled",
                    pid.as_usize(),
                )
            }
        }
    }
}
#[derive(Clone, Debug)]
pub struct CacheError(());
impl CacheError {
    pub(crate) fn too_many_cache_clears() -> CacheError {
        CacheError(())
    }
    pub(crate) fn bad_efficiency() -> CacheError {
        CacheError(())
    }
}
impl std::error::Error for CacheError {}
impl core::fmt::Display for CacheError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "lazy DFA cache has been cleared too many times")
    }
}
