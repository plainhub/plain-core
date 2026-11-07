// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::fmt;

use super::line_index::LineIndex;
use super::pair::{self, Pair};
use super::queueable_token::QueueableToken;
use super::tokens::{self, Tokens};
use crate::peg::RuleType;

pub struct FlatPairs<'i, R> {
    queue: Rc<Vec<QueueableToken<'i, R>>>,
    input: &'i str,
    start: usize,
    end: usize,
    line_index: Rc<LineIndex>,
}

pub fn new<'i, R: RuleType>(
    queue: Rc<Vec<QueueableToken<'i, R>>>,
    input: &'i str,
    line_index: Rc<LineIndex>,
    start: usize,
    end: usize,
) -> FlatPairs<'i, R> {
    FlatPairs {
        queue,
        input,
        line_index,
        start,
        end,
    }
}

impl<'i, R: RuleType> FlatPairs<'i, R> {
    #[inline]
    pub fn tokens(self) -> Tokens<'i, R> {
        tokens::new(self.queue, self.input, self.start, self.end)
    }

    fn next_start(&mut self) {
        self.start += 1;

        while self.start < self.end && !self.is_start(self.start) {
            self.start += 1;
        }
    }

    fn next_start_from_end(&mut self) {
        self.end -= 1;

        while self.end >= self.start && !self.is_start(self.end) {
            self.end -= 1;
        }
    }

    fn is_start(&self, index: usize) -> bool {
        match self.queue[index] {
            QueueableToken::Start { .. } => true,
            QueueableToken::End { .. } => false,
        }
    }
}

impl<R: RuleType> ExactSizeIterator for FlatPairs<'_, R> {
    fn len(&self) -> usize {
        // Tokens len is exactly twice as flatten pairs len
        (self.end - self.start) >> 1
    }
}

impl<'i, R: RuleType> Iterator for FlatPairs<'i, R> {
    type Item = Pair<'i, R>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            return None;
        }

        let pair = pair::new(
            Rc::clone(&self.queue),
            self.input,
            Rc::clone(&self.line_index),
            self.start,
        );
        self.next_start();

        Some(pair)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = <Self as ExactSizeIterator>::len(self);
        (len, Some(len))
    }
}

impl<R: RuleType> DoubleEndedIterator for FlatPairs<'_, R> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.end <= self.start {
            return None;
        }

        self.next_start_from_end();

        let pair = pair::new(
            Rc::clone(&self.queue),
            self.input,
            Rc::clone(&self.line_index),
            self.end,
        );

        Some(pair)
    }
}

impl<R: RuleType> fmt::Debug for FlatPairs<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlatPairs")
            .field("pairs", &self.clone().collect::<Vec<_>>())
            .finish()
    }
}

impl<'i, R: Clone> Clone for FlatPairs<'i, R> {
    fn clone(&self) -> FlatPairs<'i, R> {
        FlatPairs {
            queue: Rc::clone(&self.queue),
            input: self.input,
            line_index: Rc::clone(&self.line_index),
            start: self.start,
            end: self.end,
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/peg/iterators/flat_pairs/tests.rs"]
mod tests;
