use crate::regex_engine::syntax::hir;
use std::{
    string::{String, ToString},
    vec::Vec,
};
type Range = &'static [(char, char)];
#[derive(Debug)]
pub enum Error {
    PropertyNotFound,
    PropertyValueNotFound,
    PerlClassNotFound,
}
#[derive(Debug)]
pub struct CaseFoldError(());
impl std::error::Error for CaseFoldError {}
impl core::fmt::Display for CaseFoldError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Unicode-aware case folding is not available \
             (probably because the unicode-case feature is not enabled)"
        )
    }
}
#[derive(Debug)]
pub struct UnicodeWordError(());
impl std::error::Error for UnicodeWordError {}
impl core::fmt::Display for UnicodeWordError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Unicode-aware \\w class is not available \
             (probably because the unicode-perl feature is not enabled)"
        )
    }
}
#[derive(Debug)]
pub struct SimpleCaseFolder {
    table: &'static [(char, &'static [char])],
    last: Option<char>,
    next: usize,
}
impl SimpleCaseFolder {
    pub fn new() -> Result<SimpleCaseFolder, CaseFoldError> {
        {
            Ok (SimpleCaseFolder { table : crate::regex_engine::syntax:: unicode_tables :: case_folding_simple :: CASE_FOLDING_SIMPLE , last : None , next : 0 })
        }
    }
    pub fn mapping(&mut self, c: char) -> &'static [char] {
        if let Some(last) = self.last {
            assert!(
                last < c,
                "got codepoint U+{:X} which occurs before \
                 last codepoint U+{:X}",
                u32::from(c),
                u32::from(last),
            );
        }
        self.last = Some(c);
        if self.next >= self.table.len() {
            return &[];
        }
        let (k, v) = self.table[self.next];
        if k == c {
            self.next += 1;
            return v;
        }
        match self.get(c) {
            Err(i) => {
                self.next = i;
                &[]
            }
            Ok(i) => {
                assert!(i > self.next);
                self.next = i + 1;
                self.table[i].1
            }
        }
    }
    pub fn overlaps(&self, start: char, end: char) -> bool {
        use core::cmp::Ordering;
        assert!(start <= end);
        self.table
            .binary_search_by(|&(c, _)| {
                if start <= c && c <= end {
                    Ordering::Equal
                } else if c > end {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            })
            .is_ok()
    }
    fn get(&self, c: char) -> Result<usize, usize> {
        self.table.binary_search_by_key(&c, |&(c1, _)| c1)
    }
}
#[derive(Debug)]
pub enum ClassQuery<'a> {
    OneLetter(char),
    Binary(&'a str),
    ByValue {
        property_name: &'a str,
        property_value: &'a str,
    },
}
impl<'a> ClassQuery<'a> {
    fn canonicalize(&self) -> Result<CanonicalClassQuery, Error> {
        match *self {
            ClassQuery::OneLetter(c) => self.canonical_binary(&c.to_string()),
            ClassQuery::Binary(name) => self.canonical_binary(name),
            ClassQuery::ByValue {
                property_name,
                property_value,
            } => {
                let property_name = symbolic_name_normalize(property_name);
                let property_value = symbolic_name_normalize(property_value);
                let canon_name = match canonical_prop(&property_name)? {
                    None => return Err(Error::PropertyNotFound),
                    Some(canon_name) => canon_name,
                };
                Ok(match canon_name {
                    "General_Category" => {
                        let canon = match canonical_gencat(&property_value)? {
                            None => return Err(Error::PropertyValueNotFound),
                            Some(canon) => canon,
                        };
                        CanonicalClassQuery::GeneralCategory(canon)
                    }
                    "Script" => {
                        let canon = match canonical_script(&property_value)? {
                            None => return Err(Error::PropertyValueNotFound),
                            Some(canon) => canon,
                        };
                        CanonicalClassQuery::Script(canon)
                    }
                    _ => {
                        let vals = match property_values(canon_name)? {
                            None => return Err(Error::PropertyValueNotFound),
                            Some(vals) => vals,
                        };
                        let canon_val = match canonical_value(vals, &property_value) {
                            None => return Err(Error::PropertyValueNotFound),
                            Some(canon_val) => canon_val,
                        };
                        CanonicalClassQuery::ByValue {
                            property_name: canon_name,
                            property_value: canon_val,
                        }
                    }
                })
            }
        }
    }
    fn canonical_binary(&self, name: &str) -> Result<CanonicalClassQuery, Error> {
        let norm = symbolic_name_normalize(name);
        if norm != "cf" && norm != "sc" && norm != "lc" {
            if let Some(canon) = canonical_prop(&norm)? {
                return Ok(CanonicalClassQuery::Binary(canon));
            }
        }
        if let Some(canon) = canonical_gencat(&norm)? {
            return Ok(CanonicalClassQuery::GeneralCategory(canon));
        }
        if let Some(canon) = canonical_script(&norm)? {
            return Ok(CanonicalClassQuery::Script(canon));
        }
        Err(Error::PropertyNotFound)
    }
}
#[derive(Debug, Eq, PartialEq)]
enum CanonicalClassQuery {
    Binary(&'static str),
    GeneralCategory(&'static str),
    Script(&'static str),
    ByValue {
        property_name: &'static str,
        property_value: &'static str,
    },
}
pub fn class(query: ClassQuery<'_>) -> Result<hir::ClassUnicode, Error> {
    use self::CanonicalClassQuery::*;
    match query.canonicalize()? {
        Binary(name) => bool_property(name),
        GeneralCategory(name) => gencat(name),
        Script(name) => script(name),
        ByValue {
            property_name: "Age",
            property_value,
        } => {
            let mut class = hir::ClassUnicode::empty();
            for set in ages(property_value)? {
                class.union(&hir_class(set));
            }
            Ok(class)
        }
        ByValue {
            property_name: "Script_Extensions",
            property_value,
        } => script_extension(property_value),
        ByValue {
            property_name: "Grapheme_Cluster_Break",
            property_value,
        } => gcb(property_value),
        ByValue {
            property_name: "Sentence_Break",
            property_value,
        } => sb(property_value),
        ByValue {
            property_name: "Word_Break",
            property_value,
        } => wb(property_value),
        _ => Err(Error::PropertyNotFound),
    }
}
pub fn perl_word() -> Result<hir::ClassUnicode, Error> {
    fn imp() -> Result<hir::ClassUnicode, Error> {
        use crate::regex_engine::syntax::unicode_tables::perl_word::PERL_WORD;
        Ok(hir_class(PERL_WORD))
    }
    imp()
}
pub fn perl_space() -> Result<hir::ClassUnicode, Error> {
    fn imp() -> Result<hir::ClassUnicode, Error> {
        use crate::regex_engine::syntax::unicode_tables::property_bool::WHITE_SPACE;
        Ok(hir_class(WHITE_SPACE))
    }
    imp()
}
pub fn perl_digit() -> Result<hir::ClassUnicode, Error> {
    fn imp() -> Result<hir::ClassUnicode, Error> {
        use crate::regex_engine::syntax::unicode_tables::general_category::DECIMAL_NUMBER;
        Ok(hir_class(DECIMAL_NUMBER))
    }
    imp()
}
pub fn hir_class(ranges: &[(char, char)]) -> hir::ClassUnicode {
    let hir_ranges: Vec<hir::ClassUnicodeRange> = ranges
        .iter()
        .map(|&(s, e)| hir::ClassUnicodeRange::new(s, e))
        .collect();
    hir::ClassUnicode::new(hir_ranges)
}
pub fn is_word_character(c: char) -> Result<bool, UnicodeWordError> {
    fn imp(c: char) -> Result<bool, UnicodeWordError> {
        use crate::regex_engine::syntax::{is_word_byte, unicode_tables::perl_word::PERL_WORD};
        if u8::try_from(c).map_or(false, is_word_byte) {
            return Ok(true);
        }
        Ok(PERL_WORD
            .binary_search_by(|&(start, end)| {
                use core::cmp::Ordering;
                if start <= c && c <= end {
                    Ordering::Equal
                } else if start > c {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            })
            .is_ok())
    }
    imp(c)
}
type PropertyValues = &'static [(&'static str, &'static str)];
fn canonical_gencat(normalized_value: &str) -> Result<Option<&'static str>, Error> {
    Ok(match normalized_value {
        "any" => Some("Any"),
        "assigned" => Some("Assigned"),
        "ascii" => Some("ASCII"),
        _ => {
            let gencats = property_values("General_Category")?.unwrap();
            canonical_value(gencats, normalized_value)
        }
    })
}
fn canonical_script(normalized_value: &str) -> Result<Option<&'static str>, Error> {
    let scripts = property_values("Script")?.unwrap();
    Ok(canonical_value(scripts, normalized_value))
}
fn canonical_prop(normalized_name: &str) -> Result<Option<&'static str>, Error> {
    fn imp(name: &str) -> Result<Option<&'static str>, Error> {
        use crate::regex_engine::syntax::unicode_tables::property_names::PROPERTY_NAMES;
        Ok(PROPERTY_NAMES
            .binary_search_by_key(&name, |&(n, _)| n)
            .ok()
            .map(|i| PROPERTY_NAMES[i].1))
    }
    imp(normalized_name)
}
fn canonical_value(vals: PropertyValues, normalized_value: &str) -> Option<&'static str> {
    vals.binary_search_by_key(&normalized_value, |&(n, _)| n)
        .ok()
        .map(|i| vals[i].1)
}
fn property_values(canonical_property_name: &'static str) -> Result<Option<PropertyValues>, Error> {
    fn imp(name: &'static str) -> Result<Option<PropertyValues>, Error> {
        use crate::regex_engine::syntax::unicode_tables::property_values::PROPERTY_VALUES;
        Ok(PROPERTY_VALUES
            .binary_search_by_key(&name, |&(n, _)| n)
            .ok()
            .map(|i| PROPERTY_VALUES[i].1))
    }
    imp(canonical_property_name)
}
fn property_set(
    name_map: &'static [(&'static str, Range)],
    canonical: &'static str,
) -> Option<Range> {
    name_map
        .binary_search_by_key(&canonical, |x| x.0)
        .ok()
        .map(|i| name_map[i].1)
}
fn ages(canonical_age: &str) -> Result<impl Iterator<Item = Range>, Error> {
    fn imp(canonical_age: &str) -> Result<impl Iterator<Item = Range>, Error> {
        use crate::regex_engine::syntax::unicode_tables::age;
        const AGES: &[(&str, Range)] = &[
            ("V1_1", age::V1_1),
            ("V2_0", age::V2_0),
            ("V2_1", age::V2_1),
            ("V3_0", age::V3_0),
            ("V3_1", age::V3_1),
            ("V3_2", age::V3_2),
            ("V4_0", age::V4_0),
            ("V4_1", age::V4_1),
            ("V5_0", age::V5_0),
            ("V5_1", age::V5_1),
            ("V5_2", age::V5_2),
            ("V6_0", age::V6_0),
            ("V6_1", age::V6_1),
            ("V6_2", age::V6_2),
            ("V6_3", age::V6_3),
            ("V7_0", age::V7_0),
            ("V8_0", age::V8_0),
            ("V9_0", age::V9_0),
            ("V10_0", age::V10_0),
            ("V11_0", age::V11_0),
            ("V12_0", age::V12_0),
            ("V12_1", age::V12_1),
            ("V13_0", age::V13_0),
            ("V14_0", age::V14_0),
            ("V15_0", age::V15_0),
            ("V15_1", age::V15_1),
            ("V16_0", age::V16_0),
        ];
        assert_eq!(AGES.len(), age::BY_NAME.len(), "ages are out of sync");
        let pos = AGES.iter().position(|&(age, _)| canonical_age == age);
        match pos {
            None => Err(Error::PropertyValueNotFound),
            Some(i) => Ok(AGES[..=i].iter().map(|&(_, classes)| classes)),
        }
    }
    imp(canonical_age)
}
fn gencat(canonical_name: &'static str) -> Result<hir::ClassUnicode, Error> {
    fn imp(name: &'static str) -> Result<hir::ClassUnicode, Error> {
        use crate::regex_engine::syntax::unicode_tables::general_category::BY_NAME;
        match name {
            "ASCII" => Ok(hir_class(&[('\0', '\x7F')])),
            "Any" => Ok(hir_class(&[('\0', '\u{10FFFF}')])),
            "Assigned" => {
                let mut cls = gencat("Unassigned")?;
                cls.negate();
                Ok(cls)
            }
            name => property_set(BY_NAME, name)
                .map(hir_class)
                .ok_or(Error::PropertyValueNotFound),
        }
    }
    match canonical_name {
        "Decimal_Number" => perl_digit(),
        name => imp(name),
    }
}
fn script(canonical_name: &'static str) -> Result<hir::ClassUnicode, Error> {
    fn imp(name: &'static str) -> Result<hir::ClassUnicode, Error> {
        use crate::regex_engine::syntax::unicode_tables::script::BY_NAME;
        property_set(BY_NAME, name)
            .map(hir_class)
            .ok_or(Error::PropertyValueNotFound)
    }
    imp(canonical_name)
}
fn script_extension(canonical_name: &'static str) -> Result<hir::ClassUnicode, Error> {
    fn imp(name: &'static str) -> Result<hir::ClassUnicode, Error> {
        use crate::regex_engine::syntax::unicode_tables::script_extension::BY_NAME;
        property_set(BY_NAME, name)
            .map(hir_class)
            .ok_or(Error::PropertyValueNotFound)
    }
    imp(canonical_name)
}
fn bool_property(canonical_name: &'static str) -> Result<hir::ClassUnicode, Error> {
    fn imp(name: &'static str) -> Result<hir::ClassUnicode, Error> {
        use crate::regex_engine::syntax::unicode_tables::property_bool::BY_NAME;
        property_set(BY_NAME, name)
            .map(hir_class)
            .ok_or(Error::PropertyNotFound)
    }
    match canonical_name {
        "Decimal_Number" => perl_digit(),
        "White_Space" => perl_space(),
        name => imp(name),
    }
}
fn gcb(canonical_name: &'static str) -> Result<hir::ClassUnicode, Error> {
    fn imp(name: &'static str) -> Result<hir::ClassUnicode, Error> {
        use crate::regex_engine::syntax::unicode_tables::grapheme_cluster_break::BY_NAME;
        property_set(BY_NAME, name)
            .map(hir_class)
            .ok_or(Error::PropertyValueNotFound)
    }
    imp(canonical_name)
}
fn wb(canonical_name: &'static str) -> Result<hir::ClassUnicode, Error> {
    fn imp(name: &'static str) -> Result<hir::ClassUnicode, Error> {
        use crate::regex_engine::syntax::unicode_tables::word_break::BY_NAME;
        property_set(BY_NAME, name)
            .map(hir_class)
            .ok_or(Error::PropertyValueNotFound)
    }
    imp(canonical_name)
}
fn sb(canonical_name: &'static str) -> Result<hir::ClassUnicode, Error> {
    fn imp(name: &'static str) -> Result<hir::ClassUnicode, Error> {
        use crate::regex_engine::syntax::unicode_tables::sentence_break::BY_NAME;
        property_set(BY_NAME, name)
            .map(hir_class)
            .ok_or(Error::PropertyValueNotFound)
    }
    imp(canonical_name)
}
fn symbolic_name_normalize(x: &str) -> String {
    let mut tmp = x.as_bytes().to_vec();
    let len = symbolic_name_normalize_bytes(&mut tmp).len();
    tmp.truncate(len);
    String::from_utf8(tmp).unwrap()
}
fn symbolic_name_normalize_bytes(slice: &mut [u8]) -> &mut [u8] {
    let mut start = 0;
    let mut starts_with_is = false;
    if slice.len() >= 2 {
        starts_with_is = slice[0..2] == b"is"[..]
            || slice[0..2] == b"IS"[..]
            || slice[0..2] == b"iS"[..]
            || slice[0..2] == b"Is"[..];
        if starts_with_is {
            start = 2;
        }
    }
    let mut next_write = 0;
    for i in start..slice.len() {
        let b = slice[i];
        if b == b' ' || b == b'_' || b == b'-' {
            continue;
        } else if b'A' <= b && b <= b'Z' {
            slice[next_write] = b + (b'a' - b'A');
            next_write += 1;
        } else if b <= 0x7F {
            slice[next_write] = b;
            next_write += 1;
        }
    }
    if starts_with_is && next_write == 1 && slice[0] == b'c' {
        slice[0] = b'i';
        slice[1] = b's';
        slice[2] = b'c';
        next_write = 3;
    }
    &mut slice[..next_write]
}
