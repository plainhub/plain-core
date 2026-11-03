use bytes::Bytes;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

struct Entry {
    key: PathBuf,
    data: Bytes,
    previous: Option<usize>,
    next: Option<usize>,
}

struct Inner {
    map: HashMap<PathBuf, usize>,
    entries: Vec<Option<Entry>>,
    free: Vec<usize>,
    oldest: Option<usize>,
    newest: Option<usize>,
    bytes: usize,
    #[cfg(test)]
    eviction_probes: std::cell::Cell<usize>,
}

impl Inner {
    fn unlink(&mut self, index: usize) {
        let entry = self.entries[index].as_ref().unwrap();
        let (previous, next) = (entry.previous, entry.next);
        if let Some(previous) = previous {
            self.entries[previous].as_mut().unwrap().next = next;
        } else {
            self.oldest = next;
        }
        if let Some(next) = next {
            self.entries[next].as_mut().unwrap().previous = previous;
        } else {
            self.newest = previous;
        }
    }

    fn link_recent(&mut self, index: usize) {
        let entry = self.entries[index].as_mut().unwrap();
        entry.previous = self.newest;
        entry.next = None;
        if let Some(previous) = self.newest {
            self.entries[previous].as_mut().unwrap().next = Some(index);
        } else {
            self.oldest = Some(index);
        }
        self.newest = Some(index);
    }

    fn touch(&mut self, index: usize) {
        if self.newest != Some(index) {
            self.unlink(index);
            self.link_recent(index);
        }
    }
}

pub struct ThumbLru {
    cap_bytes: usize,
    cap_entries: usize,
    inner: Mutex<Inner>,
}

impl ThumbLru {
    pub fn new(cap_bytes: usize, cap_entries: usize) -> Self {
        Self {
            cap_bytes: cap_bytes.max(1),
            cap_entries: cap_entries.max(1),
            inner: Mutex::new(Inner {
                map: HashMap::new(),
                entries: Vec::new(),
                free: Vec::new(),
                oldest: None,
                newest: None,
                bytes: 0,
                #[cfg(test)]
                eviction_probes: std::cell::Cell::new(0),
            }),
        }
    }

    pub fn get(&self, key: &PathBuf) -> Option<Bytes> {
        let mut inner = self.inner.lock().unwrap();
        let index = *inner.map.get(key)?;
        inner.touch(index);
        Some(inner.entries[index].as_ref().unwrap().data.clone())
    }

    pub fn put(&self, key: PathBuf, data: Bytes) {
        let mut inner = self.inner.lock().unwrap();
        let size = data.len();
        let keep = if let Some(&index) = inner.map.get(&key) {
            let old = std::mem::replace(&mut inner.entries[index].as_mut().unwrap().data, data);
            inner.bytes -= old.len();
            inner.touch(index);
            index
        } else {
            let index = inner.free.pop().unwrap_or_else(|| {
                inner.entries.push(None);
                inner.entries.len() - 1
            });
            inner.map.insert(key.clone(), index);
            inner.entries[index] = Some(Entry {
                key,
                data,
                previous: None,
                next: None,
            });
            inner.link_recent(index);
            index
        };
        inner.bytes += size;
        while (inner.map.len() > self.cap_entries || inner.bytes > self.cap_bytes)
            && inner.oldest != Some(keep)
        {
            let victim = inner.oldest.unwrap();
            #[cfg(test)]
            inner.eviction_probes.set(inner.eviction_probes.get() + 1);
            inner.unlink(victim);
            let entry = inner.entries[victim].take().unwrap();
            inner.map.remove(&entry.key);
            inner.bytes -= entry.data.len();
            inner.free.push(victim);
        }
    }

    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.map.clear();
        inner.entries.clear();
        inner.free.clear();
        inner.oldest = None;
        inner.newest = None;
        inner.bytes = 0;
        #[cfg(test)]
        inner.eviction_probes.set(0);
    }

    #[cfg(test)]
    pub fn stats(&self) -> (usize, usize) {
        let inner = self.inner.lock().unwrap();
        (inner.map.len(), inner.bytes)
    }
}

static GLOBAL: std::sync::OnceLock<ThumbLru> = std::sync::OnceLock::new();

pub fn initialize(raw: i64) {
    // Missing/invalid key (get_int → 0) falls back to the 32 MB default.
    let mb = if raw <= 0 { 32 } else { raw.clamp(1, 2048) } as usize;
    let _ = GLOBAL.set(ThumbLru::new(mb * 1024 * 1024, 4096));
}

pub fn global() -> &'static ThumbLru {
    GLOBAL.get_or_init(|| ThumbLru::new(32 * 1024 * 1024, 4096))
}

/// Test/bench hook: drop the hot cache.
#[doc(hidden)]
pub fn debug_clear() {
    global().clear();
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/media/thumbnails/lru.rs"]
mod tests;
