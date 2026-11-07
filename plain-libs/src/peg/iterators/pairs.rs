// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::iter::Filter;
use core::ptr;
use core::str;

#[cfg(any())]
use serde::ser::SerializeStruct;

use super::flat_pairs::{self, FlatPairs};
use super::line_index::LineIndex;
use super::pair::{self, Pair};
use super::queueable_token::QueueableToken;
use super::tokens::{self, Tokens};
use crate::peg::RuleType;

#[derive(Clone)]
pub struct Pairs<'i, R> {
    queue: Rc<Vec<QueueableToken<'i, R>>>,
    input: &'i str,
    start: usize,
    end: usize,
    pairs_count: usize,
    line_index: Rc<LineIndex>,
}

pub fn new<'i, R: RuleType>(
    queue: Rc<Vec<QueueableToken<'i, R>>>,
    input: &'i str,
    line_index: Option<Rc<LineIndex>>,
    start: usize,
    end: usize,
) -> Pairs<'i, R> {
    let line_index = match line_index {
        Some(line_index) => line_index,
        None => {
            let last_input_pos = queue
                .last()
                .map(|token| match *token {
                    QueueableToken::Start { input_pos, .. }
                    | QueueableToken::End { input_pos, .. } => input_pos,
                })
                .unwrap_or(0);

            Rc::new(LineIndex::new(&input[..last_input_pos]))
        }
    };

    let mut pairs_count = 0;
    let mut cursor = start;
    while cursor < end {
        cursor = match queue[cursor] {
            QueueableToken::Start {
                end_token_index, ..
            } => end_token_index,
            _ => unreachable!(),
        } + 1;
        pairs_count += 1;
    }

    Pairs {
        queue,
        input,
        start,
        end,
        pairs_count,
        line_index,
    }
}

impl<'i, R: RuleType> Pairs<'i, R> {
    #[inline]
    pub fn as_str(&self) -> &'i str {
        if self.start < self.end {
            let start = self.pos(self.start);
            let end = self.pos(self.end - 1);
            // Generated positions always come from Positions and are UTF-8 borders.
            &self.input[start..end]
        } else {
            ""
        }
    }

    pub fn get_input(&self) -> &'i str {
        self.input
    }

    #[inline]
    pub fn concat(&self) -> String {
        self.clone()
            .fold(String::new(), |string, pair| string + pair.as_str())
    }

    #[inline]
    pub fn flatten(self) -> FlatPairs<'i, R> {
        flat_pairs::new(
            self.queue,
            self.input,
            self.line_index,
            self.start,
            self.end,
        )
    }

    #[inline]
    pub fn find_first_tagged(&self, tag: &'i str) -> Option<Pair<'i, R>> {
        self.clone().find_tagged(tag).next()
    }

    #[inline]
    pub fn find_tagged(
        self,
        tag: &'i str,
    ) -> Filter<FlatPairs<'i, R>, impl FnMut(&Pair<'i, R>) -> bool + 'i> {
        self.flatten()
            .filter(move |pair: &Pair<'i, R>| matches!(pair.as_node_tag(), Some(nt) if nt == tag))
    }

    #[inline]
    pub fn tokens(self) -> Tokens<'i, R> {
        tokens::new(self.queue, self.input, self.start, self.end)
    }

    #[inline]
    pub fn peek(&self) -> Option<Pair<'i, R>> {
        if self.start < self.end {
            Some(pair::new(
                Rc::clone(&self.queue),
                self.input,
                Rc::clone(&self.line_index),
                self.start,
            ))
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pairs_count == 0
    }

    #[cfg(any())]
    pub fn to_json(&self) -> String {
        ::serde_json::to_string_pretty(self).expect("Failed to pretty-print Pairs to json.")
    }

    fn pair(&self) -> usize {
        match self.queue[self.start] {
            QueueableToken::Start {
                end_token_index, ..
            } => end_token_index,
            _ => unreachable!(),
        }
    }

    fn pair_from_end(&self) -> usize {
        match self.queue[self.end - 1] {
            QueueableToken::End {
                start_token_index, ..
            } => start_token_index,
            _ => unreachable!(),
        }
    }

    fn pos(&self, index: usize) -> usize {
        match self.queue[index] {
            QueueableToken::Start { input_pos, .. } | QueueableToken::End { input_pos, .. } => {
                input_pos
            }
        }
    }
}

impl<R: RuleType> ExactSizeIterator for Pairs<'_, R> {
    #[inline]
    fn len(&self) -> usize {
        self.pairs_count
    }
}

impl<'i, R: RuleType> Iterator for Pairs<'i, R> {
    type Item = Pair<'i, R>;

    fn next(&mut self) -> Option<Self::Item> {
        let pair = self.peek()?;

        self.start = self.pair() + 1;
        self.pairs_count -= 1;
        Some(pair)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = <Self as ExactSizeIterator>::len(self);
        (len, Some(len))
    }
}

impl<R: RuleType> DoubleEndedIterator for Pairs<'_, R> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.end <= self.start {
            return None;
        }

        self.end = self.pair_from_end();
        self.pairs_count -= 1;

        let pair = pair::new(
            Rc::clone(&self.queue),
            self.input,
            Rc::clone(&self.line_index),
            self.end,
        );

        Some(pair)
    }
}

impl<R: RuleType> fmt::Debug for Pairs<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

impl<R: RuleType> fmt::Display for Pairs<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self
            .clone()
            .map(|pair| {
                if f.alternate() {
                    format!("{pair:#}")
                } else {
                    format!("{pair}")
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        write!(f, "[{inner}]")
    }
}

impl<'i, R: PartialEq> PartialEq for Pairs<'i, R> {
    fn eq(&self, other: &Pairs<'i, R>) -> bool {
        Rc::ptr_eq(&self.queue, &other.queue)
            && ptr::eq(self.input, other.input)
            && self.start == other.start
            && self.end == other.end
    }
}

impl<R: Eq> Eq for Pairs<'_, R> {}

impl<'i, R: Hash> Hash for Pairs<'i, R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (&*self.queue as *const Vec<QueueableToken<'i, R>>).hash(state);
        (self.input as *const str).hash(state);
        self.start.hash(state);
        self.end.hash(state);
    }
}

#[cfg(any())]
impl<R: RuleType> ::serde::Serialize for Pairs<'_, R> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        let start = self.pos(self.start);
        let end = self.pos(self.end - 1);
        let pairs = self.clone().collect::<Vec<_>>();

        let mut ser = serializer.serialize_struct("Pairs", 2)?;
        ser.serialize_field("pos", &(start, end))?;
        ser.serialize_field("pairs", &pairs)?;
        ser.end()
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/peg/iterators/pairs/tests.rs"]
mod tests;
