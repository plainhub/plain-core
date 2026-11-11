use crate::regex_engine::automata::util::{
    prefilter::PrefilterI,
    search::{MatchKind, Span},
};
#[derive(Clone, Debug)]
pub(crate) struct Memmem {
    finder: memchr::memmem::Finder<'static>,
}
impl Memmem {
    pub(crate) fn new<B: AsRef<[u8]>>(_kind: MatchKind, needles: &[B]) -> Option<Memmem> {
        {
            if needles.len() != 1 {
                return None;
            }
            let needle = needles[0].as_ref();
            let finder = memchr::memmem::Finder::new(needle).into_owned();
            Some(Memmem { finder })
        }
    }
}
impl PrefilterI for Memmem {
    fn name(&self) -> &'static str {
        "memmem"
    }
    fn find(&self, haystack: &[u8], span: Span) -> Option<Span> {
        {
            self.finder.find(&haystack[span]).map(|i| {
                let start = span.start + i;
                let end = start + self.finder.needle().len();
                Span { start, end }
            })
        }
    }
    fn prefix(&self, haystack: &[u8], span: Span) -> Option<Span> {
        {
            let needle = self.finder.needle();
            if haystack[span].starts_with(needle) {
                Some(Span {
                    end: span.start + needle.len(),
                    ..span
                })
            } else {
                None
            }
        }
    }
    fn memory_usage(&self) -> usize {
        { self.finder.needle().len() }
    }
    fn is_fast(&self) -> bool {
        { true }
    }
}
