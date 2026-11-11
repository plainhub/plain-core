use crate::regex_engine::aho::{PatternID, packed::pattern::Patterns, util::search::Match};
use std::{sync::Arc, vec, vec::Vec};
type Hash = usize;
const NUM_BUCKETS: usize = 64;
#[derive(Clone, Debug)]
pub(crate) struct RabinKarp {
    patterns: Arc<Patterns>,
    buckets: Vec<Vec<(Hash, PatternID)>>,
    hash_len: usize,
    hash_2pow: usize,
}
impl RabinKarp {
    pub(crate) fn new(patterns: &Arc<Patterns>) -> RabinKarp {
        assert!(patterns.len() >= 1);
        let hash_len = patterns.minimum_len();
        assert!(hash_len >= 1);
        let mut hash_2pow = 1usize;
        for _ in 1..hash_len {
            hash_2pow = hash_2pow.wrapping_shl(1);
        }
        let mut rk = RabinKarp {
            patterns: Arc::clone(patterns),
            buckets: vec![vec![]; NUM_BUCKETS],
            hash_len,
            hash_2pow,
        };
        for (id, pat) in patterns.iter() {
            let hash = rk.hash(&pat.bytes()[..rk.hash_len]);
            let bucket = hash % NUM_BUCKETS;
            rk.buckets[bucket].push((hash, id));
        }
        rk
    }
    pub(crate) fn find_at(&self, haystack: &[u8], mut at: usize) -> Option<Match> {
        assert_eq!(NUM_BUCKETS, self.buckets.len());
        if at + self.hash_len > haystack.len() {
            return None;
        }
        let mut hash = self.hash(&haystack[at..at + self.hash_len]);
        loop {
            let bucket = &self.buckets[hash % NUM_BUCKETS];
            for &(phash, pid) in bucket {
                if phash == hash {
                    if let Some(c) = self.verify(pid, haystack, at) {
                        return Some(c);
                    }
                }
            }
            if at + self.hash_len >= haystack.len() {
                return None;
            }
            hash = self.update_hash(hash, haystack[at], haystack[at + self.hash_len]);
            at += 1;
        }
    }
    pub(crate) fn memory_usage(&self) -> usize {
        self.buckets.len() * core::mem::size_of::<Vec<(Hash, PatternID)>>()
            + self.patterns.len() * core::mem::size_of::<(Hash, PatternID)>()
    }
    #[cold]
    fn verify(&self, id: PatternID, haystack: &[u8], at: usize) -> Option<Match> {
        let pat = self.patterns.get(id);
        if pat.is_prefix(&haystack[at..]) {
            Some(Match::new(id, at..at + pat.len()))
        } else {
            None
        }
    }
    fn hash(&self, bytes: &[u8]) -> Hash {
        assert_eq!(self.hash_len, bytes.len());
        let mut hash = 0usize;
        for &b in bytes {
            hash = hash.wrapping_shl(1).wrapping_add(b as usize);
        }
        hash
    }
    fn update_hash(&self, prev: Hash, old_byte: u8, new_byte: u8) -> Hash {
        prev.wrapping_sub((old_byte as usize).wrapping_mul(self.hash_2pow))
            .wrapping_shl(1)
            .wrapping_add(new_byte as usize)
    }
}
