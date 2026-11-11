use crate::xml::XmlVersion;
use crate::xml::errors::Result as XmlResult;
use crate::xml::escape::{escape_attribute, resolve_predefined_entity};
use crate::xml::name::{LocalName, Namespace, NamespaceResolver, QName};
use crate::xml::utils::is_whitespace;
use std::collections::HashSet;
use std::fmt::{self, Debug, Display, Formatter};
use std::hash::{BuildHasherDefault, DefaultHasher, Hasher};
use std::iter::FusedIterator;
use std::{borrow::Cow, ops::Range};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute<'a> {
    pub key: QName<'a>,
    pub value: Cow<'a, str>,
}
impl<'a> Attribute<'a> {
    pub fn normalized_value(&self, version: XmlVersion) -> XmlResult<Cow<'a, str>> {
        self.normalized_value_with(version, 1, resolve_predefined_entity)
    }
    pub fn normalized_value_with<'entity>(
        &self,
        version: XmlVersion,
        depth: usize,
        resolve_entity: impl FnMut(&str) -> Option<&'entity str>,
    ) -> XmlResult<Cow<'a, str>> {
        match version.normalize_attribute_value(&self.value, depth, resolve_entity)? {
            Cow::Borrowed(_) => Ok(self.value.clone()),
            Cow::Owned(s) => Ok(s.into()),
        }
    }
    #[inline]
    pub fn as_bool(&self) -> Option<bool> {
        match self.value.as_ref() {
            "1" | "true" => Some(true),
            "0" | "false" => Some(false),
            _ => None,
        }
    }
}
impl<'a> From<(&'a str, &'a str)> for Attribute<'a> {
    fn from(val: (&'a str, &'a str)) -> Attribute<'a> {
        Attribute {
            key: QName(val.0),
            value: escape_attribute(val.1),
        }
    }
}
impl<'a> From<(&'a str, Cow<'a, str>)> for Attribute<'a> {
    fn from(val: (&'a str, Cow<'a, str>)) -> Attribute<'a> {
        Attribute {
            key: QName(val.0),
            value: escape_attribute(val.1),
        }
    }
}
impl<'a> From<Attr<&'a str>> for Attribute<'a> {
    #[inline]
    fn from(attr: Attr<&'a str>) -> Self {
        Self {
            key: attr.key(),
            value: Cow::Borrowed(attr.value()),
        }
    }
}
#[derive(Clone)]
pub struct Attributes<'a> {
    buf: &'a str,
    state: IterState,
}
impl<'a> Attributes<'a> {
    #[inline]
    pub(crate) const fn wrap(buf: &'a str, pos: usize, html: bool) -> Self {
        Self {
            buf,
            state: IterState::new(pos, html),
        }
    }
    pub const fn new(buf: &'a str, pos: usize) -> Self {
        Self::wrap(buf, pos, false)
    }
    pub const fn html(buf: &'a str, pos: usize) -> Self {
        Self::wrap(buf, pos, true)
    }
    pub fn with_checks(&mut self, val: bool) -> &mut Attributes<'a> {
        self.state.check_duplicates = val;
        self
    }
    pub fn has_nil(&mut self, resolver: &NamespaceResolver) -> bool {
        use crate::xml::name::ResolveResult::*;
        self.any(|attr| {
            if let Ok(attr) = attr {
                match resolver.resolve_attribute(attr.key) {
                    (
                        Bound(Namespace("http://www.w3.org/2001/XMLSchema-instance")),
                        LocalName("nil"),
                    ) => attr.as_bool().unwrap_or_default(),
                    _ => false,
                }
            } else {
                false
            }
        })
    }
}
impl<'a> Debug for Attributes<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Attributes")
            .field("buf", &self.buf)
            .field("state", &self.state)
            .finish()
    }
}
impl<'a> Iterator for Attributes<'a> {
    type Item = Result<Attribute<'a>, AttrError>;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.state.next(self.buf.as_bytes()) {
            None => None,
            Some(Ok(a)) => Some(Ok(a.map(|range| &self.buf[range]).into())),
            Some(Err(e)) => Some(Err(e)),
        }
    }
}
impl<'a> FusedIterator for Attributes<'a> {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttrError {
    ExpectedEq(usize),
    ExpectedValue(usize),
    UnquotedValue(usize),
    ExpectedQuote(usize, u8),
    Duplicated(usize, usize),
}
impl Display for AttrError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::ExpectedEq(pos) => write!(
                f,
                r#"position {}: attribute key must be directly followed by `=` or space"#,
                pos
            ),
            Self::ExpectedValue(pos) => write!(
                f,
                r#"position {}: `=` must be followed by an attribute value"#,
                pos
            ),
            Self::UnquotedValue(pos) => write!(
                f,
                r#"position {}: attribute value must be enclosed in `"` or `'`"#,
                pos
            ),
            Self::ExpectedQuote(pos, quote) => write!(
                f,
                r#"position {}: missing closing quote `{}` in attribute value"#,
                pos, *quote as char
            ),
            Self::Duplicated(pos1, pos2) => write!(
                f,
                r#"position {}: duplicated attribute, previous declaration at position {}"#,
                pos1, pos2
            ),
        }
    }
}
impl std::error::Error for AttrError {}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Attr<T> {
    DoubleQ(T, T),
    SingleQ(T, T),
    Unquoted(T, T),
    Empty(T),
}
impl<T> Attr<T> {
    #[inline]
    pub fn map<U, F>(self, mut f: F) -> Attr<U>
    where
        F: FnMut(T) -> U,
    {
        match self {
            Attr::DoubleQ(key, value) => Attr::DoubleQ(f(key), f(value)),
            Attr::SingleQ(key, value) => Attr::SingleQ(f(key), f(value)),
            Attr::Empty(key) => Attr::Empty(f(key)),
            Attr::Unquoted(key, value) => Attr::Unquoted(f(key), f(value)),
        }
    }
}
impl<'a> Attr<&'a str> {
    #[inline]
    pub const fn key(&self) -> QName<'a> {
        QName(match self {
            Attr::DoubleQ(key, _) => *key,
            Attr::SingleQ(key, _) => *key,
            Attr::Empty(key) => *key,
            Attr::Unquoted(key, _) => *key,
        })
    }
    #[inline]
    pub const fn value(&self) -> &'a str {
        match self {
            Attr::DoubleQ(_, value) => *value,
            Attr::SingleQ(_, value) => *value,
            Attr::Empty(_) => "",
            Attr::Unquoted(_, value) => *value,
        }
    }
}
impl<T: Debug> Debug for Attr<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Attr::DoubleQ(key, value) => f
                .debug_tuple("Attr::DoubleQ")
                .field(key)
                .field(value)
                .finish(),
            Attr::SingleQ(key, value) => f
                .debug_tuple("Attr::SingleQ")
                .field(key)
                .field(value)
                .finish(),
            Attr::Empty(key) => f.debug_tuple("Attr::Empty").field(key).finish(),
            Attr::Unquoted(key, value) => f
                .debug_tuple("Attr::Unquoted")
                .field(key)
                .field(value)
                .finish(),
        }
    }
}
impl<T> From<Attr<T>> for (T, Option<T>) {
    #[inline]
    fn from(attr: Attr<T>) -> Self {
        match attr {
            Attr::DoubleQ(key, value) => (key, Some(value)),
            Attr::SingleQ(key, value) => (key, Some(value)),
            Attr::Empty(key) => (key, None),
            Attr::Unquoted(key, value) => (key, Some(value)),
        }
    }
}
type AttrResult = Result<Attr<Range<usize>>, AttrError>;
#[derive(Clone, Copy, Debug)]
enum State {
    Done,
    Next(usize),
    SkipValue(usize),
    SkipEqValue(usize),
}
const SMALL_ATTRIBUTE_COUNT: usize = 32;
#[derive(Default)]
struct IdentityHasher(u64);
impl Hasher for IdentityHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
    #[inline]
    fn write(&mut self, _: &[u8]) {
        unreachable!("IdentityHasher only supports u64 keys")
    }
    #[inline]
    fn write_u64(&mut self, n: u64) {
        self.0 = n;
    }
}
#[inline]
fn hash_name(name: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    hasher.write(name);
    hasher.finish()
}
#[derive(Clone, Debug)]
pub(crate) struct IterState {
    state: State,
    html: bool,
    check_duplicates: bool,
    keys: Vec<Range<usize>>,
    key_hashes: Option<HashSet<u64, BuildHasherDefault<IdentityHasher>>>,
}
impl IterState {
    pub const fn new(offset: usize, html: bool) -> Self {
        Self {
            state: State::Next(offset),
            html,
            check_duplicates: true,
            keys: Vec::new(),
            key_hashes: None,
        }
    }
    fn recover(&self, slice: &[u8]) -> Option<usize> {
        match self.state {
            State::Done => None,
            State::Next(offset) if offset <= slice.len() => Some(offset),
            State::Next(_) => None,
            State::SkipValue(offset) => self.skip_value(slice, offset),
            State::SkipEqValue(offset) => self.skip_eq_value(slice, offset),
        }
    }
    #[inline]
    fn skip_value(&self, slice: &[u8], offset: usize) -> Option<usize> {
        let mut iter = (offset..).zip(slice[offset..].iter());
        match iter.find(|&(_, &b)| is_whitespace(b)) {
            Some((e, _)) => Some(e),
            None => None,
        }
    }
    #[inline]
    fn skip_eq_value(&self, slice: &[u8], offset: usize) -> Option<usize> {
        let mut iter = (offset..).zip(slice[offset..].iter());
        let quote = match iter.find(|&(_, &b)| !is_whitespace(b)) {
            Some((_, b'"')) => b'"',
            Some((_, b'\'')) => b'\'',
            Some((offset, _)) => return self.skip_value(slice, offset),
            None => return None,
        };
        match iter.find(|&(_, &b)| b == quote) {
            Some((e, b'"')) => Some(e),
            Some((e, _)) => Some(e),
            None => None,
        }
    }
    #[inline]
    fn check_for_duplicates(
        &mut self,
        slice: &[u8],
        key: Range<usize>,
    ) -> Result<Range<usize>, AttrError> {
        if self.check_duplicates {
            if self.keys.len() >= SMALL_ATTRIBUTE_COUNT {
                return self.check_for_duplicates_hashed(slice, key);
            }
            if let Some(prev) = self
                .keys
                .iter()
                .find(|r| slice[(*r).clone()] == slice[key.clone()])
            {
                return Err(AttrError::Duplicated(key.start, prev.start));
            }
            self.keys.push(key.clone());
        }
        Ok(key)
    }
    #[cold]
    fn check_for_duplicates_hashed(
        &mut self,
        slice: &[u8],
        key: Range<usize>,
    ) -> Result<Range<usize>, AttrError> {
        let keys = &self.keys;
        let key_hashes = self.key_hashes.get_or_insert_with(|| {
            let mut set = HashSet::with_capacity_and_hasher(
                keys.len() * 2,
                BuildHasherDefault::<IdentityHasher>::default(),
            );
            for r in keys {
                set.insert(hash_name(&slice[r.clone()]));
            }
            set
        });
        if !key_hashes.insert(hash_name(&slice[key.clone()])) {
            if let Some(prev) = self
                .keys
                .iter()
                .find(|r| slice[(*r).clone()] == slice[key.clone()])
            {
                return Err(AttrError::Duplicated(key.start, prev.start));
            }
        }
        self.keys.push(key.clone());
        Ok(key)
    }
    #[inline]
    fn key_only(&mut self, slice: &[u8], key: Range<usize>, offset: usize) -> Option<AttrResult> {
        Some(if self.html {
            self.check_for_duplicates(slice, key).map(Attr::Empty)
        } else {
            Err(AttrError::ExpectedEq(offset))
        })
    }
    #[inline]
    fn double_q(&mut self, key: Range<usize>, value: Range<usize>) -> Option<AttrResult> {
        self.state = State::Next(value.end + 1);
        Some(Ok(Attr::DoubleQ(key, value)))
    }
    #[inline]
    fn single_q(&mut self, key: Range<usize>, value: Range<usize>) -> Option<AttrResult> {
        self.state = State::Next(value.end + 1);
        Some(Ok(Attr::SingleQ(key, value)))
    }
    pub fn next(&mut self, slice: &[u8]) -> Option<AttrResult> {
        let mut iter = match self.recover(slice) {
            Some(offset) => (offset..).zip(slice[offset..].iter()),
            None => return None,
        };
        let start_key = match iter.find(|&(_, &b)| !is_whitespace(b)) {
            Some((s, _)) => s,
            None => {
                self.state = State::Done;
                return None;
            }
        };
        let (key, offset) = match iter.find(|&(_, &b)| b == b'=' || is_whitespace(b)) {
            Some((e, b'=')) => (start_key..e, e),
            Some((e, _)) => match iter.find(|&(_, &b)| !is_whitespace(b)) {
                Some((offset, b'=')) => (start_key..e, offset),
                Some((offset, _)) => {
                    self.state = State::Next(offset);
                    return self.key_only(slice, start_key..e, offset);
                }
                None => {
                    self.state = State::Done;
                    return self.key_only(slice, start_key..e, slice.len());
                }
            },
            None => {
                self.state = State::Done;
                let e = slice.len();
                return self.key_only(slice, start_key..e, e);
            }
        };
        let key = match self.check_for_duplicates(slice, key) {
            Err(e) => {
                self.state = State::SkipEqValue(offset);
                return Some(Err(e));
            }
            Ok(key) => key,
        };
        let (start_value, quote) = match iter.find(|&(_, &b)| !is_whitespace(b)) {
            Some((s, b'"')) => (s + 1, b'"'),
            Some((s, b'\'')) => (s + 1, b'\''),
            Some((s, _)) if self.html => {
                let end = match iter.find(|&(_, &b)| is_whitespace(b)) {
                    Some((e, _)) => e,
                    None => slice.len(),
                };
                self.state = State::Next(end);
                return Some(Ok(Attr::Unquoted(key, s..end)));
            }
            Some((s, _)) => {
                self.state = State::SkipValue(s);
                return Some(Err(AttrError::UnquotedValue(s)));
            }
            None => {
                self.state = State::Done;
                return Some(Err(AttrError::ExpectedValue(slice.len())));
            }
        };
        match iter.find(|&(_, &b)| b == quote) {
            Some((e, b'"')) => self.double_q(key, start_value..e),
            Some((e, _)) => self.single_q(key, start_value..e),
            None => {
                self.state = State::Done;
                Some(Err(AttrError::ExpectedQuote(slice.len(), quote)))
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/xml/events/attributes.rs"]
mod tests;
