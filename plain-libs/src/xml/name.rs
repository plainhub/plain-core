use crate::xml::events::attributes::Attribute;
use crate::xml::events::{BytesStart, Event};
use std::fmt::{self, Debug, Formatter};
use std::iter::FusedIterator;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamespaceError {
    UnknownPrefix(String),
    InvalidXmlPrefixBind(String),
    InvalidXmlnsPrefixBind(String),
    InvalidPrefixForXml(String),
    InvalidPrefixForXmlns(String),
    TooManyBindings(usize),
    TooDeeplyNested(usize),
}
impl fmt::Display for NamespaceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::UnknownPrefix(prefix) => {
                write!(f, "unknown namespace prefix '{}'", prefix)
            }
            Self::InvalidXmlPrefixBind(namespace) => {
                write!(
                    f,
                    "the namespace prefix 'xml' cannot be bound to '{}'",
                    namespace
                )
            }
            Self::InvalidXmlnsPrefixBind(namespace) => {
                write!(
                    f,
                    "the namespace prefix 'xmlns' cannot be bound to '{}'",
                    namespace
                )
            }
            Self::InvalidPrefixForXml(prefix) => {
                write!(
                    f,
                    "the namespace prefix '{}' cannot be bound to 'http://www.w3.org/XML/1998/namespace'",
                    prefix
                )
            }
            Self::InvalidPrefixForXmlns(prefix) => {
                write!(
                    f,
                    "the namespace prefix '{}' cannot be bound to 'http://www.w3.org/2000/xmlns/'",
                    prefix
                )
            }
            Self::TooManyBindings(limit) => {
                write!(
                    f,
                    "more than {} namespace bindings in scope; \
                     raise the limit with NamespaceResolver::set_max_namespace_bindings",
                    limit,
                )
            }
            Self::TooDeeplyNested(limit) => {
                write!(
                    f,
                    "document nests elements deeper than the supported limit of {}",
                    limit
                )
            }
        }
    }
}
impl std::error::Error for NamespaceError {}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QName<'a>(pub &'a str);
impl<'a> QName<'a> {
    #[inline(always)]
    pub const fn into_inner(self) -> &'a str {
        self.0
    }
    pub fn local_name(&self) -> LocalName<'a> {
        LocalName(self.index().map_or(self.0, |i| &self.0[i + 1..]))
    }
    pub fn prefix(&self) -> Option<Prefix<'a>> {
        self.index().map(|i| Prefix(&self.0[..i]))
    }
    pub fn decompose(&self) -> (LocalName<'a>, Option<Prefix<'a>>) {
        match self.index() {
            None => (LocalName(self.0), None),
            Some(i) => (LocalName(&self.0[i + 1..]), Some(Prefix(&self.0[..i]))),
        }
    }
    pub fn as_namespace_binding(&self) -> Option<PrefixDeclaration<'a>> {
        if self.0.starts_with("xmlns") {
            return match self.0.as_bytes().get(5) {
                None => Some(PrefixDeclaration::Default),
                Some(&b':') => Some(PrefixDeclaration::Named(&self.0[6..])),
                _ => None,
            };
        }
        None
    }
    #[inline(always)]
    fn index(&self) -> Option<usize> {
        self.0.find(':')
    }
}
impl<'a> Debug for QName<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "QName({})", self.0)
    }
}
impl<'a> AsRef<str> for QName<'a> {
    #[inline]
    fn as_ref(&self) -> &str {
        self.0
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalName<'a>(pub(crate) &'a str);
impl<'a> LocalName<'a> {
    #[inline(always)]
    pub const fn into_inner(self) -> &'a str {
        self.0
    }
}
impl<'a> Debug for LocalName<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "LocalName({})", self.0)
    }
}
impl<'a> AsRef<str> for LocalName<'a> {
    #[inline]
    fn as_ref(&self) -> &str {
        self.0
    }
}
impl<'a> From<QName<'a>> for LocalName<'a> {
    #[inline]
    fn from(name: QName<'a>) -> Self {
        Self(name.index().map_or(name.0, |i| &name.0[i + 1..]))
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Prefix<'a>(&'a str);
impl<'a> Prefix<'a> {
    #[inline(always)]
    pub const fn into_inner(self) -> &'a str {
        self.0
    }
    #[inline(always)]
    pub const fn is_xml(&self) -> bool {
        matches!(self.0.as_bytes(), b"xml")
    }
    #[inline(always)]
    pub const fn is_xmlns(&self) -> bool {
        matches!(self.0.as_bytes(), b"xmlns")
    }
}
impl<'a> Debug for Prefix<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Prefix({})", self.0)
    }
}
impl<'a> AsRef<str> for Prefix<'a> {
    #[inline]
    fn as_ref(&self) -> &str {
        self.0
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrefixDeclaration<'a> {
    Default,
    Named(&'a str),
}
impl<'a> Debug for PrefixDeclaration<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => f.write_str("PrefixDeclaration::Default"),
            Self::Named(prefix) => {
                write!(f, "PrefixDeclaration::Named({})", prefix)
            }
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Namespace<'a>(pub &'a str);
impl<'a> Namespace<'a> {
    #[inline(always)]
    pub const fn into_inner(self) -> &'a str {
        self.0
    }
}
impl<'a> Debug for Namespace<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Namespace({})", self.0)
    }
}
impl<'a> AsRef<str> for Namespace<'a> {
    #[inline]
    fn as_ref(&self) -> &str {
        self.0
    }
}
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ResolveResult<'ns> {
    Unbound,
    Bound(Namespace<'ns>),
    Unknown(String),
}
impl<'ns> Debug for ResolveResult<'ns> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Unbound => write!(f, "Unbound"),
            Self::Bound(ns) => write!(f, "Bound({:?})", ns),
            Self::Unknown(p) => write!(f, "Unknown({})", p),
        }
    }
}
impl<'ns> TryFrom<ResolveResult<'ns>> for Option<Namespace<'ns>> {
    type Error = NamespaceError;
    fn try_from(result: ResolveResult<'ns>) -> Result<Self, NamespaceError> {
        use ResolveResult::*;
        match result {
            Unbound => Ok(None),
            Bound(ns) => Ok(Some(ns)),
            Unknown(p) => Err(NamespaceError::UnknownPrefix(p)),
        }
    }
}
#[derive(Debug, Clone)]
struct NamespaceBinding {
    start: usize,
    prefix_len: usize,
    value_len: usize,
    level: u16,
}
impl NamespaceBinding {
    #[inline]
    const fn prefix<'b>(&self, ns_buffer: &'b str) -> Option<Prefix<'b>> {
        if self.prefix_len == 0 {
            None
        } else {
            let (_, prefix) = ns_buffer.split_at(self.start);
            let (prefix, _) = prefix.split_at(self.prefix_len);
            Some(Prefix(prefix))
        }
    }
    #[inline]
    const fn namespace<'ns>(&self, buffer: &'ns str) -> ResolveResult<'ns> {
        if self.value_len == 0 {
            ResolveResult::Unbound
        } else {
            let (_, ns) = buffer.split_at(self.start + self.prefix_len);
            let (ns, _) = ns.split_at(self.value_len);
            ResolveResult::Bound(Namespace(ns))
        }
    }
}
#[derive(Clone, Debug)]
pub struct NamespaceResolver {
    buffer: String,
    bindings: Vec<NamespaceBinding>,
    nesting_level: u16,
    max_namespace_bindings: usize,
}
pub const DEFAULT_MAX_NAMESPACE_BINDINGS: usize = 128;
const BUILTIN_NAMESPACE_BINDINGS: usize = 2;
const RESERVED_NAMESPACE_XML: (Prefix, Namespace) = (
    Prefix("xml"),
    Namespace("http://www.w3.org/XML/1998/namespace"),
);
const RESERVED_NAMESPACE_XMLNS: (Prefix, Namespace) =
    (Prefix("xmlns"), Namespace("http://www.w3.org/2000/xmlns/"));
