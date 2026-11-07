use std::{iter, mem};

mod model;
mod serialization;
mod word;

type Pair = (u32, u32);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IoError: {0}")]
    Io(#[from] std::io::Error),

    #[error("JsonError: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Bad vocabulary json file")]
    BadVocabulary,

    #[error("Merges text file invalid at line {0}")]
    BadMerges(usize),

    #[error("Token `{0}` out of vocabulary")]
    MergeTokenOutOfVocabulary(String),

    #[error("Unk token `{0}` not found in the vocabulary")]
    UnkTokenOutOfVocabulary(String),

    #[error("Dropout should be between 0 and 1, inclusive")]
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
