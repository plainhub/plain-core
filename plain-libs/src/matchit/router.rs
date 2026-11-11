use crate::matchit::tree::Node;
use crate::matchit::{InsertError, MatchError, Params};
#[derive(Clone)]

pub struct Router<T> {
    root: Node<T>,
}
impl<T> Default for Router<T> {
    fn default() -> Self {
        Self {
            root: Node::default(),
        }
    }
}
impl<T> Router<T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn insert(&mut self, route: impl Into<String>, value: T) -> Result<(), InsertError> {
        self.root.insert(route, value)
    }
    pub fn at<'m, 'p>(&'m self, path: &'p str) -> Result<Match<'m, 'p, &'m T>, MatchError> {
        match self.root.at(path.as_bytes()) {
            Ok((value, params)) => Ok(Match {
                value: unsafe { &*value.get() },
                params,
            }),
            Err(e) => Err(e),
        }
    }
}
#[derive(Debug)]
pub struct Match<'k, 'v, V> {
    pub value: V,
    pub params: Params<'k, 'v>,
}
