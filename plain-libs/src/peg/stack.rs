// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use alloc::vec;
use alloc::vec::Vec;
use core::ops::{Index, Range};

#[derive(Debug)]
pub struct Stack<T: Clone> {
    cache: Vec<T>,

    popped: Vec<T>,

    lengths: Vec<(usize, usize)>,
}

impl<T: Clone> Default for Stack<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> Stack<T> {
    pub fn new() -> Self {
        Stack {
            cache: vec![],
            popped: vec![],
            lengths: vec![],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    pub fn peek(&self) -> Option<&T> {
        self.cache.last()
    }

    pub fn push(&mut self, elem: T) {
        self.cache.push(elem);
    }

    pub fn pop(&mut self) -> Option<T> {
        let len = self.cache.len();
        let popped = self.cache.pop();
        if let Some(popped) = &popped {
            if let Some((_, remained_count)) = self.lengths.last_mut() {
                // `len >= *unpopped_count`
                if len == *remained_count {
                    *remained_count -= 1;
                    self.popped.push(popped.clone());
                }
            }
        }
        popped
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn snapshot(&mut self) {
        self.lengths.push((self.cache.len(), self.cache.len()))
    }

    pub fn clear_snapshot(&mut self) {
        if let Some((len, remained)) = self.lengths.pop() {
            let popped_count = len - remained;
            if let Some((_, parent_remained)) = self.lengths.last_mut() {
                let merged_remained = (*parent_remained).min(remained);
                let parent_popped = *parent_remained - merged_remained;
                *parent_remained = merged_remained;

                let popped_start = self.popped.len() - popped_count;
                drop(
                    self.popped
                        .drain(popped_start..popped_start + popped_count - parent_popped),
                );
            } else {
                self.popped.truncate(self.popped.len() - popped_count);
            }
        }
    }

    pub fn restore(&mut self) {
        match self.lengths.pop() {
            Some((len_stack, remained)) => {
                if remained < self.cache.len() {
                    // Remove those elements that are pushed after the snapshot.
                    self.cache.truncate(remained);
                }
                if len_stack > remained {
                    let rewind_count = len_stack - remained;
                    let new_len = self.popped.len() - rewind_count;
                    let recovered_elements = self.popped.drain(new_len..);
                    self.cache.extend(recovered_elements.rev());
                    debug_assert_eq!(self.popped.len(), new_len);
                }
            }
            None => {
                self.cache.clear();
                // As `self.popped` and `self.lengths` should already be empty,
                // there is no need to clear it.
                debug_assert!(self.popped.is_empty());
                debug_assert!(self.lengths.is_empty());
            }
        }
    }
}

impl<T: Clone> Index<Range<usize>> for Stack<T> {
    type Output = [T];

    fn index(&self, range: Range<usize>) -> &[T] {
        self.cache.index(range)
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/peg/stack/test.rs"]
mod test;
