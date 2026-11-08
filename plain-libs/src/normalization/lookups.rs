// Copyright 2019 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Lookups of unicode properties using minimal perfect hashing.

use super::perfect_hash::mph_lookup;
use super::tables::*;

/// Look up the canonical combining class for a codepoint.
///
/// The value returned is as defined in the Unicode Character Database.
pub(super) fn canonical_combining_class<const MODERN: bool>(c: char) -> u8 {
    #[cfg(feature = "document-nfc")]
    if MODERN {
        use super::unicode18::*;
        return mph_lookup(
            c.into(),
            CANONICAL_COMBINING_CLASS_SALT,
            CANONICAL_COMBINING_CLASS_KV,
            u8_lookup_fk,
            u8_lookup_fv,
            0,
        );
    }
    mph_lookup(
        c.into(),
        CANONICAL_COMBINING_CLASS_SALT,
        CANONICAL_COMBINING_CLASS_KV,
        u8_lookup_fk,
        u8_lookup_fv,
        0,
    )
}

pub(super) fn composition_table<const MODERN: bool>(c1: char, c2: char) -> Option<char> {
    #[cfg(feature = "document-nfc")]
    if MODERN {
        use super::unicode18::*;
        return compose_pair(
            c1,
            c2,
            COMPOSITION_TABLE_SALT,
            COMPOSITION_TABLE_KV,
            composition_table_astral,
        );
    }
    compose_pair(
        c1,
        c2,
        COMPOSITION_TABLE_SALT,
        COMPOSITION_TABLE_KV,
        composition_table_astral,
    )
}

#[inline]
fn compose_pair(
    c1: char,
    c2: char,
    salt: &[u16],
    entries: &[(u32, char)],
    astral: impl FnOnce(char, char) -> Option<char>,
) -> Option<char> {
    if c1 < '\u{10000}' && c2 < '\u{10000}' {
        mph_lookup(
            (c1 as u32) << 16 | (c2 as u32),
            salt,
            entries,
            pair_lookup_fk,
            pair_lookup_fv_opt,
            None,
        )
    } else {
        astral(c1, c2)
    }
}

pub(super) fn canonical_fully_decomposed<const MODERN: bool>(c: char) -> Option<&'static [char]> {
    #[cfg(feature = "document-nfc")]
    if MODERN {
        use super::unicode18::*;
        return mph_lookup(
            c.into(),
            CANONICAL_DECOMPOSED_SALT,
            CANONICAL_DECOMPOSED_KV,
            pair_lookup_fk,
            pair_lookup_fv_opt,
            None,
        )
        .map(|(start, len)| &CANONICAL_DECOMPOSED_CHARS[start as usize..][..len as usize]);
    }
    mph_lookup(
        c.into(),
        CANONICAL_DECOMPOSED_SALT,
        CANONICAL_DECOMPOSED_KV,
        pair_lookup_fk,
        pair_lookup_fv_opt,
        None,
    )
}

#[cfg(feature = "ai-image-search")]
pub(super) fn compatibility_fully_decomposed(c: char) -> Option<&'static [char]> {
    mph_lookup(
        c.into(),
        COMPATIBILITY_DECOMPOSED_SALT,
        COMPATIBILITY_DECOMPOSED_KV,
        pair_lookup_fk,
        pair_lookup_fv_opt,
        None,
    )
}

/// Return whether the given character is a combining mark (`General_Category=Mark`)
#[cfg(feature = "ai-image-search")]
pub fn is_combining_mark(c: char) -> bool {
    mph_lookup(
        c.into(),
        COMBINING_MARK_SALT,
        COMBINING_MARK_KV,
        bool_lookup_fk,
        bool_lookup_fv,
        false,
    )
}

/// Extract the key in a 24 bit key and 8 bit value packed in a u32.
#[inline]
fn u8_lookup_fk(kv: u32) -> u32 {
    kv >> 8
}

/// Extract the value in a 24 bit key and 8 bit value packed in a u32.
#[inline]
fn u8_lookup_fv(kv: u32) -> u8 {
    (kv & 0xff) as u8
}

/// Extract the key for a boolean lookup.
#[inline]
#[cfg(feature = "ai-image-search")]
fn bool_lookup_fk(kv: u32) -> u32 {
    kv
}

/// Extract the value for a boolean lookup.
#[inline]
#[cfg(feature = "ai-image-search")]
fn bool_lookup_fv(_kv: u32) -> bool {
    true
}

/// Extract the key in a pair.
#[inline]
fn pair_lookup_fk<T>(kv: (u32, T)) -> u32 {
    kv.0
}

/// Extract the value in a pair, returning an option.
#[inline]
fn pair_lookup_fv_opt<T>(kv: (u32, T)) -> Option<T> {
    Some(kv.1)
}
