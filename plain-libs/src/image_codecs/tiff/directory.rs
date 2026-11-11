use core::fmt;
use std::{collections::BTreeMap, num::NonZeroU64};

use crate::image_codecs::tiff::{
    decoder::ifd::Entry,
    tags::{IfdPointer, Tag},
};
#[doc(alias = "IFD")]
pub struct Directory {
    pub(crate) entries: BTreeMap<u16, Entry>,
    pub(crate) next_ifd: Option<NonZeroU64>,
}

impl Directory {
    pub fn empty() -> Self {
        Directory {
            entries: BTreeMap::new(),
            next_ifd: None,
        }
    }
    pub fn get(&self, tag: Tag) -> Option<&Entry> {
        self.entries.get(&tag.to_u16())
    }
    pub fn contains(&self, tag: Tag) -> bool {
        self.entries.contains_key(&tag.to_u16())
    }
    pub fn iter(&self) -> impl Iterator<Item = (Tag, &Entry)> + '_ {
        self.entries
            .iter()
            .map(|(k, v)| (Tag::from_u16_exhaustive(*k), v))
    }
    pub fn extend(&mut self, iter: impl IntoIterator<Item = (Tag, Entry)>) {
        // Code size conscious extension, avoid monomorphic extensions with the assumption of these
        // not being performance sensitive in practice. (Maybe we have a polymorphic interface for
        // the crate usage in the future.
        self.extend_inner(iter.into_iter().by_ref())
    }
    pub fn len(&self) -> usize {
        // The keys are `u16`. Since IFDs are required to have at least one entry this would have
        // been a trivial thing to do in the specification by storing it minus one but alas. In
        // BigTIFF the count is stored as 8-bit anyways.
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn next(&self) -> Option<IfdPointer> {
        self.next_ifd.map(|n| IfdPointer(n.get()))
    }

    pub fn set_next(&mut self, next: Option<IfdPointer>) {
        self.next_ifd = next.and_then(|n| NonZeroU64::new(n.0));
    }

    fn extend_inner(&mut self, iter: &mut dyn Iterator<Item = (Tag, Entry)>) {
        for (tag, entry) in iter {
            // If the tag is already present, it will be overwritten.
            let map_entry = self.entries.entry(tag.to_u16());

            match map_entry {
                std::collections::btree_map::Entry::Vacant(vacant_entry) => {
                    vacant_entry.insert(entry);
                }
                std::collections::btree_map::Entry::Occupied(mut occupied_entry) => {
                    occupied_entry.insert(entry);
                }
            }
        }
    }
}

impl fmt::Debug for Directory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Directory")
            .field(
                "entries",
                &self.entries.iter().map(|(k, v)| (Tag::from_u16(*k), v)),
            )
            .field("next_ifd", &self.next_ifd)
            .finish()
    }
}

impl core::iter::FromIterator<(Tag, Entry)> for Directory {
    fn from_iter<T: IntoIterator<Item = (Tag, Entry)>>(iter: T) -> Self {
        let mut dir = Directory::empty();
        dir.extend(iter);
        dir
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/image_codecs/tiff/directory.rs"]
mod tests;
