// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::Range;
use core::ptr;
use core::str;

use crate::peg::span;

#[derive(Clone, Copy)]
pub struct Position<'i> {
    input: &'i str,
    pos: usize,
}

impl<'i> Position<'i> {
    pub(crate) fn new_internal(input: &str, pos: usize) -> Position<'_> {
        debug_assert!(input.get(pos..).is_some());
        Position { input, pos }
    }

    pub fn new(input: &str, pos: usize) -> Option<Position<'_>> {
        input.get(pos..).map(|_| Position { input, pos })
    }

    #[inline]
    pub fn from_start(input: &'i str) -> Position<'i> {
        // Position 0 is always safe because it's always a valid UTF-8 border.
        Position { input, pos: 0 }
    }

    #[inline]
    pub fn pos(&self) -> usize {
        self.pos
    }

    #[inline]
    pub fn span(&self, other: &Position<'i>) -> span::Span<'i> {
        if ptr::eq(self.input, other.input)
        /* && self.input.get(self.pos..other.pos).is_some() */
        {
            span::Span::new_internal(self.input, self.pos, other.pos)
        } else {
            // TODO: maybe a panic if self.pos < other.pos
            panic!("span created from positions from different inputs")
        }
    }

    #[inline]
    pub fn line_col(&self) -> (usize, usize) {
        if self.pos > self.input.len() {
            panic!("position out of bounds");
        }
        let mut pos = self.pos;
        let slice = &self.input[..pos];
        let mut chars = slice.chars().peekable();

        let mut line_col = (1, 1);

        while pos != 0 {
            match chars.next() {
                Some('\r') => {
                    if let Some(&'\n') = chars.peek() {
                        chars.next();

                        if pos == 1 {
                            pos -= 1;
                        } else {
                            pos -= 2;
                        }

                        line_col = (line_col.0 + 1, 1);
                    } else {
                        pos -= 1;
                        line_col = (line_col.0, line_col.1 + 1);
                    }
                }
                Some('\n') => {
                    pos -= 1;
                    line_col = (line_col.0 + 1, 1);
                }
                Some(c) => {
                    pos -= c.len_utf8();
                    line_col = (line_col.0, line_col.1 + 1);
                }
                None => unreachable!(),
            }
        }

        line_col
    }

    #[inline]
    pub fn line_of(&self) -> &'i str {
        if self.pos > self.input.len() {
            panic!("position out of bounds");
        };
        // Safe since start and end can only be valid UTF-8 borders.
        &self.input[self.find_line_start()..self.find_line_end()]
    }

    pub(crate) fn find_line_start(&self) -> usize {
        if self.input.is_empty() {
            return 0;
        };
        // Position's pos is always a UTF-8 border.
        let start = self
            .input
            .char_indices()
            .rev()
            .skip_while(|&(i, _)| i >= self.pos)
            .find(|&(_, c)| c == '\n');
        match start {
            Some((i, _)) => i + 1,
            None => 0,
        }
    }

    pub(crate) fn find_line_end(&self) -> usize {
        if self.input.is_empty() {
            0
        } else if self.pos == self.input.len() - 1 {
            self.input.len()
        } else {
            // Position's pos is always a UTF-8 border.
            let end = self
                .input
                .char_indices()
                .skip_while(|&(i, _)| i < self.pos)
                .find(|&(_, c)| c == '\n');
            match end {
                Some((i, _)) => i + 1,
                None => self.input.len(),
            }
        }
    }

    #[inline]
    pub(crate) fn at_start(&self) -> bool {
        self.pos == 0
    }

    #[inline]
    pub(crate) fn at_end(&self) -> bool {
        self.pos == self.input.len()
    }

    #[inline]
    pub(crate) fn skip(&mut self, n: usize) -> bool {
        let skipped = {
            let mut len = 0;
            // Position's pos is always a UTF-8 border.
            let mut chars = self.input[self.pos..].chars();
            for _ in 0..n {
                if let Some(c) = chars.next() {
                    len += c.len_utf8();
                } else {
                    return false;
                }
            }
            len
        };

        self.pos += skipped;
        true
    }

    #[inline]
    pub(crate) fn skip_back(&mut self, n: usize) -> bool {
        let skipped = {
            let mut len = 0;
            // Position's pos is always a UTF-8 border.
            let mut chars = self.input[..self.pos].chars().rev();
            for _ in 0..n {
                if let Some(c) = chars.next() {
                    len += c.len_utf8();
                } else {
                    return false;
                }
            }
            len
        };

        self.pos -= skipped;
        true
    }

    #[inline]
    pub(crate) fn skip_until(&mut self, strings: &[&str]) -> bool {
        #[cfg(not(feature = "peg"))]
        {
            self.skip_until_basic(strings)
        }
        #[cfg(feature = "peg")]
        {
            match strings {
                [] => (),
                [s1] => {
                    if let Some(from) =
                        crate::memchr::memmem::find(&self.input.as_bytes()[self.pos..], s1.as_bytes())
                    {
                        self.pos += from;
                        return true;
                    }
                }
                [s1, s2] if !s1.is_empty() && !s2.is_empty() => {
                    let b1 = s1.as_bytes()[0];
                    let b2 = s2.as_bytes()[0];
                    let miter = crate::memchr::memchr2_iter(b1, b2, &self.input.as_bytes()[self.pos..]);
                    for from in miter {
                        let start = &self.input[self.pos + from..];
                        if start.starts_with(s1) || start.starts_with(s2) {
                            self.pos += from;
                            return true;
                        }
                    }
                }
                [s1, s2, s3] if !s1.is_empty() && !s2.is_empty() && s3.is_empty() => {
                    let b1 = s1.as_bytes()[0];
                    let b2 = s2.as_bytes()[0];
                    let b3 = s2.as_bytes()[0];
                    let miter =
                        crate::memchr::memchr3_iter(b1, b2, b3, &self.input.as_bytes()[self.pos..]);
                    for from in miter {
                        let start = &self.input[self.pos + from..];
                        if start.starts_with(s1) || start.starts_with(s2) || start.starts_with(s3) {
                            self.pos += from;
                            return true;
                        }
                    }
                }
                _ => {
                    return self.skip_until_basic(strings);
                }
            }
            self.pos = self.input.len();
            false
        }
    }

    #[inline]
    fn skip_until_basic(&mut self, strings: &[&str]) -> bool {
        // TODO: optimize with Aho-Corasick, e.g. https://crates.io/crates/daachorse?
        for from in self.pos..self.input.len() {
            let bytes = if let Some(string) = self.input.get(from..) {
                string.as_bytes()
            } else {
                continue;
            };

            for slice in strings.iter() {
                let to = slice.len();
                if Some(slice.as_bytes()) == bytes.get(0..to) {
                    self.pos = from;
                    return true;
                }
            }
        }

        self.pos = self.input.len();
        false
    }

    #[inline]
    pub(crate) fn match_char(&self, c: char) -> bool {
        matches!(self.input[self.pos..].chars().next(), Some(cc) if c == cc)
    }

    #[inline]
    pub(crate) fn match_char_by<F>(&mut self, f: F) -> bool
    where
        F: FnOnce(char) -> bool,
    {
        if let Some(c) = self.input[self.pos..].chars().next() {
            if f(c) {
                self.pos += c.len_utf8();
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    #[inline]
    pub(crate) fn match_string(&mut self, string: &str) -> bool {
        let to = self.pos + string.len();

        if Some(string.as_bytes()) == self.input.as_bytes().get(self.pos..to) {
            self.pos = to;
            true
        } else {
            false
        }
    }

    #[inline]
    pub(crate) fn match_insensitive(&mut self, string: &str) -> bool {
        let matched = {
            let slice = &self.input[self.pos..];
            if let Some(slice) = slice.get(0..string.len()) {
                slice.eq_ignore_ascii_case(string)
            } else {
                false
            }
        };

        if matched {
            self.pos += string.len();
            true
        } else {
            false
        }
    }

    #[inline]
    pub(crate) fn match_range(&mut self, range: Range<char>) -> bool {
        if let Some(c) = self.input[self.pos..].chars().next() {
            if range.start <= c && c <= range.end {
                self.pos += c.len_utf8();
                return true;
            }
        }

        false
    }
}

impl fmt::Debug for Position<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Position").field("pos", &self.pos).finish()
    }
}

impl<'i> PartialEq for Position<'i> {
    fn eq(&self, other: &Position<'i>) -> bool {
        ptr::eq(self.input, other.input) && self.pos == other.pos
    }
}

impl Eq for Position<'_> {}

impl<'i> PartialOrd for Position<'i> {
    fn partial_cmp(&self, other: &Position<'i>) -> Option<Ordering> {
        if ptr::eq(self.input, other.input) {
            self.pos.partial_cmp(&other.pos)
        } else {
            None
        }
    }
}

impl<'i> Ord for Position<'i> {
    fn cmp(&self, other: &Position<'i>) -> Ordering {
        self.partial_cmp(other)
            .expect("cannot compare positions from different strs")
    }
}

impl Hash for Position<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.input as *const str).hash(state);
        self.pos.hash(state);
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/peg/position/tests.rs"]
mod tests;
