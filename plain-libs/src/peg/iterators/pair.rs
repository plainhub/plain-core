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
#[cfg(any())]
use alloc::string::String;
use alloc::vec::Vec;
use core::borrow::Borrow;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ptr;
use core::str;

#[cfg(any())]
use serde::ser::SerializeStruct;

use super::line_index::LineIndex;
use super::pairs::{self, Pairs};
use super::queueable_token::QueueableToken;
use super::tokens::{self, Tokens};
use crate::peg::RuleType;
use crate::peg::span::Span;

#[derive(Clone)]
pub struct Pair<'i, R> {
    queue: Rc<Vec<QueueableToken<'i, R>>>,
    input: &'i str,

    start: usize,
    line_index: Rc<LineIndex>,
}

pub fn new<'i, R: RuleType>(
    queue: Rc<Vec<QueueableToken<'i, R>>>,
    input: &'i str,
    line_index: Rc<LineIndex>,
    start: usize,
) -> Pair<'i, R> {
    Pair {
        queue,
        input,
        start,
        line_index,
    }
}

impl<'i, R: RuleType> Pair<'i, R> {
    #[inline]
    pub fn as_rule(&self) -> R {
        match self.queue[self.pair()] {
            QueueableToken::End { rule, .. } => rule,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn as_str(&self) -> &'i str {
        let start = self.pos(self.start);
        let end = self.pos(self.pair());

        // Generated positions always come from Positions and are UTF-8 borders.
        &self.input[start..end]
    }

    pub fn get_input(&self) -> &'i str {
        self.input
    }

    #[inline]
    #[deprecated(since = "2.0.0", note = "Please use `as_span` instead")]
    pub fn into_span(self) -> Span<'i> {
        self.as_span()
    }

    #[inline]
    pub fn as_span(&self) -> Span<'i> {
        let start = self.pos(self.start);
        let end = self.pos(self.pair());

        Span::new_internal(self.input, start, end)
    }

    #[inline]
    pub fn as_node_tag(&self) -> Option<&str> {
        match &self.queue[self.pair()] {
            QueueableToken::End { tag, .. } => tag.as_ref().map(|x| x.borrow()),
            _ => None,
        }
    }

    #[inline]
    pub fn into_inner(self) -> Pairs<'i, R> {
        let pair = self.pair();

        pairs::new(
            self.queue,
            self.input,
            Some(self.line_index),
            self.start + 1,
            pair,
        )
    }

    #[inline]
    pub fn tokens(self) -> Tokens<'i, R> {
        let end = self.pair();

        tokens::new(self.queue, self.input, self.start, end + 1)
    }

    #[cfg(any())]
    pub fn to_json(&self) -> String {
        ::serde_json::to_string_pretty(self).expect("Failed to pretty-print Pair to json.")
    }

    pub fn line_col(&self) -> (usize, usize) {
        let pos = self.pos(self.start);
        self.line_index.line_col(self.input, pos)
    }

    fn pair(&self) -> usize {
        match self.queue[self.start] {
            QueueableToken::Start {
                end_token_index, ..
            } => end_token_index,
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

impl<'i, R: RuleType> Pairs<'i, R> {
    pub fn single(pair: Pair<'i, R>) -> Self {
        let end = pair.pair();
        pairs::new(
            pair.queue,
            pair.input,
            Some(pair.line_index),
            pair.start,
            end,
        )
    }
}

impl<R: RuleType> fmt::Debug for Pair<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pair = &mut f.debug_struct("Pair");
        pair.field("rule", &self.as_rule());
        // In order not to break compatibility
        if let Some(s) = self.as_node_tag() {
            pair.field("node_tag", &s);
        }
        pair.field("span", &self.as_span())
            .field("inner", &self.clone().into_inner().collect::<Vec<_>>())
            .finish()
    }
}

impl<R: RuleType> fmt::Display for Pair<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            let rule = self.as_rule();
            let start = self.pos(self.start);
            let end = self.pos(self.pair());
            let mut pairs = self.clone().into_inner().peekable();

            if pairs.peek().is_none() {
                write!(f, "{rule:?}({start}, {end})")
            } else {
                write!(
                    f,
                    "{:?}({}, {}, [{}])",
                    rule,
                    start,
                    end,
                    pairs
                        .map(|pair| format!("{pair:#}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        } else {
            write!(f, "{}", self.as_str())
        }
    }
}

impl<'i, R: PartialEq> PartialEq for Pair<'i, R> {
    fn eq(&self, other: &Pair<'i, R>) -> bool {
        Rc::ptr_eq(&self.queue, &other.queue)
            && ptr::eq(self.input, other.input)
            && self.start == other.start
    }
}

impl<R: Eq> Eq for Pair<'_, R> {}

impl<'i, R: Hash> Hash for Pair<'i, R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (&*self.queue as *const Vec<QueueableToken<'i, R>>).hash(state);
        (self.input as *const str).hash(state);
        self.start.hash(state);
    }
}

#[cfg(any())]
impl<R: RuleType> ::serde::Serialize for Pair<'_, R> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        let start = self.pos(self.start);
        let end = self.pos(self.pair());
        let rule = format!("{:?}", self.as_rule());
        let inner = self.clone().into_inner();

        let mut ser = serializer.serialize_struct("Pairs", 3)?;
        ser.serialize_field("pos", &(start, end))?;
        ser.serialize_field("rule", &rule)?;

        if inner.peek().is_none() {
            ser.serialize_field("inner", &self.as_str())?;
        } else {
            ser.serialize_field("inner", &inner)?;
        }

        ser.end()
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/peg/iterators/pair/tests.rs"]
mod tests;
