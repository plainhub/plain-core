pub mod attributes;
use crate::xml::XmlVersion;
use crate::xml::encoding::EncodingError;
use crate::xml::errors::{Error, IllFormedError};
use crate::xml::escape::{
    EscapeError, escape, minimal_escape, normalize_xml10_eols, normalize_xml11_eols, parse_number,
    partial_escape,
};
use crate::xml::name::{LocalName, QName};
use crate::xml::utils::{self, name_len, trim_xml_end, trim_xml_start, write_cow_string};
use attributes::{AttrError, Attribute, Attributes};
use std::borrow::Cow;
use std::fmt::{self, Debug, Formatter};
use std::iter::FusedIterator;
use std::mem::replace;
use std::ops::Deref;
#[derive(Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct BytesStart<'i> {
    pub(crate) buf: Cow<'i, str>,
    pub(crate) name_len: usize,
}
impl<'i> BytesStart<'i> {
    #[inline]
    pub(crate) const fn wrap(content: &'i str, name_len: usize) -> Self {
        BytesStart {
            buf: Cow::Borrowed(content),
            name_len,
        }
    }
    #[inline]
    pub fn new<C: Into<Cow<'i, str>>>(name: C) -> Self {
        let buf: Cow<'i, str> = name.into();
        BytesStart {
            name_len: buf.len(),
            buf,
        }
    }
    #[inline]
    pub fn from_content<C: Into<Cow<'i, str>>>(content: C, name_len: usize) -> Self {
        BytesStart {
            buf: content.into(),
            name_len,
        }
    }
    pub fn into_owned(self) -> BytesStart<'static> {
        BytesStart {
            buf: Cow::Owned(self.buf.into_owned()),
            name_len: self.name_len,
        }
    }
    pub fn to_owned(&self) -> BytesStart<'static> {
        BytesStart {
            buf: Cow::Owned(self.buf.clone().into_owned()),
            name_len: self.name_len,
        }
    }
    pub fn borrow(&self) -> BytesStart<'_> {
        BytesStart {
            buf: Cow::Borrowed(&self.buf),
            name_len: self.name_len,
        }
    }
    #[inline]
    pub fn to_end(&self) -> BytesEnd<'_> {
        BytesEnd::from(self.name())
    }
    #[inline]
    pub fn name(&self) -> QName<'_> {
        QName(&self.buf[..self.name_len])
    }
    #[inline]
    pub fn local_name(&self) -> LocalName<'_> {
        self.name().into()
    }
    pub fn set_name(&mut self, name: &str) -> &mut BytesStart<'i> {
        let s = self.buf.to_mut();
        s.replace_range(..self.name_len, name);
        self.name_len = name.len();
        self
    }
}
impl<'i> BytesStart<'i> {
    pub fn with_attributes<'a, I>(mut self, attributes: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Attribute<'a>>,
    {
        self.extend_attributes(attributes);
        self
    }
    pub fn extend_attributes<'a, I>(&mut self, attributes: I) -> &mut BytesStart<'i>
    where
        I: IntoIterator,
        I::Item: Into<Attribute<'a>>,
    {
        for attr in attributes {
            self.push_attribute(attr);
        }
        self
    }
    pub fn push_attribute<'a, A>(&mut self, attr: A)
    where
        A: Into<Attribute<'a>>,
    {
        self.buf.to_mut().push(' ');
        self.push_attr(attr.into());
    }
    pub fn clear_attributes(&mut self) -> &mut BytesStart<'i> {
        self.buf.to_mut().truncate(self.name_len);
        self
    }
    pub fn attributes(&self) -> Attributes<'_> {
        Attributes::wrap(&self.buf, self.name_len, false)
    }
    pub fn html_attributes(&self) -> Attributes<'_> {
        Attributes::wrap(&self.buf, self.name_len, true)
    }
    #[inline]
    pub fn attributes_raw(&self) -> &str {
        &self.buf[self.name_len..]
    }
    pub fn try_get_attribute<'a>(
        &'a self,
        attr_name: &str,
    ) -> Result<Option<Attribute<'a>>, AttrError> {
        for a in self.attributes().with_checks(false) {
            let a = a?;
            if a.key.as_ref() == attr_name {
                return Ok(Some(a));
            }
        }
        Ok(None)
    }
    pub(crate) fn push_attr<'a>(&mut self, attr: Attribute<'a>) {
        let s = self.buf.to_mut();
        s.push_str(attr.key.as_ref());
        s.push_str("=\"");
        s.push_str(&attr.value);
        s.push('"');
    }
}
impl<'i> Debug for BytesStart<'i> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "BytesStart {{ buf: ")?;
        write_cow_string(f, &self.buf)?;
        write!(f, ", name_len: {} }}", self.name_len)
    }
}
impl<'i> Deref for BytesStart<'i> {
    type Target = str;
    fn deref(&self) -> &str {
        &self.buf
    }
}
impl AsRef<str> for BytesStart<'_> {
    fn as_ref(&self) -> &str {
        self
    }
}
#[derive(Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct BytesEnd<'i> {
    name: Cow<'i, str>,
}
impl<'i> BytesEnd<'i> {
    #[inline]
    pub(crate) const fn wrap(name: Cow<'i, str>) -> Self {
        BytesEnd { name }
    }
    #[inline]
    pub fn new<C: Into<Cow<'i, str>>>(name: C) -> Self {
        Self::wrap(name.into())
    }
    pub fn into_owned(self) -> BytesEnd<'static> {
        BytesEnd {
            name: Cow::Owned(self.name.into_owned()),
        }
    }
    #[inline]
    pub fn borrow(&self) -> BytesEnd<'_> {
        BytesEnd {
            name: Cow::Borrowed(&self.name),
        }
    }
    #[inline]
    pub fn name(&self) -> QName<'_> {
        QName(&self.name)
    }
    #[inline]
    pub fn local_name(&self) -> LocalName<'_> {
        self.name().into()
    }
}
impl<'i> Debug for BytesEnd<'i> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "BytesEnd {{ name: ")?;
        write_cow_string(f, &self.name)?;
        write!(f, " }}")
    }
}
impl<'i> Deref for BytesEnd<'i> {
    type Target = str;
    fn deref(&self) -> &str {
        &self.name
    }
}
impl AsRef<str> for BytesEnd<'_> {
    fn as_ref(&self) -> &str {
        self
    }
}
impl<'i> From<QName<'i>> for BytesEnd<'i> {
    #[inline]
    fn from(name: QName<'i>) -> Self {
        Self::wrap(Cow::Borrowed(name.into_inner()))
    }
}
#[derive(Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct BytesText<'i> {
    content: Cow<'i, str>,
}
impl<'i> BytesText<'i> {
    #[inline]
    pub(crate) const fn wrap(content: &'i str) -> Self {
        Self {
            content: Cow::Borrowed(content),
        }
    }
    #[inline]
    pub fn from_escaped<C: Into<Cow<'i, str>>>(content: C) -> Self {
        Self {
            content: content.into(),
        }
    }
    #[inline]
    pub fn new(content: &'i str) -> Self {
        Self::from_escaped(escape(content))
    }
    #[inline]
    pub fn into_owned(self) -> BytesText<'static> {
        BytesText {
            content: Cow::Owned(self.content.into_owned()),
        }
    }
    #[inline]
    pub fn into_inner(self) -> Cow<'i, str> {
        self.content
    }
    #[inline]
    pub fn borrow(&self) -> BytesText<'_> {
        BytesText {
            content: Cow::Borrowed(&self.content),
        }
    }
    pub fn xml10_content(&self) -> Cow<'i, str> {
        match &self.content {
            Cow::Borrowed(s) => normalize_xml10_eols(s),
            Cow::Owned(s) => Cow::Owned(normalize_xml10_eols(s).into_owned()),
        }
    }
    pub fn xml11_content(&self) -> Cow<'i, str> {
        match &self.content {
            Cow::Borrowed(s) => normalize_xml11_eols(s),
            Cow::Owned(s) => Cow::Owned(normalize_xml11_eols(s).into_owned()),
        }
    }
    #[inline]
    pub fn xml_content(&self, version: XmlVersion) -> Cow<'i, str> {
        match version {
            XmlVersion::Explicit1_1 => self.xml11_content(),
            _ => self.xml10_content(),
        }
    }
    #[inline]
    pub fn html_content(&self) -> Cow<'i, str> {
        self.xml10_content()
    }
    pub fn inplace_trim_start(&mut self) -> bool {
        self.content = trim_cow(
            replace(&mut self.content, Cow::Borrowed("")),
            trim_xml_start,
        );
        self.content.is_empty()
    }
    pub fn inplace_trim_end(&mut self) -> bool {
        self.content = trim_cow(replace(&mut self.content, Cow::Borrowed("")), trim_xml_end);
        self.content.is_empty()
    }
}
impl<'i> Debug for BytesText<'i> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "BytesText {{ content: ")?;
        write_cow_string(f, &self.content)?;
        write!(f, " }}")
    }
}
impl<'i> Deref for BytesText<'i> {
    type Target = str;
    fn deref(&self) -> &str {
        &self.content
    }
}
impl AsRef<str> for BytesText<'_> {
    fn as_ref(&self) -> &str {
        self
    }
}
#[derive(Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct BytesCData<'i> {
    content: Cow<'i, str>,
}
impl<'i> BytesCData<'i> {
    #[inline]
    pub(crate) const fn wrap(content: &'i str) -> Self {
        Self {
            content: Cow::Borrowed(content),
        }
    }
    #[inline]
    pub fn new<C: Into<Cow<'i, str>>>(content: C) -> Self {
        Self {
            content: content.into(),
        }
    }
    #[inline]
    pub const fn escaped(content: &'i str) -> CDataIterator<'i> {
        CDataIterator {
            inner: utils::CDataIterator::new(content),
        }
    }
    #[inline]
    pub fn into_owned(self) -> BytesCData<'static> {
        BytesCData {
            content: Cow::Owned(self.content.into_owned()),
        }
    }
    #[inline]
    pub fn into_inner(self) -> Cow<'i, str> {
        self.content
    }
    #[inline]
    pub fn borrow(&self) -> BytesCData<'_> {
        BytesCData {
            content: Cow::Borrowed(&self.content),
        }
    }
    pub fn escape(self) -> Result<BytesText<'i>, EncodingError> {
        Ok(match self.content {
            Cow::Borrowed(s) => BytesText::from_escaped(escape(s)),
            Cow::Owned(s) => BytesText::from_escaped(escape(&s).into_owned()),
        })
    }
    pub fn partial_escape(self) -> Result<BytesText<'i>, EncodingError> {
        Ok(match self.content {
            Cow::Borrowed(s) => BytesText::from_escaped(partial_escape(s)),
            Cow::Owned(s) => BytesText::from_escaped(partial_escape(&s).into_owned()),
        })
    }
    pub fn minimal_escape(self) -> Result<BytesText<'i>, EncodingError> {
        Ok(match self.content {
            Cow::Borrowed(s) => BytesText::from_escaped(minimal_escape(s)),
            Cow::Owned(s) => BytesText::from_escaped(minimal_escape(&s).into_owned()),
        })
    }
    pub fn xml10_content(&self) -> Cow<'i, str> {
        match &self.content {
            Cow::Borrowed(s) => normalize_xml10_eols(s),
            Cow::Owned(s) => Cow::Owned(normalize_xml10_eols(s).into_owned()),
        }
    }
    pub fn xml11_content(&self) -> Cow<'i, str> {
        match &self.content {
            Cow::Borrowed(s) => normalize_xml11_eols(s),
            Cow::Owned(s) => Cow::Owned(normalize_xml11_eols(s).into_owned()),
        }
    }
    #[inline]
    pub fn xml_content(&self, version: XmlVersion) -> Cow<'i, str> {
        match version {
            XmlVersion::Explicit1_1 => self.xml11_content(),
            _ => self.xml10_content(),
        }
    }
    #[inline]
    pub fn html_content(&self) -> Cow<'i, str> {
        self.xml10_content()
    }
}
impl<'i> Debug for BytesCData<'i> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "BytesCData {{ content: ")?;
        write_cow_string(f, &self.content)?;
        write!(f, " }}")
    }
}
impl<'i> Deref for BytesCData<'i> {
    type Target = str;
    fn deref(&self) -> &str {
        &self.content
    }
}
impl AsRef<str> for BytesCData<'_> {
    fn as_ref(&self) -> &str {
        self
    }
}
#[derive(Debug, Clone)]
pub struct CDataIterator<'a> {
    inner: utils::CDataIterator<'a>,
}
impl<'a> Iterator for CDataIterator<'a> {
    type Item = BytesCData<'a>;
    fn next(&mut self) -> Option<BytesCData<'a>> {
        self.inner.next().map(BytesCData::wrap)
    }
}
impl FusedIterator for CDataIterator<'_> {}
#[derive(Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct BytesPI<'i> {
    content: BytesStart<'i>,
}
impl<'i> BytesPI<'i> {
    #[inline]
    pub(crate) const fn wrap(content: &'i str, target_len: usize) -> Self {
        Self {
            content: BytesStart::wrap(content, target_len),
        }
    }
    #[inline]
    pub fn new<C: Into<Cow<'i, str>>>(content: C) -> Self {
        let buf: Cow<'i, str> = content.into();
        let name_len = name_len(buf.as_bytes());
        Self {
            content: BytesStart { buf, name_len },
        }
    }
    #[inline]
    pub fn into_owned(self) -> BytesPI<'static> {
        BytesPI {
            content: self.content.into_owned(),
        }
    }
    #[inline]
    pub fn into_inner(self) -> Cow<'i, str> {
        self.content.buf
    }
    #[inline]
    pub fn borrow(&self) -> BytesPI<'_> {
        BytesPI {
            content: self.content.borrow(),
        }
    }
    #[inline]
    pub fn target(&self) -> &str {
        self.content.name().0
    }
    #[inline]
    pub fn content(&self) -> &str {
        self.content.attributes_raw()
    }
    #[inline]
    pub fn attributes(&self) -> Attributes<'_> {
        self.content.attributes()
    }
}
impl<'i> Debug for BytesPI<'i> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "BytesPI {{ content: ")?;
        write_cow_string(f, &self.content.buf)?;
        write!(f, " }}")
    }
}
impl<'i> Deref for BytesPI<'i> {
    type Target = str;
    fn deref(&self) -> &str {
        &self.content.buf
    }
}
impl AsRef<str> for BytesPI<'_> {
    fn as_ref(&self) -> &str {
        self
    }
}
#[derive(Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct BytesDecl<'i> {
    content: BytesStart<'i>,
}
impl<'i> BytesDecl<'i> {
    pub fn new(
        version: &str,
        encoding: Option<&str>,
        standalone: Option<&str>,
    ) -> BytesDecl<'static> {
        let encoding_attr_len = if let Some(xs) = encoding {
            12 + xs.len()
        } else {
            0
        };
        let standalone_attr_len = if let Some(xs) = standalone {
            14 + xs.len()
        } else {
            0
        };
        let mut buf = String::with_capacity(14 + encoding_attr_len + standalone_attr_len);
        buf.push_str("xml version=\"");
        buf.push_str(version);
        if let Some(encoding_val) = encoding {
            buf.push_str("\" encoding=\"");
            buf.push_str(encoding_val);
        }
        if let Some(standalone_val) = standalone {
            buf.push_str("\" standalone=\"");
            buf.push_str(standalone_val);
        }
        buf.push('"');
        BytesDecl {
            content: BytesStart::from_content(buf, 3),
        }
    }
    pub const fn from_start(start: BytesStart<'i>) -> Self {
        Self { content: start }
    }
    pub fn version(&self) -> Result<Cow<'_, str>, Error> {
        match self.content.attributes().with_checks(false).next() {
            Some(Ok(a)) if a.key.as_ref() == "version" => Ok(a.value),
            Some(Ok(a)) => {
                let found = a.key.as_ref().to_string();
                Err(Error::IllFormed(IllFormedError::MissingDeclVersion(Some(
                    found,
                ))))
            }
            Some(Err(e)) => Err(e.into()),
            None => Err(Error::IllFormed(IllFormedError::MissingDeclVersion(None))),
        }
    }
    pub fn encoding(&self) -> Option<Result<Cow<'_, str>, AttrError>> {
        self.content
            .try_get_attribute("encoding")
            .map(|a| a.map(|a| a.value))
            .transpose()
    }
    pub fn standalone(&self) -> Option<Result<Cow<'_, str>, AttrError>> {
        self.content
            .try_get_attribute("standalone")
            .map(|a| a.map(|a| a.value))
            .transpose()
    }
    pub fn xml_version(&self) -> Result<XmlVersion, Error> {
        let v = self.version()?;
        match v.as_ref() {
            "1.0" => Ok(XmlVersion::Explicit1_0),
            "1.1" => Ok(XmlVersion::Explicit1_1),
            _ => Err(Error::IllFormed(IllFormedError::UnknownVersion)),
        }
    }
    pub fn into_owned(self) -> BytesDecl<'static> {
        BytesDecl {
            content: self.content.into_owned(),
        }
    }
    #[inline]
    pub fn borrow(&self) -> BytesDecl<'_> {
        BytesDecl {
            content: self.content.borrow(),
        }
    }
}
impl<'i> Deref for BytesDecl<'i> {
    type Target = str;
    fn deref(&self) -> &str {
        &self.content.buf
    }
}
impl AsRef<str> for BytesDecl<'_> {
    fn as_ref(&self) -> &str {
        self
    }
}
#[derive(Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct BytesRef<'i> {
    content: Cow<'i, str>,
}
impl<'i> BytesRef<'i> {
    #[inline]
    pub(crate) const fn wrap(content: &'i str) -> Self {
        Self {
            content: Cow::Borrowed(content),
        }
    }
    #[inline]
    pub fn new<C: Into<Cow<'i, str>>>(name: C) -> Self {
        Self {
            content: name.into(),
        }
    }
    pub fn into_owned(self) -> BytesRef<'static> {
        BytesRef {
            content: Cow::Owned(self.content.into_owned()),
        }
    }
    #[inline]
    pub fn into_inner(self) -> Cow<'i, str> {
        self.content
    }
    #[inline]
    pub fn borrow(&self) -> BytesRef<'_> {
        BytesRef {
            content: Cow::Borrowed(&self.content),
        }
    }
    pub fn xml10_content(&self) -> Cow<'i, str> {
        match &self.content {
            Cow::Borrowed(s) => normalize_xml10_eols(s),
            Cow::Owned(s) => Cow::Owned(normalize_xml10_eols(s).into_owned()),
        }
    }
    pub fn xml11_content(&self) -> Cow<'i, str> {
        match &self.content {
            Cow::Borrowed(s) => normalize_xml11_eols(s),
            Cow::Owned(s) => Cow::Owned(normalize_xml11_eols(s).into_owned()),
        }
    }
    #[inline]
    pub fn xml_content(&self, version: XmlVersion) -> Cow<'i, str> {
        match version {
            XmlVersion::Explicit1_1 => self.xml11_content(),
            _ => self.xml10_content(),
        }
    }
    #[inline]
    pub fn html_content(&self) -> Cow<'i, str> {
        self.xml10_content()
    }
    pub fn is_char_ref(&self) -> bool {
        self.content.starts_with('#')
    }
    pub fn resolve_char_ref(&self) -> Result<Option<char>, Error> {
        if let Some(num) = self.content.strip_prefix('#') {
            let ch = parse_number(num).map_err(EscapeError::InvalidCharRef)?;
            return Ok(Some(ch));
        }
        Ok(None)
    }
}
impl<'i> Debug for BytesRef<'i> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "BytesRef {{ content: ")?;
        write_cow_string(f, &self.content)?;
        write!(f, " }}")
    }
}
impl<'i> Deref for BytesRef<'i> {
    type Target = str;
    fn deref(&self) -> &str {
        &self.content
    }
}
impl AsRef<str> for BytesRef<'_> {
    fn as_ref(&self) -> &str {
        self
    }
}
#[derive(Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub enum Event<'i> {
    Start(BytesStart<'i>),
    End(BytesEnd<'i>),
    Empty(BytesStart<'i>),
    Text(BytesText<'i>),
    CData(BytesCData<'i>),
    Comment(BytesText<'i>),
    Decl(BytesDecl<'i>),
    PI(BytesPI<'i>),
    DocType(BytesText<'i>),
    GeneralRef(BytesRef<'i>),
    Eof,
}
impl<'i> Event<'i> {
    pub fn into_owned(self) -> Event<'static> {
        match self {
            Event::Start(e) => Event::Start(e.into_owned()),
            Event::End(e) => Event::End(e.into_owned()),
            Event::Empty(e) => Event::Empty(e.into_owned()),
            Event::Text(e) => Event::Text(e.into_owned()),
            Event::Comment(e) => Event::Comment(e.into_owned()),
            Event::CData(e) => Event::CData(e.into_owned()),
            Event::Decl(e) => Event::Decl(e.into_owned()),
            Event::PI(e) => Event::PI(e.into_owned()),
            Event::DocType(e) => Event::DocType(e.into_owned()),
            Event::GeneralRef(e) => Event::GeneralRef(e.into_owned()),
            Event::Eof => Event::Eof,
        }
    }
    #[inline]
    pub fn borrow(&self) -> Event<'_> {
        match self {
            Event::Start(e) => Event::Start(e.borrow()),
            Event::End(e) => Event::End(e.borrow()),
            Event::Empty(e) => Event::Empty(e.borrow()),
            Event::Text(e) => Event::Text(e.borrow()),
            Event::Comment(e) => Event::Comment(e.borrow()),
            Event::CData(e) => Event::CData(e.borrow()),
            Event::Decl(e) => Event::Decl(e.borrow()),
            Event::PI(e) => Event::PI(e.borrow()),
            Event::DocType(e) => Event::DocType(e.borrow()),
            Event::GeneralRef(e) => Event::GeneralRef(e.borrow()),
            Event::Eof => Event::Eof,
        }
    }
}
impl<'i> Deref for Event<'i> {
    type Target = str;
    fn deref(&self) -> &str {
        match *self {
            Event::Start(ref e) | Event::Empty(ref e) => e,
            Event::End(ref e) => e,
            Event::Text(ref e) => e,
            Event::Decl(ref e) => e,
            Event::PI(ref e) => e,
            Event::CData(ref e) => e,
            Event::Comment(ref e) => e,
            Event::DocType(ref e) => e,
            Event::GeneralRef(ref e) => e,
            Event::Eof => "",
        }
    }
}
impl<'i> AsRef<Event<'i>> for Event<'i> {
    fn as_ref(&self) -> &Event<'i> {
        self
    }
}
fn trim_cow<'a, F>(value: Cow<'a, str>, trim: F) -> Cow<'a, str>
where
    F: for<'s> FnOnce(&'s str) -> &'s str,
{
    match value {
        Cow::Borrowed(s) => Cow::Borrowed(trim(s)),
        Cow::Owned(s) => {
            let trimmed = trim(&s);
            if trimmed.len() != s.len() {
                Cow::Owned(trimmed.to_owned())
            } else {
                Cow::Owned(s)
            }
        }
    }
}
