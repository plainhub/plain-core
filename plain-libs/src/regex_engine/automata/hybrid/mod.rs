pub use self::{
    error::{BuildError, CacheError, StartError},
    id::LazyStateID,
};
pub mod dfa;
mod error;
mod id;
pub mod regex;
mod search;
