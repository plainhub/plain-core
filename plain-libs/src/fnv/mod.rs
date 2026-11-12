use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

// Adapted from fnv1.0.7 and the existing HTTP header hasher; see SOURCE.json.
pub struct FnvHasher(u64);

impl Default for FnvHasher {
    #[inline(always)]
    fn default() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl Hasher for FnvHasher {
    #[inline(always)]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) {
        let mut hash = self.0;
        for &byte in bytes {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        self.0 = hash;
    }
}

pub type FnvHashMap<K, V> = HashMap<K, V, BuildHasherDefault<FnvHasher>>;
pub type FnvHashSet<T> = HashSet<T, BuildHasherDefault<FnvHasher>>;

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/fnv.rs"]
mod tests;
