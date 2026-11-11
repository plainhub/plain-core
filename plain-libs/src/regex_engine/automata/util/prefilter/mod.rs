mod aho_corasick;
mod byteset;
mod memchr;
mod memmem;
mod teddy;
pub(crate) use crate::regex_engine::automata::util::prefilter::{
    aho_corasick::AhoCorasick,
    byteset::ByteSet,
    memchr::{Memchr, Memchr2, Memchr3},
    memmem::Memmem,
    teddy::Teddy,
};
use crate::regex_engine::automata::util::search::{MatchKind, Span};
use crate::regex_engine::syntax::hir::{Hir, literal};
use core::{
    borrow::Borrow,
    fmt::Debug,
    panic::{RefUnwindSafe, UnwindSafe},
};
use std::sync::Arc;
#[derive(Clone, Debug)]
pub struct Prefilter {
    pre: Arc<dyn PrefilterI>,
    is_fast: bool,
    max_needle_len: usize,
}
impl Prefilter {
    pub fn new<B: AsRef<[u8]>>(kind: MatchKind, needles: &[B]) -> Option<Prefilter> {
        Choice::new(kind, needles).and_then(|choice| {
            let max_needle_len = needles.iter().map(|b| b.as_ref().len()).max().unwrap_or(0);
            Prefilter::from_choice(choice, max_needle_len)
        })
    }
    fn from_choice(choice: Choice, max_needle_len: usize) -> Option<Prefilter> {
        {
            let pre: Arc<dyn PrefilterI> = match choice {
                Choice::Memchr(p) => Arc::new(p),
                Choice::Memchr2(p) => Arc::new(p),
                Choice::Memchr3(p) => Arc::new(p),
                Choice::Memmem(p) => Arc::new(p),
                Choice::Teddy(p) => Arc::new(p),
                Choice::ByteSet(p) => Arc::new(p),
                Choice::AhoCorasick(p) => Arc::new(p),
            };
            let is_fast = pre.is_fast();
            Some(Prefilter {
                pre,
                is_fast,
                max_needle_len,
            })
        }
    }

