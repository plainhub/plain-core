use super::*;
use crate::indexmap::{self, IndexMap};
use std::convert::Infallible;
use std::fmt;
use std::marker::PhantomData;
use std::ops;
#[derive(Debug)]
pub(super) struct Store {
    slab: slab::Slab<Stream>,
    ids: IndexMap<StreamId, SlabIndex>,
}
pub(super) struct Ptr<'a> {
    key: Key,
    store: &'a mut Store,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Key {
    index: SlabIndex,
    stream_id: StreamId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SlabIndex(u32);
pub(super) struct Queue<N> {
    indices: Option<store::Indices>,
    _p: PhantomData<N>,
}
pub(super) trait Next {
    fn next(stream: &Stream) -> Option<Key>;
    fn set_next(stream: &mut Stream, key: Option<Key>);
    fn take_next(stream: &mut Stream) -> Option<Key>;
    fn is_queued(stream: &Stream) -> bool;
    fn set_queued(stream: &mut Stream, val: bool);
}
#[derive(Debug, Clone, Copy)]
struct Indices {
    pub head: Key,
    pub tail: Key,
}
pub(super) enum Entry<'a> {
    Occupied(OccupiedEntry<'a>),
    Vacant(VacantEntry<'a>),
}
pub(super) struct OccupiedEntry<'a> {
    ids: crate::indexmap::map::OccupiedEntry<'a, StreamId, SlabIndex>,
}
pub(super) struct VacantEntry<'a> {
    ids: crate::indexmap::map::VacantEntry<'a, StreamId, SlabIndex>,
    slab: &'a mut slab::Slab<Stream>,
}
pub(super) trait Resolve {
    fn resolve(&mut self, key: Key) -> Ptr<'_>;
}
impl Store {
    pub fn new() -> Self {
        Store {
            slab: slab::Slab::new(),
            ids: IndexMap::new(),
        }
    }
    pub fn find_mut(&mut self, id: &StreamId) -> Option<Ptr<'_>> {
        let index = match self.ids.get(id) {
            Some(key) => *key,
            None => return None,
        };
        Some(Ptr {
            key: Key {
                index,
                stream_id: *id,
            },
            store: self,
        })
    }
    pub fn insert(&mut self, id: StreamId, val: Stream) -> Ptr<'_> {
        let index = SlabIndex(self.slab.insert(val) as u32);
        assert!(self.ids.insert(id, index).is_none());
        Ptr {
            key: Key {
                index,
                stream_id: id,
            },
            store: self,
        }
    }
    pub fn find_entry(&mut self, id: StreamId) -> Entry<'_> {
        use self::indexmap::map::Entry::*;
        match self.ids.entry(id) {
            Occupied(e) => Entry::Occupied(OccupiedEntry { ids: e }),
            Vacant(e) => Entry::Vacant(VacantEntry {
                ids: e,
                slab: &mut self.slab,
            }),
        }
    }
    pub(crate) fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(Ptr),
    {
        match self.try_for_each(|ptr| {
            f(ptr);
            Ok::<_, Infallible>(())
        }) {
            Ok(()) => (),
            Err(infallible) => match infallible {},
        }
    }
    pub fn try_for_each<F, E>(&mut self, mut f: F) -> Result<(), E>
    where
        F: FnMut(Ptr) -> Result<(), E>,
    {
        let mut len = self.ids.len();
        let mut i = 0;
        while i < len {
            let (stream_id, index) = {
                let entry = self.ids.get_index(i).unwrap();
                (*entry.0, *entry.1)
            };
            f(Ptr {
                key: Key { index, stream_id },
                store: self,
            })?;
            let new_len = self.ids.len();
            if new_len < len {
                debug_assert!(new_len == len - 1);
                len -= 1;
            } else {
                i += 1;
            }
        }
        Ok(())
    }
}
impl Resolve for Store {
    fn resolve(&mut self, key: Key) -> Ptr<'_> {
        Ptr { key, store: self }
    }
}
impl ops::Index<Key> for Store {
    type Output = Stream;
    fn index(&self, key: Key) -> &Self::Output {
        self.slab
            .get(key.index.0 as usize)
            .filter(|s| s.id == key.stream_id)
            .unwrap_or_else(|| {
                panic!("dangling store key for stream_id={:?}", key.stream_id);
            })
    }
}
impl ops::IndexMut<Key> for Store {
    fn index_mut(&mut self, key: Key) -> &mut Self::Output {
        self.slab
            .get_mut(key.index.0 as usize)
            .filter(|s| s.id == key.stream_id)
            .unwrap_or_else(|| {
                panic!("dangling store key for stream_id={:?}", key.stream_id);
            })
    }
}
impl Store {}
impl<N> Queue<N>
where
    N: Next,
{
    pub fn new() -> Self {
        Queue {
            indices: None,
            _p: PhantomData,
        }
    }
    pub fn take(&mut self) -> Self {
        Queue {
            indices: self.indices.take(),
            _p: PhantomData,
        }
    }
    pub fn push(&mut self, stream: &mut store::Ptr) -> bool {
        tracing::trace!("Queue::push_back");
        if N::is_queued(stream) {
            tracing::trace!(" -> already queued");
            return false;
        }
        N::set_queued(stream, true);
        debug_assert!(N::next(stream).is_none());
        match self.indices {
            Some(ref mut idxs) => {
                tracing::trace!(" -> existing entries");
                let key = stream.key();
                N::set_next(&mut stream.resolve(idxs.tail), Some(key));
                idxs.tail = stream.key();
            }
            None => {
                tracing::trace!(" -> first entry");
                self.indices = Some(store::Indices {
                    head: stream.key(),
                    tail: stream.key(),
                });
            }
        }
        true
    }
    pub fn push_front(&mut self, stream: &mut store::Ptr) -> bool {
        tracing::trace!("Queue::push_front");
        if N::is_queued(stream) {
            tracing::trace!(" -> already queued");
            return false;
        }
        N::set_queued(stream, true);
        debug_assert!(N::next(stream).is_none());
        match self.indices {
            Some(ref mut idxs) => {
                tracing::trace!(" -> existing entries");
                let head_key = stream.resolve(idxs.head).key();
                N::set_next(stream, Some(head_key));
                idxs.head = stream.key();
            }
            None => {
                tracing::trace!(" -> first entry");
                self.indices = Some(store::Indices {
                    head: stream.key(),
                    tail: stream.key(),
                });
            }
        }
        true
    }
    pub fn pop<'a, R>(&mut self, store: &'a mut R) -> Option<store::Ptr<'a>>
    where
        R: Resolve,
    {
        if let Some(mut idxs) = self.indices {
            let mut stream = store.resolve(idxs.head);
            if idxs.head == idxs.tail {
                assert!(N::next(&stream).is_none());
                self.indices = None;
            } else {
                idxs.head = N::take_next(&mut stream).unwrap();
                self.indices = Some(idxs);
            }
            debug_assert!(N::is_queued(&stream));
            N::set_queued(&mut stream, false);
            return Some(stream);
        }
        None
    }
    pub fn is_empty(&self) -> bool {
        self.indices.is_none()
    }
    pub fn pop_if<'a, R, F>(&mut self, store: &'a mut R, f: F) -> Option<store::Ptr<'a>>
    where
        R: Resolve,
        F: Fn(&Stream) -> bool,
    {
        if let Some(idxs) = self.indices {
            let should_pop = f(&store.resolve(idxs.head));
            if should_pop {
                return self.pop(store);
            }
        }
        None
    }
}
impl<N> fmt::Debug for Queue<N> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Queue")
            .field("indices", &self.indices)
            .finish()
    }
}
impl<'a> Ptr<'a> {
    pub fn key(&self) -> Key {
        self.key
    }
    pub fn store_mut(&mut self) -> &mut Store {
        self.store
    }
    pub fn remove(self) -> StreamId {
        debug_assert!(!self.store.ids.contains_key(&self.key.stream_id));
        let stream = self.store.slab.remove(self.key.index.0 as usize);
        assert_eq!(stream.id, self.key.stream_id);
        stream.id
    }
    pub fn unlink(&mut self) {
        let id = self.key.stream_id;
        self.store.ids.swap_remove(&id);
    }
}
impl<'a> Resolve for Ptr<'a> {
    fn resolve(&mut self, key: Key) -> Ptr<'_> {
        Ptr {
            key,
            store: &mut *self.store,
        }
    }
}
impl<'a> ops::Deref for Ptr<'a> {
    type Target = Stream;
    fn deref(&self) -> &Stream {
        &self.store[self.key]
    }
}
impl<'a> ops::DerefMut for Ptr<'a> {
    fn deref_mut(&mut self) -> &mut Stream {
        &mut self.store[self.key]
    }
}
impl<'a> fmt::Debug for Ptr<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt(fmt)
    }
}
impl<'a> OccupiedEntry<'a> {
    pub fn key(&self) -> Key {
        let stream_id = *self.ids.key();
        let index = *self.ids.get();
        Key { index, stream_id }
    }
}
impl<'a> VacantEntry<'a> {
    pub fn insert(self, value: Stream) -> Key {
        let stream_id = value.id;
        let index = SlabIndex(self.slab.insert(value) as u32);
        self.ids.insert(index);
        Key { index, stream_id }
    }
}
