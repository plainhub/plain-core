use crate::matchit::tree::{Node, denormalize_params};
use std::fmt;
#[non_exhaustive]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InsertError {
    Conflict { with: String },
    TooManyParams,
    UnnamedParam,
    InvalidCatchAll,
}
impl fmt::Display for InsertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict { with } => {
                write!(
                    f,
                    "insertion failed due to conflict with previously registered route: {}",
                    with
                )
            }
            Self::TooManyParams => write!(f, "only one parameter is allowed per path segment"),
            Self::UnnamedParam => write!(f, "parameters must be registered with a name"),
            Self::InvalidCatchAll => write!(
                f,
                "catch-all parameters are only allowed at the end of a route"
            ),
        }
    }
}
impl std::error::Error for InsertError {}
impl InsertError {
    pub(crate) fn conflict<T>(route: &[u8], prefix: &[u8], current: &Node<T>) -> Self {
        let mut route = route[..route.len() - prefix.len()].to_owned();
        if !route.ends_with(&current.prefix) {
            route.extend_from_slice(&current.prefix);
        }
        let mut last = current;
        while let Some(node) = last.children.first() {
            last = node;
        }
        let mut current = current.children.first();
        while let Some(node) = current {
            route.extend_from_slice(&node.prefix);
            current = node.children.first();
        }
        denormalize_params(&mut route, &last.param_remapping);
        InsertError::Conflict {
            with: String::from_utf8(route).unwrap(),
        }
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MatchError {
    MissingTrailingSlash,
    ExtraTrailingSlash,
    NotFound,
}
impl MatchError {
    pub(crate) fn unsure(full_path: &[u8]) -> Self {
        if full_path[full_path.len() - 1] == b'/' {
            MatchError::ExtraTrailingSlash
        } else {
            MatchError::MissingTrailingSlash
        }
    }
}
impl fmt::Display for MatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            MatchError::MissingTrailingSlash => "match error: expected trailing slash",
            MatchError::ExtraTrailingSlash => "match error: found extra trailing slash",
            MatchError::NotFound => "match error: route not found",
        };
        write!(f, "{}", msg)
    }
}
impl std::error::Error for MatchError {}
