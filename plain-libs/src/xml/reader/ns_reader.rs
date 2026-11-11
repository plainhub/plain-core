use crate::xml::errors::Result;
use crate::xml::events::{BytesText, Event};
use crate::xml::name::{NamespaceResolver, QName, ResolveResult};
use crate::xml::reader::{Config, Reader, Span, XmlSource};
use std::io::BufRead;
use std::ops::Deref;
#[derive(Debug, Clone)]
pub struct NsReader<R> {
    pub(crate) reader: Reader<R>,
    pub(crate) ns_resolver: NamespaceResolver,
    pub(crate) pending_pop: bool,
}
impl<R> NsReader<R> {
    #[inline]
    pub fn from_reader(reader: R) -> Self {
        Self::new(Reader::from_reader(reader))
    }
    #[inline]
    pub const fn config(&self) -> &Config {
        self.reader.config()
    }
    #[inline]
    pub fn config_mut(&mut self) -> &mut Config {
        self.reader.config_mut()
    }
}
impl<R> NsReader<R> {
    #[inline]
    fn new(reader: Reader<R>) -> Self {
        Self {
            reader,
            ns_resolver: NamespaceResolver::default(),
            pending_pop: false,
        }
    }
    fn read_event_impl<'i, B>(&mut self, buf: B) -> Result<Event<'i>>
    where
        R: XmlSource<'i, B>,
    {
        self.pop();
        let event = self.reader.read_event_impl(buf);
        self.process_event(event)
    }
    pub(super) fn pop(&mut self) {
        if self.pending_pop {
            self.ns_resolver.pop();
            self.pending_pop = false;
        }
    }
    pub(super) fn process_event<'i>(&mut self, event: Result<Event<'i>>) -> Result<Event<'i>> {
        match event {
            Ok(Event::Start(e)) => {
                self.ns_resolver.push(&e)?;
                Ok(Event::Start(e))
            }
            Ok(Event::Empty(e)) => {
                self.ns_resolver.push(&e)?;
                self.pending_pop = true;
                Ok(Event::Empty(e))
            }
            Ok(Event::End(e)) => {
                self.pending_pop = true;
                Ok(Event::End(e))
            }
            e => e,
        }
    }
}
impl<R> NsReader<R> {
    #[inline]
    pub fn into_inner(self) -> R {
        self.reader.into_inner()
    }
    pub fn get_mut(&mut self) -> &mut R {
        self.reader.get_mut()
    }
    #[inline]
    pub const fn resolver(&self) -> &NamespaceResolver {
        &self.ns_resolver
    }
    #[inline]
    pub fn resolver_mut(&mut self) -> &mut NamespaceResolver {
        &mut self.ns_resolver
    }
}
impl<R: BufRead> NsReader<R> {
    #[inline]
    pub fn read_event_into<'b>(&mut self, buf: &'b mut Vec<u8>) -> Result<Event<'b>> {
        self.read_event_impl(buf)
    }
    #[inline]
    pub fn read_resolved_event_into<'b>(
        &mut self,
        buf: &'b mut Vec<u8>,
    ) -> Result<(ResolveResult<'_>, Event<'b>)> {
        let event = self.read_event_impl(buf)?;
        Ok(self.ns_resolver.resolve_event(event))
    }
    #[inline]
    pub fn read_to_end_into(&mut self, end: QName, buf: &mut Vec<u8>) -> Result<Span> {
        let result = self.reader.read_to_end_into(end, buf)?;
        self.ns_resolver.pop();
        Ok(result)
    }
    #[inline]
    pub fn read_text_into<'b>(
        &mut self,
        end: QName,
        buf: &'b mut Vec<u8>,
    ) -> Result<BytesText<'b>> {
        let result = self.reader.read_text_into(end, buf)?;
        self.ns_resolver.pop();
        Ok(result)
    }
}
impl<'i> NsReader<&'i [u8]> {
    #[inline]
    pub fn from_str(s: &'i str) -> Self {
        Self::new(Reader::from_str(s))
    }
    #[inline]
    pub fn read_event(&mut self) -> Result<Event<'i>> {
        self.read_event_impl(())
    }
    #[inline]
    pub fn read_resolved_event(&mut self) -> Result<(ResolveResult<'_>, Event<'i>)> {
        let event = self.read_event_impl(())?;
        Ok(self.ns_resolver.resolve_event(event))
    }
    #[inline]
    pub fn read_to_end(&mut self, end: QName) -> Result<Span> {
        let result = self.reader.read_to_end(end)?;
        self.ns_resolver.pop();
        Ok(result)
    }
    #[inline]
    pub fn read_text(&mut self, end: QName) -> Result<BytesText<'i>> {
        let result = self.reader.read_text(end)?;
        self.ns_resolver.pop();
        Ok(result)
    }
}
impl<R> Deref for NsReader<R> {
    type Target = Reader<R>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.reader
    }
}
