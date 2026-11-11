pub(crate) use crate::memchr::memmem::searcher::Pre;
pub use crate::memchr::memmem::searcher::PrefilterConfig as Prefilter;
use crate::memchr::{
    arch::all::{
        packedpair::{DefaultFrequencyRank, HeuristicFrequencyRank},
        rabinkarp,
    },
    cow::CowBytes,
    memmem::searcher::{PrefilterState, Searcher, SearcherRev},
};
mod searcher;
#[inline]
pub fn find_iter<'h, 'n, N: 'n + ?Sized + AsRef<[u8]>>(
    haystack: &'h [u8],
    needle: &'n N,
) -> FindIter<'h, 'n> {
    FindIter::new(haystack, Finder::new(needle))
}
#[inline]
pub fn rfind_iter<'h, 'n, N: 'n + ?Sized + AsRef<[u8]>>(
    haystack: &'h [u8],
    needle: &'n N,
) -> FindRevIter<'h, 'n> {
    FindRevIter::new(haystack, FinderRev::new(needle))
}
#[inline]
pub fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if haystack.len() < 64 {
        rabinkarp::Finder::new(needle).find(haystack, needle)
    } else {
        Finder::new(needle).find(haystack)
    }
}
#[inline]
pub fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if haystack.len() < 64 {
        rabinkarp::FinderRev::new(needle).rfind(haystack, needle)
    } else {
        FinderRev::new(needle).rfind(haystack)
    }
}
#[derive(Debug, Clone)]
pub struct FindIter<'h, 'n> {
    haystack: &'h [u8],
    prestate: PrefilterState,
    finder: Finder<'n>,
    pos: usize,
}
impl<'h, 'n> FindIter<'h, 'n> {
    #[inline(always)]
    pub(crate) fn new(haystack: &'h [u8], finder: Finder<'n>) -> FindIter<'h, 'n> {
        let prestate = PrefilterState::new();
        FindIter {
            haystack,
            prestate,
            finder,
            pos: 0,
        }
    }
    #[inline]
    pub fn into_owned(self) -> FindIter<'h, 'static> {
        FindIter {
            haystack: self.haystack,
            prestate: self.prestate,
            finder: self.finder.into_owned(),
            pos: self.pos,
        }
    }
}
impl<'h, 'n> Iterator for FindIter<'h, 'n> {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        let needle = self.finder.needle();
        let haystack = self.haystack.get(self.pos..)?;
        let idx = self
            .finder
            .searcher
            .find(&mut self.prestate, haystack, needle)?;
        let pos = self.pos + idx;
        self.pos = pos + needle.len().max(1);
        Some(pos)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.haystack.len().checked_sub(self.pos) {
            None => (0, Some(0)),
            Some(haystack_len) => match self.finder.needle().len() {
                0 => (haystack_len.saturating_add(1), haystack_len.checked_add(1)),
                needle_len => (0, Some(haystack_len / needle_len)),
            },
        }
    }
}
#[derive(Clone, Debug)]
pub struct FindRevIter<'h, 'n> {
    haystack: &'h [u8],
    finder: FinderRev<'n>,
    pos: Option<usize>,
}
impl<'h, 'n> FindRevIter<'h, 'n> {
    #[inline(always)]
    pub(crate) fn new(haystack: &'h [u8], finder: FinderRev<'n>) -> FindRevIter<'h, 'n> {
        let pos = Some(haystack.len());
        FindRevIter {
            haystack,
            finder,
            pos,
        }
    }
    #[inline]
    pub fn into_owned(self) -> FindRevIter<'h, 'static> {
        FindRevIter {
            haystack: self.haystack,
            finder: self.finder.into_owned(),
            pos: self.pos,
        }
    }
}
impl<'h, 'n> Iterator for FindRevIter<'h, 'n> {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        let pos = self.pos?;
        let result = self.finder.rfind(&self.haystack[..pos]);
        match result {
            None => None,
            Some(i) => {
                if pos == i {
                    self.pos = pos.checked_sub(1);
                } else {
                    self.pos = Some(i);
                }
                Some(i)
            }
        }
    }
}
#[derive(Clone, Debug)]
pub struct Finder<'n> {
    needle: CowBytes<'n>,
    searcher: Searcher,
}
impl<'n> Finder<'n> {
    #[inline]
    pub fn new<B: ?Sized + AsRef<[u8]>>(needle: &'n B) -> Finder<'n> {
        FinderBuilder::new().build_forward(needle)
    }
    #[inline]
    pub fn find(&self, haystack: &[u8]) -> Option<usize> {
        let mut prestate = PrefilterState::new();
        let needle = self.needle.as_slice();
        self.searcher.find(&mut prestate, haystack, needle)
    }
    #[inline]
    pub fn find_iter<'a, 'h>(&'a self, haystack: &'h [u8]) -> FindIter<'h, 'a> {
        FindIter::new(haystack, self.as_ref())
    }
    #[inline]
    pub fn into_owned(self) -> Finder<'static> {
        Finder {
            needle: self.needle.into_owned(),
            searcher: self.searcher,
        }
    }
    #[inline]
    pub fn as_ref(&self) -> Finder<'_> {
        Finder {
            needle: CowBytes::new(self.needle()),
            searcher: self.searcher.clone(),
        }
    }
    #[inline]
    pub fn needle(&self) -> &[u8] {
        self.needle.as_slice()
    }
}
#[derive(Clone, Debug)]
pub struct FinderRev<'n> {
    needle: CowBytes<'n>,
    searcher: SearcherRev,
}
impl<'n> FinderRev<'n> {
    #[inline]
    pub fn new<B: ?Sized + AsRef<[u8]>>(needle: &'n B) -> FinderRev<'n> {
        FinderBuilder::new().build_reverse(needle)
    }
    pub fn rfind<B: AsRef<[u8]>>(&self, haystack: B) -> Option<usize> {
        self.searcher
            .rfind(haystack.as_ref(), self.needle.as_slice())
    }
    #[inline]
    pub fn rfind_iter<'a, 'h>(&'a self, haystack: &'h [u8]) -> FindRevIter<'h, 'a> {
        FindRevIter::new(haystack, self.as_ref())
    }
    #[inline]
    pub fn into_owned(self) -> FinderRev<'static> {
        FinderRev {
            needle: self.needle.into_owned(),
            searcher: self.searcher,
        }
    }
    #[inline]
    pub fn as_ref(&self) -> FinderRev<'_> {
        FinderRev {
            needle: CowBytes::new(self.needle()),
            searcher: self.searcher.clone(),
        }
    }
    #[inline]
    pub fn needle(&self) -> &[u8] {
        self.needle.as_slice()
    }
}
#[derive(Clone, Debug, Default)]
pub struct FinderBuilder {
    prefilter: Prefilter,
}
impl FinderBuilder {
    pub fn new() -> FinderBuilder {
        FinderBuilder::default()
    }
    pub fn build_forward<'n, B: ?Sized + AsRef<[u8]>>(&self, needle: &'n B) -> Finder<'n> {
        self.build_forward_with_ranker(DefaultFrequencyRank, needle)
    }

    pub fn build_forward_with_ranker<'n, R: HeuristicFrequencyRank, B: ?Sized + AsRef<[u8]>>(
        &self,
        ranker: R,
        needle: &'n B,
    ) -> Finder<'n> {
        let needle = needle.as_ref();
        Finder {
            needle: CowBytes::new(needle),
            searcher: Searcher::new(self.prefilter, ranker, needle),
        }
    }
    pub fn build_forward_with_ranker_owned<
        R: HeuristicFrequencyRank,
        B: Into<alloc::boxed::Box<[u8]>>,
    >(
        &self,
        ranker: R,
        needle: B,
    ) -> Finder<'static> {
        let needle = needle.into();
        let searcher = Searcher::new(self.prefilter, ranker, &needle);
        Finder {
            needle: CowBytes::new_owned(needle),
            searcher,
        }
    }
    pub fn build_reverse<'n, B: ?Sized + AsRef<[u8]>>(&self, needle: &'n B) -> FinderRev<'n> {
        let needle = needle.as_ref();
        FinderRev {
            needle: CowBytes::new(needle),
            searcher: SearcherRev::new(needle),
        }
    }

    pub fn prefilter(&mut self, prefilter: Prefilter) -> &mut FinderBuilder {
        self.prefilter = prefilter;
        self
    }
}
