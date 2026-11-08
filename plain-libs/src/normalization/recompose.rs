// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::{Character, decompose::Decompositions};
use smallvec::SmallVec;

enum RecompositionState {
    Composing,
    Purging(usize),
    Finished(usize),
}

/// External iterator for a string recomposition's characters.
pub(super) struct Recompositions<I, A: Character, const MODERN: bool> {
    iter: Decompositions<I, A, MODERN>,
    state: RecompositionState,
    buffer: SmallVec<[A; 4]>,
    composee: Option<A>,
    last_ccc: Option<u8>,
}

#[inline]
pub(super) fn new_canonical<I: Iterator<Item = char>, A: Character, const MODERN: bool>(
    iter: I,
) -> Recompositions<I, A, MODERN> {
    Recompositions {
        iter: super::decompose::new_canonical(iter),
        state: self::RecompositionState::Composing,
        buffer: SmallVec::new(),
        composee: None,
        last_ccc: None,
    }
}

#[inline]
#[cfg(feature = "ai-image-search")]
pub(super) fn new_compatible<I: Iterator<Item = char>>(
    iter: I,
) -> Recompositions<I, (char, isize), false> {
    Recompositions {
        iter: super::decompose::new_compatible(iter),
        state: self::RecompositionState::Composing,
        buffer: SmallVec::new(),
        composee: None,
        last_ccc: None,
    }
}

impl<I: Iterator<Item = char>, A: Character, const MODERN: bool> Iterator
    for Recompositions<I, A, MODERN>
{
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        use self::RecompositionState::*;

        loop {
            match self.state {
                Composing => {
                    for item in self.iter.by_ref() {
                        let ch = item.character();
                        let ch_class = super::lookups::canonical_combining_class::<MODERN>(ch);
                        let k = match self.composee {
                            None => {
                                if ch_class != 0 {
                                    return Some(item);
                                }
                                self.composee = Some(item);
                                continue;
                            }
                            Some(k) => k,
                        };
                        match self.last_ccc {
                            None => match super::normalize::compose::<MODERN>(k.character(), ch) {
                                Some(r) => {
                                    self.composee = Some(k.merge(item, r));
                                    continue;
                                }
                                None => {
                                    if ch_class == 0 {
                                        self.composee = Some(item);
                                        return Some(k);
                                    }
                                    self.buffer.push(item);
                                    self.last_ccc = Some(ch_class);
                                }
                            },
                            Some(l_class) => {
                                if l_class >= ch_class {
                                    // `ch` is blocked from `composee`
                                    if ch_class == 0 {
                                        self.composee = Some(item);
                                        self.last_ccc = None;
                                        self.state = Purging(0);
                                        return Some(k);
                                    }
                                    self.buffer.push(item);
                                    self.last_ccc = Some(ch_class);
                                    continue;
                                }
                                match super::normalize::compose::<MODERN>(k.character(), ch) {
                                    Some(r) => {
                                        self.composee = Some(k.merge(item, r));
                                        continue;
                                    }
                                    None => {
                                        self.buffer.push(item);
                                        self.last_ccc = Some(ch_class);
                                    }
                                }
                            }
                        }
                    }
                    self.state = Finished(0);
                    if self.composee.is_some() {
                        return self.composee.take();
                    }
                }
                Purging(next) => match self.buffer.get(next).cloned() {
                    None => {
                        self.buffer.clear();
                        self.state = Composing;
                    }
                    s => {
                        self.state = Purging(next + 1);
                        return s;
                    }
                },
                Finished(next) => match self.buffer.get(next).cloned() {
                    None => {
                        self.buffer.clear();
                        return self.composee.take();
                    }
                    s => {
                        self.state = Finished(next + 1);
                        return s;
                    }
                },
            }
        }
    }
}