impl Default for NamespaceResolver {
    fn default() -> Self {
        let mut buffer = String::new();
        let mut bindings = Vec::new();
        for ent in &[RESERVED_NAMESPACE_XML, RESERVED_NAMESPACE_XMLNS] {
            let prefix = ent.0.into_inner();
            let uri = ent.1.into_inner();
            bindings.push(NamespaceBinding {
                start: buffer.len(),
                prefix_len: prefix.len(),
                value_len: uri.len(),
                level: 0,
            });
            buffer.push_str(prefix);
            buffer.push_str(uri);
        }
        Self {
            buffer,
            bindings,
            nesting_level: 0,
            max_namespace_bindings: DEFAULT_MAX_NAMESPACE_BINDINGS,
        }
    }
}
impl NamespaceResolver {
    pub fn add(
        &mut self,
        prefix: PrefixDeclaration,
        namespace: Namespace,
    ) -> Result<(), NamespaceError> {
        let level = self.nesting_level;
        match prefix {
            PrefixDeclaration::Default => {
                if self
                    .bindings
                    .len()
                    .saturating_sub(BUILTIN_NAMESPACE_BINDINGS)
                    >= self.max_namespace_bindings
                {
                    return Err(NamespaceError::TooManyBindings(self.max_namespace_bindings));
                }
                let start = self.buffer.len();
                self.buffer.push_str(namespace.0);
                self.bindings.push(NamespaceBinding {
                    start,
                    prefix_len: 0,
                    value_len: namespace.0.len(),
                    level,
                });
            }
            PrefixDeclaration::Named("xml") => {
                if namespace != RESERVED_NAMESPACE_XML.1 {
                    return Err(NamespaceError::InvalidXmlPrefixBind(
                        namespace.0.to_string(),
                    ));
                }
            }
            PrefixDeclaration::Named("xmlns") => {
                return Err(NamespaceError::InvalidXmlnsPrefixBind(
                    namespace.0.to_string(),
                ));
            }
            PrefixDeclaration::Named(prefix) => {
                if namespace == RESERVED_NAMESPACE_XML.1 {
                    return Err(NamespaceError::InvalidPrefixForXml(prefix.to_string()));
                } else if namespace == RESERVED_NAMESPACE_XMLNS.1 {
                    return Err(NamespaceError::InvalidPrefixForXmlns(prefix.to_string()));
                }
                if self
                    .bindings
                    .len()
                    .saturating_sub(BUILTIN_NAMESPACE_BINDINGS)
                    >= self.max_namespace_bindings
                {
                    return Err(NamespaceError::TooManyBindings(self.max_namespace_bindings));
                }
                let start = self.buffer.len();
                self.buffer.push_str(prefix);
                self.buffer.push_str(namespace.0);
                self.bindings.push(NamespaceBinding {
                    start,
                    prefix_len: prefix.len(),
                    value_len: namespace.0.len(),
                    level,
                });
            }
        }
        Ok(())
    }
    pub fn push(&mut self, start: &BytesStart) -> Result<(), NamespaceError> {
        self.nesting_level = self
            .nesting_level
            .checked_add(1)
            .ok_or(NamespaceError::TooDeeplyNested(u16::MAX as usize))?;
        for a in start.attributes().with_checks(false) {
            if let Ok(Attribute { key: k, value: v }) = a {
                if let Some(prefix) = k.as_namespace_binding() {
                    self.add(prefix, Namespace(&v))?;
                }
            } else {
                break;
            }
        }
        Ok(())
    }
    #[inline]
    pub const fn max_namespace_bindings(&self) -> usize {
        self.max_namespace_bindings
    }
    #[inline]
    pub fn set_max_namespace_bindings(&mut self, limit: usize) -> &mut Self {
        self.max_namespace_bindings = limit;
        self
    }
    #[inline]
    pub fn pop(&mut self) {
        self.set_level(self.nesting_level.saturating_sub(1));
    }
    pub fn with<F, R>(&mut self, start: &BytesStart, mut action: F) -> Result<R, NamespaceError>
    where
        F: FnMut(&Self) -> R,
    {
        self.push(start)?;
        let result = action(self);
        self.pop();
        Ok(result)
    }
    pub fn set_level(&mut self, level: u16) {
        self.nesting_level = level;
        match self.bindings.iter().rposition(|n| n.level <= level) {
            None => {
                self.buffer.clear();
                self.bindings.clear();
            }
            Some(last_valid_pos) => {
                if let Some(len) = self.bindings.get(last_valid_pos + 1).map(|n| n.start) {
                    self.buffer.truncate(len);
                    self.bindings.truncate(last_valid_pos + 1);
                }
            }
        }
    }
    #[inline]
    pub fn resolve<'n>(
        &self,
        name: QName<'n>,
        use_default: bool,
    ) -> (ResolveResult<'_>, LocalName<'n>) {
        let (local_name, prefix) = name.decompose();
        (self.resolve_prefix(prefix, use_default), local_name)
    }
    #[inline]
    pub fn resolve_element<'n>(&self, name: QName<'n>) -> (ResolveResult<'_>, LocalName<'n>) {
        self.resolve(name, true)
    }
    #[inline]
    pub fn resolve_attribute<'n>(&self, name: QName<'n>) -> (ResolveResult<'_>, LocalName<'n>) {
        self.resolve(name, false)
    }
    pub fn resolve_event<'i>(&self, event: Event<'i>) -> (ResolveResult<'_>, Event<'i>) {
        use Event::*;
        match event {
            Empty(e) => (self.resolve_prefix(e.name().prefix(), true), Empty(e)),
            Start(e) => (self.resolve_prefix(e.name().prefix(), true), Start(e)),
            End(e) => (self.resolve_prefix(e.name().prefix(), true), End(e)),
            e => (ResolveResult::Unbound, e),
        }
    }
    pub fn resolve_prefix(&self, prefix: Option<Prefix>, use_default: bool) -> ResolveResult<'_> {
        let mut iter = self.bindings.iter().rev();
        match (prefix, use_default) {
            (None, false) => ResolveResult::Unbound,
            (None, true) => match iter.find(|n| n.prefix_len == 0) {
                Some(n) => n.namespace(&self.buffer),
                None => ResolveResult::Unbound,
            },
            (Some(p), _) => match iter.find(|n| n.prefix(&self.buffer) == prefix) {
                Some(n) if n.value_len != 0 => n.namespace(&self.buffer),
                _ => ResolveResult::Unknown(p.into_inner().to_string()),
            },
        }
    }
    #[inline]
    pub const fn bindings(&self) -> NamespaceBindingsIter<'_> {
        NamespaceBindingsIter {
            resolver: self,
            cursor: 2,
        }
    }
    pub const fn bindings_of(&self, level: u16) -> NamespaceBindingsOfLevelIter<'_> {
        NamespaceBindingsOfLevelIter {
            resolver: self,
            cursor: 0,
            level,
        }
    }
    pub const fn level(&self) -> u16 {
        self.nesting_level
    }
}
#[derive(Debug, Clone)]
pub struct NamespaceBindingsIter<'a> {
    resolver: &'a NamespaceResolver,
    cursor: usize,
}
impl<'a> Iterator for NamespaceBindingsIter<'a> {
    type Item = (PrefixDeclaration<'a>, Namespace<'a>);
    fn next(&mut self) -> Option<(PrefixDeclaration<'a>, Namespace<'a>)> {
        while let Some(binding) = self.resolver.bindings.get(self.cursor) {
            self.cursor += 1;
            let prefix = binding.prefix(&self.resolver.buffer);
            if self.resolver.bindings[self.cursor..]
                .iter()
                .any(|ne| prefix == ne.prefix(&self.resolver.buffer))
            {
                continue;
            }
            if let ResolveResult::Bound(namespace) = binding.namespace(&self.resolver.buffer) {
                let prefix = match prefix {
                    Some(Prefix(prefix)) => PrefixDeclaration::Named(prefix),
                    None => PrefixDeclaration::Default,
                };
                return Some((prefix, namespace));
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.resolver.bindings.len() - self.cursor))
    }
}
impl<'a> FusedIterator for NamespaceBindingsIter<'a> {}
#[derive(Debug, Clone)]
pub struct NamespaceBindingsOfLevelIter<'a> {
    resolver: &'a NamespaceResolver,
    cursor: usize,
    level: u16,
}
impl<'a> Iterator for NamespaceBindingsOfLevelIter<'a> {
    type Item = (PrefixDeclaration<'a>, Namespace<'a>);
    fn next(&mut self) -> Option<(PrefixDeclaration<'a>, Namespace<'a>)> {
        while let Some(binding) = self.resolver.bindings.get(self.cursor) {
            self.cursor += 1;
            if binding.level < self.level {
                continue;
            }
            if binding.level > self.level {
                break;
            }
            if let ResolveResult::Bound(namespace) = binding.namespace(&self.resolver.buffer) {
                let prefix = match binding.prefix(&self.resolver.buffer) {
                    Some(Prefix(prefix)) => PrefixDeclaration::Named(prefix),
                    None => PrefixDeclaration::Default,
                };
                return Some((prefix, namespace));
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.resolver.bindings.len() - self.cursor))
    }
}
impl<'a> FusedIterator for NamespaceBindingsOfLevelIter<'a> {}
