use crate::regex_engine::automata::{
    nfa::thompson::Transition,
    util::{
        int::{U64, Usize},
        primitives::StateID,
    },
};
use std::{vec, vec::Vec};
const PRIME: u64 = 1099511628211;
const INIT: u64 = 14695981039346656037;
#[derive(Clone, Debug)]
pub struct Utf8BoundedMap {
    version: u16,
    capacity: usize,
    map: Vec<Utf8BoundedEntry>,
}
#[derive(Clone, Debug, Default)]
struct Utf8BoundedEntry {
    version: u16,
    key: Vec<Transition>,
    val: StateID,
}
impl Utf8BoundedMap {
    pub fn new(capacity: usize) -> Utf8BoundedMap {
        assert!(capacity > 0);
        Utf8BoundedMap {
            version: 0,
            capacity,
            map: vec![],
        }
    }
    pub fn clear(&mut self) {
        if self.map.is_empty() {
            self.map = vec![Utf8BoundedEntry::default(); self.capacity];
        } else {
            self.version = self.version.wrapping_add(1);
            if self.version == 0 {
                self.map = vec![Utf8BoundedEntry::default(); self.capacity];
            }
        }
    }
    pub fn hash(&self, key: &[Transition]) -> usize {
        let mut h = INIT;
        for t in key {
            h = (h ^ u64::from(t.start)).wrapping_mul(PRIME);
            h = (h ^ u64::from(t.end)).wrapping_mul(PRIME);
            h = (h ^ t.next.as_u64()).wrapping_mul(PRIME);
        }
        (h % self.map.len().as_u64()).as_usize()
    }
    pub fn get(&mut self, key: &[Transition], hash: usize) -> Option<StateID> {
        let entry = &self.map[hash];
        if entry.version != self.version {
            return None;
        }
        if entry.key != key {
            return None;
        }
        Some(entry.val)
    }
    pub fn set(&mut self, key: Vec<Transition>, hash: usize, state_id: StateID) {
        self.map[hash] = Utf8BoundedEntry {
            version: self.version,
            key,
            val: state_id,
        };
    }
}
#[derive(Clone, Debug)]
pub struct Utf8SuffixMap {
    version: u16,
    capacity: usize,
    map: Vec<Utf8SuffixEntry>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Utf8SuffixKey {
    pub from: StateID,
    pub start: u8,
    pub end: u8,
}
#[derive(Clone, Debug, Default)]
struct Utf8SuffixEntry {
    version: u16,
    key: Utf8SuffixKey,
    val: StateID,
}
impl Utf8SuffixMap {
    pub fn new(capacity: usize) -> Utf8SuffixMap {
        assert!(capacity > 0);
        Utf8SuffixMap {
            version: 0,
            capacity,
            map: vec![],
        }
    }
    pub fn clear(&mut self) {
        if self.map.is_empty() {
            self.map = vec![Utf8SuffixEntry::default(); self.capacity];
        } else {
            self.version = self.version.wrapping_add(1);
            if self.version == 0 {
                self.map = vec![Utf8SuffixEntry::default(); self.capacity];
            }
        }
    }
    pub fn hash(&self, key: &Utf8SuffixKey) -> usize {
        const PRIME: u64 = 1099511628211;
        const INIT: u64 = 14695981039346656037;
        let mut h = INIT;
        h = (h ^ key.from.as_u64()).wrapping_mul(PRIME);
        h = (h ^ u64::from(key.start)).wrapping_mul(PRIME);
        h = (h ^ u64::from(key.end)).wrapping_mul(PRIME);
        (h % self.map.len().as_u64()).as_usize()
    }
    pub fn get(&mut self, key: &Utf8SuffixKey, hash: usize) -> Option<StateID> {
        let entry = &self.map[hash];
        if entry.version != self.version {
            return None;
        }
        if key != &entry.key {
            return None;
        }
        Some(entry.val)
    }
    pub fn set(&mut self, key: Utf8SuffixKey, hash: usize, state_id: StateID) {
        self.map[hash] = Utf8SuffixEntry {
            version: self.version,
            key,
            val: state_id,
        };
    }
}
