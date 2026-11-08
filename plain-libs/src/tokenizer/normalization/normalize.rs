// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Functions for computing canonical and compatible decompositions for Unicode characters.
use super::lookups::{
    canonical_fully_decomposed, compatibility_fully_decomposed, composition_table,
};
use std::char;

/// Compute canonical Unicode decomposition for character.
/// See [Unicode Standard Annex #15](http://www.unicode.org/reports/tr15/)
/// for more information.
#[inline]
pub(super) fn decompose_canonical<F>(c: char, emit_char: F)
where
    F: FnMut(char),
{
    decompose(c, canonical_fully_decomposed, emit_char)
}

/// Compute canonical or compatible Unicode decomposition for character.
/// See [Unicode Standard Annex #15](http://www.unicode.org/reports/tr15/)
/// for more information.
#[inline]
pub(super) fn decompose_compatible<F: FnMut(char)>(c: char, emit_char: F) {
    let decompose_char =
        |c| compatibility_fully_decomposed(c).or_else(|| canonical_fully_decomposed(c));
    decompose(c, decompose_char, emit_char)
}

#[inline]
fn decompose<D, F>(c: char, decompose_char: D, mut emit_char: F)
where
    D: Fn(char) -> Option<&'static [char]>,
    F: FnMut(char),
{
    // 7-bit ASCII never decomposes
    if c <= '\x7f' {
        emit_char(c);
        return;
    }

    // Perform decomposition for Hangul
    if is_hangul_syllable(c) {
        decompose_hangul(c, emit_char);
        return;
    }

    if let Some(decomposed) = decompose_char(c) {
        for &d in decomposed {
            emit_char(d);
        }
        return;
    }

    // Finally bottom out.
    emit_char(c);
}

/// Compose two characters into a single character, if possible.
/// See [Unicode Standard Annex #15](http://www.unicode.org/reports/tr15/)
/// for more information.
pub(super) fn compose(a: char, b: char) -> Option<char> {
    compose_hangul(a, b).or_else(|| composition_table(a, b))
}

// Constants from Unicode 9.0.0 Section 3.12 Conjoining Jamo Behavior
// http://www.unicode.org/versions/Unicode9.0.0/ch03.pdf#M9.32468.Heading.310.Combining.Jamo.Behavior
const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

const S_LAST: u32 = S_BASE + S_COUNT - 1;
const L_LAST: u32 = L_BASE + L_COUNT - 1;
const V_LAST: u32 = V_BASE + V_COUNT - 1;
const T_LAST: u32 = T_BASE + T_COUNT - 1;

// Composition only occurs for `TPart`s in `U+11A8 ..= U+11C2`,
// i.e. `T_BASE + 1 ..= T_LAST`.
const T_FIRST: u32 = T_BASE + 1;

fn is_hangul_syllable(c: char) -> bool {
    (c as u32) >= S_BASE && (c as u32) < (S_BASE + S_COUNT)
}

// Decompose a precomposed Hangul syllable
#[inline(always)]
fn decompose_hangul<F>(s: char, mut emit_char: F)
where
    F: FnMut(char),
{
    let s_index = s as u32 - S_BASE;
    let l_index = s_index / N_COUNT;
    emit_char(char::from_u32(L_BASE + l_index).unwrap());
    let v_index = (s_index % N_COUNT) / T_COUNT;
    emit_char(char::from_u32(V_BASE + v_index).unwrap());
    let t_index = s_index % T_COUNT;
    if t_index > 0 {
        emit_char(char::from_u32(T_BASE + t_index).unwrap());
    }
}

// Compose a pair of Hangul Jamo
#[inline(always)]
fn compose_hangul(a: char, b: char) -> Option<char> {
    let (a, b) = (a as u32, b as u32);
    match (a, b) {
        // Compose a leading consonant and a vowel together into an LV_Syllable
        (L_BASE..=L_LAST, V_BASE..=V_LAST) => {
            let l_index = a - L_BASE;
            let v_index = b - V_BASE;
            let lv_index = l_index * N_COUNT + v_index * T_COUNT;
            let s = S_BASE + lv_index;
            char::from_u32(s)
        }
        // Compose an LV_Syllable and a trailing consonant into an LVT_Syllable
        (S_BASE..=S_LAST, T_FIRST..=T_LAST) if (a - S_BASE) % T_COUNT == 0 => {
            char::from_u32(a + (b - T_BASE))
        }
        _ => None,
    }
}
