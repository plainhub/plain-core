// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::{Bound, RangeBounds};
use core::ptr;
use core::str;

use crate::peg::position;

#[derive(Clone, Copy)]
pub struct Span<'i> {
    input: &'i str,
    start: usize,
    end: usize,
}

impl<'i> Span<'i> {
    pub(crate) fn new_internal(input: &str, start: usize, end: usize) -> Span<'_> {
        debug_assert!(input.get(start..end).is_some());
        Span { input, start, end }
    }

    pub fn new(input: &str, start: usize, end: usize) -> Option<Span<'_>> {
        if input.get(start..end).is_some() {
            Some(Span { input, start, end })
        } else {
            None
        }
    }

    pub fn get(&self, range: impl RangeBounds<usize>) -> Option<Span<'i>> {
        let start = match range.start_bound() {
            Bound::Included(offset) => *offset,
            Bound::Excluded(offset) => *offset + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(offset) => *offset + 1,
            Bound::Excluded(offset) => *offset,
            Bound::Unbounded => self.as_str().len(),
        };

        self.as_str().get(start..end).map(|_| Span {
            input: self.input,
            start: self.start + start,
            end: self.start + end,
        })
    }

    #[inline]
    pub fn start(&self) -> usize {
        self.start
    }

    #[inline]
    pub fn end(&self) -> usize {
        self.end
    }

    #[inline]
    pub fn start_pos(&self) -> position::Position<'i> {
        position::Position::new_internal(self.input, self.start)
    }

    #[inline]
    pub fn end_pos(&self) -> position::Position<'i> {
        position::Position::new_internal(self.input, self.end)
    }

    #[inline]
    pub fn split(self) -> (position::Position<'i>, position::Position<'i>) {
        let pos1 = position::Position::new_internal(self.input, self.start);
        let pos2 = position::Position::new_internal(self.input, self.end);

        (pos1, pos2)
    }

    #[inline]
    pub fn as_str(&self) -> &'i str {
        // Span's start and end positions are always a UTF-8 borders.
        &self.input[self.start..self.end]
    }

    pub fn get_input(&self) -> &'i str {
        self.input
    }

    #[inline]
    pub fn lines(&self) -> Lines<'_> {
        Lines {
            inner: self.lines_span(),
        }
    }

    pub fn lines_span(&self) -> LinesSpan<'_> {
        LinesSpan {
            span: self,
            pos: self.start,
        }
    }
}

impl fmt::Debug for Span<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let range = self.start..self.end;
        f.debug_struct("Span")
            .field("str", &self.as_str())
            .field("range", &range)
            .finish()
    }
}

impl<'i> PartialEq for Span<'i> {
    fn eq(&self, other: &Span<'i>) -> bool {
        ptr::eq(self.input, other.input) && self.start == other.start && self.end == other.end
    }
}

impl Eq for Span<'_> {}

impl Hash for Span<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.input as *const str).hash(state);
        self.start.hash(state);
        self.end.hash(state);
    }
}

pub fn merge_spans<'i>(a: &Span<'i>, b: &Span<'i>) -> Option<Span<'i>> {
    if a.end() >= b.start() && a.start() <= b.end() {
        // The spans overlap or are contiguous, so they can be merged.
        Span::new(
            a.get_input(),
            core::cmp::min(a.start(), b.start()),
            core::cmp::max(a.end(), b.end()),
        )
    } else {
        // The spans don't overlap and aren't contiguous, so they can't be merged.
        None
    }
}

pub struct LinesSpan<'i> {
    span: &'i Span<'i>,
    pos: usize,
}

impl<'i> Iterator for LinesSpan<'i> {
    type Item = Span<'i>;
    fn next(&mut self) -> Option<Self::Item> {
        // `pos` holds the start of the next line to yield, so every line the span
        // covers has been yielded once `pos` has reached the end of the span. The
        // exception is a zero-width span, which still sits on one line: that line is
        // yielded by the first call, while `pos` is still the start of the span.
        if self.pos >= self.span.end && self.pos > self.span.start {
            return None;
        }
        let pos = position::Position::new(self.span.input, self.pos)?;
        if pos.at_end() {
            return None;
        }

        let line_start = pos.find_line_start();
        self.pos = pos.find_line_end();

        Span::new(self.span.input, line_start, self.pos)
    }
}

pub struct Lines<'i> {
    inner: LinesSpan<'i>,
}

impl<'i> Iterator for Lines<'i> {
    type Item = &'i str;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|span| span.as_str())
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/peg/span/tests.rs"]
mod tests;
