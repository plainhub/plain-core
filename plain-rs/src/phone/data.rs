use flate2::read::GzDecoder;
use serde::Deserialize;
use std::collections::{BTreeMap, VecDeque};
use std::io::Read;
use std::sync::{Arc, LazyLock, Mutex};

const BUNDLE: &[u8] = include_bytes!("data/metadata.bin");
const CACHE_BYTES: usize = 8 * 1024 * 1024;
const CACHE_ENTRIES: usize = 32;

#[derive(Deserialize)]
struct Index {
    entries: BTreeMap<String, [usize; 2]>,
}

static INDEX: LazyLock<(Index, usize)> = LazyLock::new(|| {
    assert_eq!(&BUNDLE[..4], b"PHN1");
    let start = 8 + u32::from_be_bytes(BUNDLE[4..8].try_into().unwrap()) as usize;
    let index: Index =
        serde_json::from_slice(&BUNDLE[8..start]).expect("invalid bundled phone index");
    for [offset, length] in index.entries.values() {
        assert!(*offset <= BUNDLE.len() - start && *length <= BUNDLE.len() - start - offset);
    }
    (index, start)
});

#[derive(Default)]
pub(super) struct Data {
    cache: Mutex<Cache>,
}

#[derive(Default)]
struct Cache {
    entries: VecDeque<([usize; 2], Arc<Map>)>,
    bytes: usize,
}

pub(super) enum Map {
    Prefixes {
        entries: Vec<(u32, u32)>,
        descriptions: Vec<String>,
        lengths: Vec<u32>,
    },
    Countries(Vec<(String, String)>),
}

impl Data {
    pub fn contains(&self, key: &str) -> bool {
        INDEX.0.entries.contains_key(key)
    }

    pub fn get(&self, key: &str) -> Option<Arc<Map>> {
        let range = *INDEX.0.entries.get(key)?;
        let mut cache = self.cache.lock().unwrap();
        if let Some(index) = cache
            .entries
            .iter()
            .position(|(cached, _)| *cached == range)
        {
            let entry = cache.entries.remove(index).unwrap();
            let result = entry.1.clone();
            cache.entries.push_back(entry);
            return Some(result);
        }
        let start = INDEX.1 + range[0];
        let mut raw = Vec::new();
        GzDecoder::new(&BUNDLE[start..start + range[1]])
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut raw)
            .expect("invalid bundled phone compression");
        assert!(raw.len() <= 16 * 1024 * 1024, "phone map exceeds limit");
        let map = Arc::new(Map::decode(&raw).expect("invalid bundled phone map"));
        let bytes = map.bytes();
        if bytes <= CACHE_BYTES {
            while cache.bytes + bytes > CACHE_BYTES || cache.entries.len() >= CACHE_ENTRIES {
                let (_, removed) = cache.entries.pop_front().unwrap();
                cache.bytes -= removed.bytes();
            }
            cache.bytes += bytes;
            cache.entries.push_back((range, map.clone()));
        }
        Some(map)
    }
}

impl Map {
    fn decode(raw: &[u8]) -> Result<Self, &'static str> {
        let mut reader = Reader(raw);
        let magic = reader.take(4)?;
        let count = reader.integer()? as usize;
        if count > raw.len() / 8 {
            return Err("invalid phone map count");
        }
        let map = match magic {
            b"PFX1" => {
                let entries = (0..count)
                    .map(|_| Ok((reader.integer()?, reader.integer()?)))
                    .collect::<Result<Vec<_>, &'static str>>()?;
                let count = reader.integer()? as usize;
                if count > reader.0.len() / 4 {
                    return Err("invalid description count");
                }
                let descriptions = (0..count)
                    .map(|_| reader.text())
                    .collect::<Result<Vec<_>, _>>()?;
                if !entries.windows(2).all(|pair| pair[0].0 < pair[1].0)
                    || entries.iter().any(|(prefix, index)| {
                        *prefix == 0 || *index as usize >= descriptions.len()
                    })
                {
                    return Err("invalid phone prefixes");
                }
                let mut lengths = Vec::new();
                for (prefix, _) in &entries {
                    let length = prefix.ilog10() + 1;
                    if !lengths.contains(&length) {
                        lengths.push(length);
                    }
                }
                lengths.sort_unstable();
                Self::Prefixes {
                    entries,
                    descriptions,
                    lengths,
                }
            }
            b"CTY1" => {
                let entries = (0..count)
                    .map(|_| Ok((reader.text()?, reader.text()?)))
                    .collect::<Result<Vec<_>, &'static str>>()?;
                if !entries.windows(2).all(|pair| pair[0].0 < pair[1].0) {
                    return Err("invalid country order");
                }
                Self::Countries(entries)
            }
            _ => return Err("invalid phone map type"),
        };
        if !reader.0.is_empty() {
            return Err("unexpected phone map bytes");
        }
        Ok(map)
    }

    pub fn prefix(&self, number: u64) -> &str {
        let Self::Prefixes {
            entries,
            descriptions,
            lengths,
        } = self
        else {
            return "";
        };
        if number == 0 {
            return "";
        }
        let digits = number.ilog10() + 1;
        for length in lengths.iter().rev().filter(|length| **length <= digits) {
            let prefix = number / 10_u64.pow(digits - length);
            let Ok(prefix) = u32::try_from(prefix) else {
                continue;
            };
            if let Ok(index) = entries.binary_search_by_key(&prefix, |entry| entry.0) {
                return &descriptions[entries[index].1 as usize];
            }
        }
        ""
    }

    pub fn country(&self, region: &str) -> &str {
        let Self::Countries(entries) = self else {
            return "";
        };
        entries
            .binary_search_by(|entry| entry.0.as_str().cmp(region))
            .map(|index| entries[index].1.as_str())
            .unwrap_or_default()
    }

    fn bytes(&self) -> usize {
        match self {
            Self::Prefixes {
                entries,
                descriptions,
                lengths,
            } => {
                entries.capacity() * std::mem::size_of::<(u32, u32)>()
                    + descriptions.capacity() * std::mem::size_of::<String>()
                    + descriptions.iter().map(String::capacity).sum::<usize>()
                    + lengths.capacity() * std::mem::size_of::<u32>()
            }
            Self::Countries(entries) => {
                entries.capacity() * std::mem::size_of::<(String, String)>()
                    + entries
                        .iter()
                        .map(|(key, value)| key.capacity() + value.capacity())
                        .sum::<usize>()
            }
        }
    }
}

struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], &'static str> {
        let (value, rest) = self
            .0
            .split_at_checked(count)
            .ok_or("incomplete phone map")?;
        self.0 = rest;
        Ok(value)
    }
    fn integer(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn text(&mut self) -> Result<String, &'static str> {
        let length = self.integer()? as usize;
        String::from_utf8(self.take(length)?.to_vec())
            .map_err(|_| "invalid phone description UTF-8")
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-rs/unit/phone/data.rs"]
mod tests;