    pub fn from_hirs_prefix<H: Borrow<Hir>>(kind: MatchKind, hirs: &[H]) -> Option<Prefilter> {
        prefixes(kind, hirs)
            .literals()
            .and_then(|lits| Prefilter::new(kind, lits))
    }
    #[inline]
    pub fn find(&self, haystack: &[u8], span: Span) -> Option<Span> {
        { self.pre.find(haystack, span) }
    }
    #[inline]
    pub fn prefix(&self, haystack: &[u8], span: Span) -> Option<Span> {
        { self.pre.prefix(haystack, span) }
    }
    #[inline]
    pub fn memory_usage(&self) -> usize {
        { self.pre.memory_usage() }
    }
    #[inline]
    pub fn max_needle_len(&self) -> usize {
        { self.max_needle_len }
    }
    #[inline]
    pub fn is_fast(&self) -> bool {
        { self.is_fast }
    }
}
pub(crate) trait PrefilterI:
    Debug + Send + Sync + RefUnwindSafe + UnwindSafe + 'static
{
    fn name(&self) -> &'static str;
    fn find(&self, haystack: &[u8], span: Span) -> Option<Span>;
    fn prefix(&self, haystack: &[u8], span: Span) -> Option<Span>;
    fn memory_usage(&self) -> usize;
    fn is_fast(&self) -> bool;
}
impl<P: PrefilterI + ?Sized> PrefilterI for Arc<P> {
    fn name(&self) -> &'static str {
        (**self).name()
    }
    #[cfg_attr(all(), inline(always))]
    fn find(&self, haystack: &[u8], span: Span) -> Option<Span> {
        (**self).find(haystack, span)
    }
    #[cfg_attr(all(), inline(always))]
    fn prefix(&self, haystack: &[u8], span: Span) -> Option<Span> {
        (**self).prefix(haystack, span)
    }
    #[cfg_attr(all(), inline(always))]
    fn memory_usage(&self) -> usize {
        (**self).memory_usage()
    }
    #[cfg_attr(all(), inline(always))]
    fn is_fast(&self) -> bool {
        (&**self).is_fast()
    }
}
#[derive(Clone, Debug)]
pub(crate) enum Choice {
    Memchr(Memchr),
    Memchr2(Memchr2),
    Memchr3(Memchr3),
    Memmem(Memmem),
    Teddy(Teddy),
    ByteSet(ByteSet),
    AhoCorasick(AhoCorasick),
}
impl Choice {
    pub(crate) fn new<B: AsRef<[u8]>>(kind: MatchKind, needles: &[B]) -> Option<Choice> {
        if needles.len() == 0 {
            debug!("prefilter building failed: found empty set of literals");
            return None;
        }
        if needles.iter().any(|n| n.as_ref().is_empty()) {
            debug!("prefilter building failed: literals match empty string");
            return None;
        }
        if let Some(pre) = Memchr::new(kind, needles) {
            debug!("prefilter built: memchr");
            return Some(Choice::Memchr(pre));
        }
        if let Some(pre) = Memchr2::new(kind, needles) {
            debug!("prefilter built: memchr2");
            return Some(Choice::Memchr2(pre));
        }
        if let Some(pre) = Memchr3::new(kind, needles) {
            debug!("prefilter built: memchr3");
            return Some(Choice::Memchr3(pre));
        }
        if let Some(pre) = Memmem::new(kind, needles) {
            debug!("prefilter built: memmem");
            return Some(Choice::Memmem(pre));
        }
        if let Some(pre) = Teddy::new(kind, needles) {
            debug!("prefilter built: teddy");
            return Some(Choice::Teddy(pre));
        }
        if let Some(pre) = ByteSet::new(kind, needles) {
            debug!("prefilter built: byteset");
            return Some(Choice::ByteSet(pre));
        }
        if let Some(pre) = AhoCorasick::new(kind, needles) {
            debug!("prefilter built: aho-corasick");
            return Some(Choice::AhoCorasick(pre));
        }
        debug!("prefilter building failed: no strategy could be found");
        None
    }
}
pub(crate) fn prefixes<H>(kind: MatchKind, hirs: &[H]) -> literal::Seq
where
    H: core::borrow::Borrow<Hir>,
{
    let mut extractor = literal::Extractor::new();
    extractor.kind(literal::ExtractKind::Prefix);
    let mut prefixes = literal::Seq::empty();
    for hir in hirs {
        prefixes.union(&mut extractor.extract(hir.borrow()));
    }
    debug!(
        "prefixes (len={:?}, exact={:?}) extracted before optimization: {:?}",
        prefixes.len(),
        prefixes.is_exact(),
        prefixes
    );
    match kind {
        MatchKind::All => {
            prefixes.sort();
            prefixes.dedup();
        }
        MatchKind::LeftmostFirst => {
            prefixes.optimize_for_prefix_by_preference();
        }
    }
    debug!(
        "prefixes (len={:?}, exact={:?}) extracted after optimization: {:?}",
        prefixes.len(),
        prefixes.is_exact(),
        prefixes
    );
    prefixes
}
pub(crate) fn suffixes<H>(kind: MatchKind, hirs: &[H]) -> literal::Seq
where
    H: core::borrow::Borrow<Hir>,
{
    let mut extractor = literal::Extractor::new();
    extractor.kind(literal::ExtractKind::Suffix);
    let mut suffixes = literal::Seq::empty();
    for hir in hirs {
        suffixes.union(&mut extractor.extract(hir.borrow()));
    }
    debug!(
        "suffixes (len={:?}, exact={:?}) extracted before optimization: {:?}",
        suffixes.len(),
        suffixes.is_exact(),
        suffixes
    );
    match kind {
        MatchKind::All => {
            suffixes.sort();
            suffixes.dedup();
        }
        MatchKind::LeftmostFirst => {
            suffixes.optimize_for_suffix_by_preference();
        }
    }
    debug!(
        "suffixes (len={:?}, exact={:?}) extracted after optimization: {:?}",
        suffixes.len(),
        suffixes.is_exact(),
        suffixes
    );
    suffixes
}
