mod error;
mod params;
mod router;
mod tree;
pub use error::{InsertError, MatchError};
pub use params::{Params, ParamsIter};
pub use router::{Match, Router};

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/matchit.rs"]
mod tests;
