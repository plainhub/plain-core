// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod tables;

use tables::grapheme::{GraphemeCat, GraphemeCat::*};

pub(super) fn graphemes(text: &str) -> impl Iterator<Item = &str> {
    let mut chars = text.char_indices().peekable();
    let mut context = Context::default();
    std::iter::from_fn(move || {
        let &(start, _) = chars.peek()?;
        while let Some((_, ch)) = chars.next() {
            let before = context.category(ch);
            context.consume(ch, before);
            let Some(&(end, next)) = chars.peek() else {
                break;
            };
            let after = context.category(next);
            if context.is_break(before, after) {
                return Some(&text[start..end]);
            }
        }
        Some(&text[start..])
    })
}

#[derive(Default)]
struct Context {
    category_cache: Option<(u32, u32, GraphemeCat)>,
    odd_regional_run: bool,
    indic_consonant: bool,
    indic_linker: bool,
    pictograph_run: bool,
    pictograph_zwj: bool,
}

impl Context {
    fn category(&mut self, ch: char) -> GraphemeCat {
        if ch <= '\u{7e}' {
            return match ch {
                ' '..='~' => Any,
                '\r' => Cr,
                '\n' => Lf,
                _ => Control,
            };
        }
        let code = ch as u32;
        if let Some((start, end, cat)) = self.category_cache {
            if (start..=end).contains(&code) {
                return cat;
            }
        }
        let entry = tables::grapheme::grapheme_category(ch);
        self.category_cache = Some(entry);
        entry.2
    }

    fn consume(&mut self, ch: char, category: GraphemeCat) {
        self.odd_regional_run = category == RegionalIndicator && !self.odd_regional_run;
        self.pictograph_zwj = category == Zwj && self.pictograph_run;
        self.pictograph_run =
            category == ExtendedPictographic || category == Extend && self.pictograph_run;

        if tables::is_incb_linker(ch) {
            self.indic_linker = true;
        } else if !tables::derived_property::is_incb_extend(ch) {
            self.indic_consonant = category == InCBConsonant;
            self.indic_linker = false;
        }
    }

    fn is_break(&self, before: GraphemeCat, after: GraphemeCat) -> bool {
        match (before, after) {
            (Cr, Lf) => false,                                                  // GB3
            (Control | Cr | Lf, _) | (_, Control | Cr | Lf) => true,            // GB4, GB5
            (L, L | V | Lv | Lvt) => false,                                     // GB6
            (Lv | V, V | T) => false,                                           // GB7
            (Lvt | T, T) => false,                                              // GB8
            (_, Extend | Zwj | SpacingMark) | (Prepend, _) => false,            // GB9–GB9b
            (_, InCBConsonant) => !(self.indic_consonant && self.indic_linker), // GB9c
            (Zwj, ExtendedPictographic) => !self.pictograph_zwj,                // GB11
            (RegionalIndicator, RegionalIndicator) => !self.odd_regional_run,   // GB12, GB13
            _ => true,                                                          // GB999
        }
    }
}
