#[cfg(not(any(
    target_pointer_width = "16",
    target_pointer_width = "32",
    target_pointer_width = "64"
)))]
compile_error!("not supported on non-{16,32,64}, please file an issue");
pub use crate::regex_engine::automata::util::primitives::PatternID;
pub use crate::regex_engine::automata::util::search::*;
pub mod dfa;
pub mod hybrid;
pub mod meta;
pub mod nfa;
pub mod util;
