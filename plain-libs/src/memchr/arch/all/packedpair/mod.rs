use crate::memchr::memchr;
mod default_rank;
#[derive(Clone, Copy, Debug)]
pub struct Finder {
    pair: Pair,
    byte1: u8,
    byte2: u8,
}
impl Finder {
    #[inline]
    pub fn new(needle: &[u8]) -> Option<Finder> {
        Finder::with_pair(needle, Pair::new(needle)?)
    }
    #[inline]
    pub fn with_pair(needle: &[u8], pair: Pair) -> Option<Finder> {
        let byte1 = needle[usize::from(pair.index1())];
        let byte2 = needle[usize::from(pair.index2())];
        Some(Finder { pair, byte1, byte2 })
    }
    #[inline]
    pub fn find_prefilter(&self, haystack: &[u8]) -> Option<usize> {
        let mut i = 0;
        let index1 = usize::from(self.pair.index1());
        let index2 = usize::from(self.pair.index2());
        loop {
            i += memchr(self.byte1, &haystack[i..])?;
            let found = i;
            i += 1;
            let aligned1 = match found.checked_sub(index1) {
                None => continue,
                Some(aligned1) => aligned1,
            };
            let aligned2 = match aligned1.checked_add(index2) {
                None => continue,
                Some(aligned_index2) => aligned_index2,
            };
            if haystack.get(aligned2).map_or(true, |&b| b != self.byte2) {
                continue;
            }
            return Some(aligned1);
        }
    }
    #[inline]
    pub fn pair(&self) -> &Pair {
        &self.pair
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Pair {
    index1: u8,
    index2: u8,
}
impl Pair {
    #[inline]
    pub fn new(needle: &[u8]) -> Option<Pair> {
        Pair::with_ranker(needle, DefaultFrequencyRank)
    }
    #[inline]
    pub fn with_ranker<R: HeuristicFrequencyRank>(needle: &[u8], ranker: R) -> Option<Pair> {
        if needle.len() <= 1 {
            return None;
        }
        let (mut rare1, mut index1) = (needle[0], 0);
        let (mut rare2, mut index2) = (needle[1], 1);
        if ranker.rank(rare2) < ranker.rank(rare1) {
            core::mem::swap(&mut rare1, &mut rare2);
            core::mem::swap(&mut index1, &mut index2);
        }
        let max = usize::from(u8::MAX);
        for (i, &b) in needle.iter().enumerate().take(max).skip(2) {
            if ranker.rank(b) < ranker.rank(rare1) {
                rare2 = rare1;
                index2 = index1;
                rare1 = b;
                index1 = u8::try_from(i).unwrap();
            } else if b != rare1 && ranker.rank(b) < ranker.rank(rare2) {
                rare2 = b;
                index2 = u8::try_from(i).unwrap();
            }
        }
        assert_ne!(index1, index2);
        Some(Pair { index1, index2 })
    }

    #[inline]
    pub fn index1(&self) -> u8 {
        self.index1
    }
    #[inline]
    pub fn index2(&self) -> u8 {
        self.index2
    }
}
pub trait HeuristicFrequencyRank {
    fn rank(&self, byte: u8) -> u8;
}
pub(crate) struct DefaultFrequencyRank;
impl HeuristicFrequencyRank for DefaultFrequencyRank {
    fn rank(&self, byte: u8) -> u8 {
        self::default_rank::RANK[usize::from(byte)]
    }
}
impl<'a, R> HeuristicFrequencyRank for &'a R
where
    R: HeuristicFrequencyRank,
{
    fn rank(&self, byte: u8) -> u8 {
        (**self).rank(byte)
    }
}
