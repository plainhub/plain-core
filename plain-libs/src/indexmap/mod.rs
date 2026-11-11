#[macro_use]
mod arbitrary;
mod inner;
#[macro_use]
mod macros;
pub mod map;
mod serde;
pub mod set;
mod util;
pub use crate::equivalent::Equivalent;
pub use crate::indexmap::map::IndexMap;
pub use crate::indexmap::set::IndexSet;
#[derive(Clone, Copy, Debug, PartialEq)]
struct HashValue(usize);
impl HashValue {
    #[inline(always)]
    fn get(self) -> u64 {
        self.0 as u64
    }
}
#[derive(Copy, Debug)]
struct Bucket<K, V> {
    hash: HashValue,
    key: K,
    value: V,
}
impl<K, V> Clone for Bucket<K, V>
where
    K: Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        Bucket {
            hash: self.hash,
            key: self.key.clone(),
            value: self.value.clone(),
        }
    }
    fn clone_from(&mut self, other: &Self) {
        self.hash = other.hash;
        self.key.clone_from(&other.key);
        self.value.clone_from(&other.value);
    }
}
impl<K, V> Bucket<K, V> {
    const fn key_ref(&self) -> &K {
        &self.key
    }
    const fn value_ref(&self) -> &V {
        &self.value
    }
    const fn value_mut(&mut self) -> &mut V {
        &mut self.value
    }
    fn key(self) -> K {
        self.key
    }
    fn value(self) -> V {
        self.value
    }
    fn key_value(self) -> (K, V) {
        (self.key, self.value)
    }
    const fn refs(&self) -> (&K, &V) {
        (&self.key, &self.value)
    }
    const fn ref_mut(&mut self) -> (&K, &mut V) {
        (&self.key, &mut self.value)
    }
    const fn muts(&mut self) -> (&mut K, &mut V) {
        (&mut self.key, &mut self.value)
    }
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TryReserveError {
    kind: TryReserveErrorKind,
}
#[derive(Clone, PartialEq, Eq, Debug)]
enum TryReserveErrorKind {
    Std(alloc::collections::TryReserveError),
    CapacityOverflow,
    AllocError { layout: alloc::alloc::Layout },
}
impl TryReserveError {
    fn from_alloc(error: alloc::collections::TryReserveError) -> Self {
        Self {
            kind: TryReserveErrorKind::Std(error),
        }
    }
    fn from_hashbrown(error: crate::hashbrown::TryReserveError) -> Self {
        Self {
            kind: match error {
                crate::hashbrown::TryReserveError::CapacityOverflow => {
                    TryReserveErrorKind::CapacityOverflow
                }
                crate::hashbrown::TryReserveError::AllocError { layout } => {
                    TryReserveErrorKind::AllocError { layout }
                }
            },
        }
    }
}
impl core::fmt::Display for TryReserveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let reason = match &self.kind {
            TryReserveErrorKind::Std(e) => return core::fmt::Display::fmt(e, f),
            TryReserveErrorKind::CapacityOverflow => {
                " because the computed capacity exceeded the collection's maximum"
            }
            TryReserveErrorKind::AllocError { .. } => {
                " because the memory allocator returned an error"
            }
        };
        f.write_str("memory allocation failed")?;
        f.write_str(reason)
    }
}
impl core::error::Error for TryReserveError {}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetDisjointMutError {
    IndexOutOfBounds,
    OverlappingIndices,
}
impl core::fmt::Display for GetDisjointMutError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            GetDisjointMutError::IndexOutOfBounds => "an index is out of bounds",
            GetDisjointMutError::OverlappingIndices => "there were overlapping indices",
        };
        core::fmt::Display::fmt(msg, f)
    }
}
impl core::error::Error for GetDisjointMutError {}
pub use crate::{
    plain_indexmap_indexmap as indexmap,
    plain_indexmap_indexmap_with_default as indexmap_with_default,
    plain_indexmap_indexset as indexset,
    plain_indexmap_indexset_with_default as indexset_with_default,
};

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/indexmap.rs"]
mod tests;
