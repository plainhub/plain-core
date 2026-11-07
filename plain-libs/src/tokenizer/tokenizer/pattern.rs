use crate::tokenizer::utils::SysRegex;
use crate::tokenizer::{Offsets, Result};
use regex::Regex;

pub trait Pattern {
    fn find_matches(&self, inside: &str) -> Result<Vec<(Offsets, bool)>>;
}

impl Pattern for char {
    fn find_matches(&self, inside: &str) -> Result<Vec<(Offsets, bool)>> {
        let is_char = |c: char| -> bool { c == *self };
        is_char.find_matches(inside)
    }
}

impl Pattern for &str {
    fn find_matches(&self, inside: &str) -> Result<Vec<(Offsets, bool)>> {
        if self.is_empty() {
            // If we try to find the matches with an empty string, just don't match anything
            return Ok(vec![((0, inside.chars().count()), false)]);
        }

        let re = Regex::new(&regex::escape(self))?;
        (&re).find_matches(inside)
    }
}

impl Pattern for &String {
    fn find_matches(&self, inside: &str) -> Result<Vec<(Offsets, bool)>> {
        let s: &str = self;
        s.find_matches(inside)
    }
}

impl Pattern for &Regex {
    fn find_matches(&self, inside: &str) -> Result<Vec<(Offsets, bool)>> {
        if inside.is_empty() {
            return Ok(vec![((0, 0), false)]);
        }

        let mut prev = 0;
        let mut splits = Vec::with_capacity(inside.len());
        for m in self.find_iter(inside) {
            if prev != m.start() {
                splits.push(((prev, m.start()), false));
            }
            splits.push(((m.start(), m.end()), true));
            prev = m.end();
        }
        if prev != inside.len() {
            splits.push(((prev, inside.len()), false))
        }
        Ok(splits)
    }
}

impl Pattern for &SysRegex {
    fn find_matches(&self, inside: &str) -> Result<Vec<(Offsets, bool)>> {
        if inside.is_empty() {
            return Ok(vec![((0, 0), false)]);
        }

        let mut prev = 0;
        let mut splits = Vec::with_capacity(inside.len());
        for (start, end) in self.find_iter(inside) {
            if prev != start {
                splits.push(((prev, start), false));
            }
            splits.push(((start, end), true));
            prev = end;
        }
        if prev != inside.len() {
            splits.push(((prev, inside.len()), false))
        }
        Ok(splits)
    }
}

impl<F> Pattern for F
where
    F: Fn(char) -> bool,
{
    fn find_matches(&self, inside: &str) -> Result<Vec<(Offsets, bool)>> {
        if inside.is_empty() {
            return Ok(vec![((0, 0), false)]);
        }

        let mut last_offset = 0;
        let mut last_seen = 0;

        let mut matches = inside
            .char_indices()
            .flat_map(|(b, c)| {
                last_seen = b + c.len_utf8();
                if self(c) {
                    let mut events = Vec::with_capacity(2);
                    if last_offset < b {
                        // We need to emit what was before this match
                        events.push(((last_offset, b), false));
                    }
                    events.push(((b, b + c.len_utf8()), true));
                    last_offset = b + c.len_utf8();
                    events
                } else {
                    vec![]
                }
            })
            .collect::<Vec<_>>();

        // Do not forget the last potential split
        if last_seen > last_offset {
            matches.push(((last_offset, last_seen), false));
        }

        Ok(matches)
    }
}

pub struct Invert<P: Pattern>(pub P);
impl<P: Pattern> Pattern for Invert<P> {
    fn find_matches(&self, inside: &str) -> Result<Vec<(Offsets, bool)>> {
        Ok(self
            .0
            .find_matches(inside)?
            .into_iter()
            .map(|(offsets, flag)| (offsets, !flag))
            .collect())
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/tokenizer/pattern/tests.rs"]
mod tests;
