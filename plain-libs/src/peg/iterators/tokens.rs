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
use core::str;

use super::queueable_token::QueueableToken;
use crate::peg::RuleType;
use crate::peg::position;
use crate::peg::token::Token;

#[derive(Clone)]
pub struct Tokens<'i, R> {
    queue: Rc<Vec<QueueableToken<'i, R>>>,
    input: &'i str,
    start: usize,
    end: usize,
}

pub fn new<'i, R: RuleType>(
    queue: Rc<Vec<QueueableToken<'i, R>>>,
    input: &'i str,
    start: usize,
    end: usize,
) -> Tokens<'i, R> {
    if cfg!(debug_assertions) {
        for tok in queue.iter() {
            match *tok {
                QueueableToken::Start { input_pos, .. } | QueueableToken::End { input_pos, .. } => {
                    assert!(
                        input.get(input_pos..).is_some(),
                        "💥 INVALID `Tokens` CREATED 💥"
                    )
                }
            }
        }
    }

    Tokens {
        queue,
        input,
        start,
        end,
    }
}

impl<'i, R: RuleType> Tokens<'i, R> {
    fn create_token(&self, index: usize) -> Token<'i, R> {
        match self.queue[index] {
            QueueableToken::Start {
                end_token_index,
                input_pos,
            } => {
                let rule = match self.queue[end_token_index] {
                    QueueableToken::End { rule, .. } => rule,
                    _ => unreachable!(),
                };

                Token::Start {
                    rule,
                    pos: position::Position::new_internal(self.input, input_pos),
                }
            }
            QueueableToken::End {
                rule, input_pos, ..
            } => Token::End {
                rule,
                pos: position::Position::new_internal(self.input, input_pos),
            },
        }
    }
}

impl<R: RuleType> ExactSizeIterator for Tokens<'_, R> {
    fn len(&self) -> usize {
        self.end - self.start
    }
}

impl<'i, R: RuleType> Iterator for Tokens<'i, R> {
    type Item = Token<'i, R>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            return None;
        }

        let token = self.create_token(self.start);

        self.start += 1;

        Some(token)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = <Self as ExactSizeIterator>::len(self);
        (len, Some(len))
    }
}

impl<R: RuleType> DoubleEndedIterator for Tokens<'_, R> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.end <= self.start {
            return None;
        }

        let token = self.create_token(self.end - 1);

        self.end -= 1;

        Some(token)
    }
}

impl<R: RuleType> fmt::Debug for Tokens<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/peg/iterators/tokens/tests.rs"]
mod tests;
