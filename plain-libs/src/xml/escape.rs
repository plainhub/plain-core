use crate::memchr::{memchr, memchr2_iter, memchr3};
use std::borrow::Cow;
use std::fmt::{self, Write};
use std::num::ParseIntError;
use std::ops::Range;
use std::slice::Iter;
#[derive(Clone, Debug, PartialEq)]
pub enum ParseCharRefError {
    UnexpectedSign,
    InvalidNumber(ParseIntError),
    InvalidCodepoint(u32),
    IllegalCharacter(u32),
}
impl std::fmt::Display for ParseCharRefError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::UnexpectedSign => f.write_str("unexpected number sign"),
            Self::InvalidNumber(e) => e.fmt(f),
            Self::InvalidCodepoint(n) => write!(f, "`{}` is not a valid codepoint", n),
            Self::IllegalCharacter(n) => write!(f, "0x{:x} character is not permitted in XML", n),
        }
    }
}
impl std::error::Error for ParseCharRefError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidNumber(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum EscapeError {
    UnrecognizedEntity(Range<usize>, String),
    UnterminatedEntity(Range<usize>),
    InvalidCharRef(ParseCharRefError),
    TooManyNestedEntities,
}
impl std::fmt::Display for EscapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::UnrecognizedEntity(rge, res) => {
                write!(f, "at {:?}: unrecognized entity `{}`", rge, res)
            }
            Self::UnterminatedEntity(e) => write!(
                f,
                "Error while escaping character at range {:?}: Cannot find ';' after '&'",
                e
            ),
            Self::InvalidCharRef(e) => {
                write!(f, "invalid character reference: {}", e)
            }
            Self::TooManyNestedEntities => {
                f.write_str("too many nested entities in an attribute value")
            }
        }
    }
}
impl std::error::Error for EscapeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidCharRef(e) => Some(e),
            _ => None,
        }
    }
}
pub fn escape<'a>(raw: impl Into<Cow<'a, str>>) -> Cow<'a, str> {
    _escape(raw, |ch| {
        matches!(ch, b'<' | b'>' | b'&' | b'\'' | b'\"' | b'\r')
    })
}
pub fn partial_escape<'a>(raw: impl Into<Cow<'a, str>>) -> Cow<'a, str> {
    _escape(raw, |ch| matches!(ch, b'<' | b'>' | b'&' | b'\r'))
}
pub fn minimal_escape<'a>(raw: impl Into<Cow<'a, str>>) -> Cow<'a, str> {
    _escape(raw, |ch| matches!(ch, b'<' | b'&' | b'\r'))
}
pub(crate) fn escape_attribute<'a>(raw: impl Into<Cow<'a, str>>) -> Cow<'a, str> {
    _escape(raw, |ch| {
        matches!(
            ch,
            b'<' | b'>' | b'&' | b'\'' | b'\"' | b'\r' | b'\n' | b'\t'
        )
    })
}
pub(crate) fn escape_char<W>(writer: &mut W, value: &str, from: usize, to: usize) -> fmt::Result
where
    W: fmt::Write,
{
    writer.write_str(&value[from..to])?;
    match value.as_bytes()[to] {
        b'<' => writer.write_str("&lt;")?,
        b'>' => writer.write_str("&gt;")?,
        b'\'' => writer.write_str("&apos;")?,
        b'&' => writer.write_str("&amp;")?,
        b'"' => writer.write_str("&quot;")?,
        b'\t' => writer.write_str("&#9;")?,
        b'\n' => writer.write_str("&#10;")?,
        b'\r' => writer.write_str("&#13;")?,
        b' ' => writer.write_str("&#32;")?,
        _ => unreachable!("Only '<', '>','\', '&', '\"', '\\t', '\\r', '\\n', and ' ' are escaped"),
    }
    Ok(())
}
fn _escape<'a, F: Fn(u8) -> bool>(raw: impl Into<Cow<'a, str>>, escape_chars: F) -> Cow<'a, str> {
    let raw = raw.into();
    let bytes = raw.as_bytes();
    let mut escaped = None;
    let mut iter = bytes.iter();
    let mut pos = 0;
    while let Some(i) = iter.position(|&b| escape_chars(b)) {
        if escaped.is_none() {
            escaped = Some(String::with_capacity(raw.len()));
        }
        let escaped = escaped.as_mut().expect("initialized");
        let new_pos = pos + i;
        escape_char(escaped, &raw, pos, new_pos).unwrap();
        pos = new_pos + 1;
    }
    if let Some(mut escaped) = escaped {
        if let Some(raw) = raw.get(pos..) {
            escaped.write_str(raw).unwrap();
        }
        Cow::Owned(escaped)
    } else {
        raw
    }
}
pub fn unescape(raw: &str) -> Result<Cow<'_, str>, EscapeError> {
    unescape_with(raw, resolve_predefined_entity)
}
pub fn unescape_with<'input, 'entity, F>(
    raw: &'input str,
    mut resolve_entity: F,
) -> Result<Cow<'input, str>, EscapeError>
where
    F: FnMut(&str) -> Option<&'entity str>,
{
    let bytes = raw.as_bytes();
    let mut unescaped = None;
    let mut last_end = 0;
    let mut iter = memchr2_iter(b'&', b';', bytes);
    while let Some(start) = iter.by_ref().find(|p| bytes[*p] == b'&') {
        match iter.next() {
            Some(end) if bytes[end] == b';' => {
                if unescaped.is_none() {
                    unescaped = Some(String::with_capacity(raw.len()));
                }
                let unescaped = unescaped.as_mut().expect("initialized");
                unescaped.push_str(&raw[last_end..start]);
                let pat = &raw[start + 1..end];
                if let Some(entity) = pat.strip_prefix('#') {
                    let codepoint = parse_number(entity).map_err(EscapeError::InvalidCharRef)?;
                    unescaped.push_str(codepoint.encode_utf8(&mut [0u8; 4]));
                } else if let Some(value) = resolve_entity(pat) {
                    unescaped.push_str(value);
                } else {
                    return Err(EscapeError::UnrecognizedEntity(
                        start + 1..end,
                        pat.to_string(),
                    ));
                }
                last_end = end + 1;
            }
            _ => return Err(EscapeError::UnterminatedEntity(start..raw.len())),
        }
    }
    if let Some(mut unescaped) = unescaped {
        if let Some(raw) = raw.get(last_end..) {
            unescaped.push_str(raw);
        }
        Ok(Cow::Owned(unescaped))
    } else {
        Ok(Cow::Borrowed(raw))
    }
}
pub(crate) fn normalize_xml11_eols<'input>(text: &'input str) -> Cow<'input, str> {
    let bytes = text.as_bytes();
    if let Some(i) = memchr3(b'\r', 0xC2, 0xE2, bytes) {
        let mut normalized = String::with_capacity(text.len());
        normalized.push_str(&text[0..i]);
        let mut pos = normalize_xml11_eol_step(&mut normalized, text, i, '\n');
        while let Some(i) = memchr3(b'\r', 0xC2, 0xE2, &bytes[pos..]) {
            let index = pos + i;
            normalized.push_str(&text[pos..index]);
            pos = normalize_xml11_eol_step(&mut normalized, text, index, '\n');
        }
        if let Some(rest) = text.get(pos..) {
            normalized.push_str(rest);
        }
        return normalized.into();
    }
    Cow::Borrowed(text)
}
fn normalize_xml11_eol_step(normalized: &mut String, text: &str, index: usize, ch: char) -> usize {
    let input = text.as_bytes();
    match input[index] {
        b'\r' => {
            if index + 1 < input.len() {
                let next = input[index + 1];
                if next == b'\n' {
                    normalized.push(ch);
                    return index + 2;
                }
                if next == 0xC2 {
                    if index + 2 < input.len() && input[index + 2] == 0x85 {
                        normalized.push(ch);
                    } else {
                        normalized.push(ch);
                        normalized.push_str(&text[index + 1..index + 3]);
                    }
                    return index + 3;
                }
            }
            normalized.push(ch);
            index + 1
        }
        b'\n' => {
            normalized.push(ch);
            index + 1
        }
        0xC2 => {
            if index + 1 < input.len() && input[index + 1] == 0x85 {
                normalized.push(ch);
            } else {
                normalized.push_str(&text[index..index + 2]);
            }
            index + 2
        }
        0xE2 => {
            if index + 2 < input.len() && input[index + 1] == 0x80 && input[index + 2] == 0xA8 {
                normalized.push(ch);
            } else {
                normalized.push_str(&text[index..index + 3]);
            }
            index + 3
        }
        x => unreachable!(
            "at {}: expected ''\\n', '\\r', '\\xC2', or '\\xE2', found '{}' / {} / `0x{:X}`",
            index, x as char, x, x
        ),
    }
}
pub(crate) fn normalize_xml10_eols<'input>(text: &'input str) -> Cow<'input, str> {
    let bytes = text.as_bytes();
    if let Some(i) = memchr(b'\r', bytes) {
        let mut normalized = String::with_capacity(text.len());
        normalized.push_str(&text[0..i]);
        let mut pos = normalize_xml10_eol_step(&mut normalized, text, i, '\n');
        while let Some(i) = memchr(b'\r', &bytes[pos..]) {
            let index = pos + i;
            normalized.push_str(&text[pos..index]);
            pos = normalize_xml10_eol_step(&mut normalized, text, index, '\n');
        }
        if let Some(rest) = text.get(pos..) {
            normalized.push_str(rest);
        }
        return normalized.into();
    }
    Cow::Borrowed(text)
}
fn normalize_xml10_eol_step(normalized: &mut String, text: &str, index: usize, ch: char) -> usize {
    let input = text.as_bytes();
    match input[index] {
        b'\r' => {
            normalized.push(ch);
            if index + 1 < input.len() && input[index + 1] == b'\n' {
                return index + 2;
            }
            index + 1
        }
        b'\n' => {
            normalized.push(ch);
            index + 1
        }
        x => unreachable!(
            "at {}: expected ''\\n' or '\\r', found '{}' / {} / `0x{:X}`",
            index, x as char, x, x
        ),
    }
}
pub(crate) fn normalize_xml10_attribute_value<'input, 'entity, F>(
    value: &'input str,
    depth: usize,
    resolve_entity: F,
) -> Result<Cow<'input, str>, EscapeError>
where
    F: FnMut(&str) -> Option<&'entity str>,
{
    normalize_attribute_value(
        value,
        depth,
        is_xml10_normalization_char,
        normalize_xml10_eol_step,
        resolve_entity,
    )
}
const fn is_xml10_normalization_char(b: &u8) -> bool {
    matches!(*b, b'\t' | b'\r' | b'\n' | b'&')
}
pub(crate) fn normalize_xml11_attribute_value<'input, 'entity, F>(
    value: &'input str,
    depth: usize,
    resolve_entity: F,
) -> Result<Cow<'input, str>, EscapeError>
where
    F: FnMut(&str) -> Option<&'entity str>,
{
    normalize_attribute_value(
        value,
        depth,
        is_xml11_normalization_char,
        normalize_xml11_eol_step,
        resolve_entity,
    )
}
const fn is_xml11_normalization_char(b: &u8) -> bool {
    matches!(*b, b'\t' | b'\r' | b'\n' | 0xC2 | 0xE2 | b'&')
}
pub fn normalize_attribute_value<'input, 'entity, C, E, F>(
    value: &'input str,
    depth: usize,
    is_normalization_char: C,
    normalize_eol_step: E,
    mut resolve_entity: F,
) -> Result<Cow<'input, str>, EscapeError>
where
    C: Fn(&u8) -> bool,
    E: Fn(&mut String, &str, usize, char) -> usize,
    F: FnMut(&str) -> Option<&'entity str>,
{
    let mut iter = value.as_bytes().iter();
    if let Some(i) = iter.position(&is_normalization_char) {
        let mut normalized = String::with_capacity(value.len());
        let pos = normalize_attr_step(
            &mut normalized,
            &mut iter,
            value,
            0,
            i,
            depth,
            &is_normalization_char,
            &normalize_eol_step,
            &mut resolve_entity,
        )?;
        normalize_attr_steps(
            &mut normalized,
            &mut iter,
            value,
            pos,
            depth,
            &is_normalization_char,
            &normalize_eol_step,
            &mut resolve_entity,
        )?;
        return Ok(normalized.into());
    }
    Ok(Cow::Borrowed(value))
}
fn normalize_attr_steps<'entity, C, E, F>(
    normalized: &mut String,
    iter: &mut Iter<u8>,
    input: &str,
    mut pos: usize,
    depth: usize,
    is_normalization_char: &C,
    normalize_eol_step: &E,
    resolve_entity: &mut F,
) -> Result<(), EscapeError>
where
    C: Fn(&u8) -> bool,
    E: Fn(&mut String, &str, usize, char) -> usize,
    F: FnMut(&str) -> Option<&'entity str>,
{
    while let Some(i) = iter.position(is_normalization_char) {
        pos = normalize_attr_step(
            normalized,
            iter,
            input,
            pos,
            pos + i,
            depth,
            is_normalization_char,
            normalize_eol_step,
            resolve_entity,
        )?;
    }
    if let Some(rest) = input.get(pos..) {
        normalized.push_str(rest);
    }
    Ok(())
}
fn normalize_attr_step<'entity, C, E, F>(
    normalized: &mut String,
    iter: &mut Iter<u8>,
    input: &str,
    last_pos: usize,
    index: usize,
    depth: usize,
    is_normalization_char: &C,
    normalize_eol_step: &E,
    resolve_entity: &mut F,
) -> Result<usize, EscapeError>
where
    C: Fn(&u8) -> bool,
    E: Fn(&mut String, &str, usize, char) -> usize,
    F: FnMut(&str) -> Option<&'entity str>,
{
    if depth == 0 {
        return Err(EscapeError::TooManyNestedEntities);
    }
    normalized.push_str(&input[last_pos..index]);
    match input.as_bytes()[index] {
        b'&' => {
            let start = index + 1;
            let end = start
                + match iter.position(|&b| b == b';') {
                    Some(end) => end,
                    None => return Err(EscapeError::UnterminatedEntity(index..input.len())),
                };
            let pat = &input[start..end];
            if let Some(entity) = pat.strip_prefix('#') {
                let codepoint = parse_number(entity).map_err(EscapeError::InvalidCharRef)?;
                normalized.push_str(codepoint.encode_utf8(&mut [0u8; 4]));
            } else if pat == "amp" {
                normalized.push('&');
            } else if let Some(value) = resolve_entity(pat) {
                normalize_attr_steps(
                    normalized,
                    &mut value.as_bytes().iter(),
                    value,
                    0,
                    depth.saturating_sub(1),
                    is_normalization_char,
                    normalize_eol_step,
                    resolve_entity,
                )?;
            } else {
                return Err(EscapeError::UnrecognizedEntity(start..end, pat.to_string()));
            }
            Ok(end + 1)
        }
        b'\t' => {
            normalized.push(' ');
            Ok(index + 1)
        }
        _ => {
            let pos = normalize_eol_step(normalized, input, index, ' ');
            for _ in 0..pos - index - 1 {
                iter.next();
            }
            Ok(pos)
        }
    }
}
#[inline]
pub const fn resolve_predefined_entity(entity: &str) -> Option<&'static str> {
    resolve_xml_entity(entity)
}
pub const fn resolve_xml_entity(entity: &str) -> Option<&'static str> {
    let s = match entity.as_bytes() {
        b"lt" => "<",
        b"gt" => ">",
        b"amp" => "&",
        b"apos" => "'",
        b"quot" => "\"",
        _ => return None,
    };
    Some(s)
}
pub(crate) fn parse_number(num: &str) -> Result<char, ParseCharRefError> {
    let code = if let Some(hex) = num.strip_prefix('x') {
        from_str_radix(hex, 16)?
    } else {
        from_str_radix(num, 10)?
    };
    if code == 0 {
        return Err(ParseCharRefError::IllegalCharacter(code));
    }
    match std::char::from_u32(code) {
        Some(c) => Ok(c),
        None => Err(ParseCharRefError::InvalidCodepoint(code)),
    }
}
#[inline]
fn from_str_radix(src: &str, radix: u32) -> Result<u32, ParseCharRefError> {
    match src.as_bytes().first().copied() {
        Some(b'+') | Some(b'-') => Err(ParseCharRefError::UnexpectedSign),
        _ => u32::from_str_radix(src, radix).map_err(ParseCharRefError::InvalidNumber),
    }
}
