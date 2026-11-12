use std::{iter, mem};

mod model;
mod serialization;
mod word;

type Pair = (u32, u32);

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),

    JsonError(crate::serde_json::Error),

    BadVocabulary,

    BadMerges(usize),

    MergeTokenOutOfVocabulary(String),

    UnkTokenOutOfVocabulary(String),

    InvalidDropout,
}

pub(crate) trait WithFirstLastIterator: Iterator + Sized {
    fn with_first_and_last(self) -> FirstLastIterator<Self>;
}

impl<I> WithFirstLastIterator for I
where
    I: Iterator,
{
    fn with_first_and_last(self) -> FirstLastIterator<Self> {
        FirstLastIterator {
            first: true,
            iter: self.peekable(),
        }
    }
}

pub(crate) struct FirstLastIterator<I>
where
    I: Iterator,
{
    first: bool,
    iter: iter::Peekable<I>,
}

impl<I> Iterator for FirstLastIterator<I>
where
    I: Iterator,
{
    type Item = (bool, bool, I::Item);

    fn next(&mut self) -> Option<Self::Item> {
        let first = mem::replace(&mut self.first, false);
        self.iter
            .next()
            .map(|e| (first, self.iter.peek().is_none(), e))
    }
}

// Re-export
pub use model::*;
use word::*;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(inner) => write!(f, "IoError: {0}", inner),
            Self::JsonError(inner) => write!(f, "JsonError: {0}", inner),
            Self::BadVocabulary => f.write_str("Bad vocabulary json file"),
            Self::BadMerges(inner) => write!(f, "Merges text file invalid at line {0}", inner),
            Self::MergeTokenOutOfVocabulary(inner) => {
                write!(f, "Token `{0}` out of vocabulary", inner)
            }
            Self::UnkTokenOutOfVocabulary(inner) => {
                write!(f, "Unk token `{0}` not found in the vocabulary", inner)
            }
            Self::InvalidDropout => f.write_str("Dropout should be between 0 and 1, inclusive"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(inner) => Some(inner),
            Self::JsonError(inner) => Some(inner),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(inner: std::io::Error) -> Self {
        Self::Io(inner)
    }
}

impl From<crate::serde_json::Error> for Error {
    fn from(inner: crate::serde_json::Error) -> Self {
        Self::JsonError(inner)
    }
}
