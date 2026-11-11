pub use self::{
    error::BuildError,
    regex::{Builder, Cache, CapturesMatches, Config, FindMatches, Regex, Split, SplitN},
};
mod error;
mod limited;
mod literal;
mod prefix;
mod regex;
mod reverse_inner;
mod reverse_suffix;
mod stopat;
mod strategy;
mod wrappers;
