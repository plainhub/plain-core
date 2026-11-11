pub use crate::regex_engine::automata::dfa::{
    automaton::{Automaton, OverlappingState, StartError},
    start::StartKind,
};
const DEAD: crate::regex_engine::automata::util::primitives::StateID =
    crate::regex_engine::automata::util::primitives::StateID::ZERO;
pub(crate) mod accel;
mod automaton;
pub mod dense;
mod determinize;
mod minimize;
pub mod onepass;
pub mod regex;
mod remapper;
mod search;

mod special;
mod start;
