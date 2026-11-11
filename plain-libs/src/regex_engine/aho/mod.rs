pub use crate::regex_engine::aho::ahocorasick::StreamFindIter;
pub use crate::regex_engine::aho::{
    ahocorasick::{
        AhoCorasick, AhoCorasickBuilder, AhoCorasickKind, FindIter, FindOverlappingIter,
    },
    util::{
        error::{BuildError, MatchError, MatchErrorKind},
        primitives::{PatternID, PatternIDError},
        search::{Anchored, Input, Match, MatchKind, Span, StartKind},
    },
};
#[macro_use]
mod macros;
mod ahocorasick;
pub mod automaton;
pub mod dfa;
pub mod nfa;
pub mod packed;
pub(crate) mod util;
