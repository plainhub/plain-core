pub use crate::regex_engine::aho::packed::api::{Builder, Config, FindIter, MatchKind, Searcher};
mod api;
mod ext;
mod pattern;
mod rabinkarp;
mod teddy;
mod vector;
