use super::super::{atoms, child, u32_at};
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub(super) struct Defaults {
    pub description: u32,
    pub(super) duration: u32,
    pub(super) size: u32,
    pub(super) flags: u32,
}

pub(super) fn read(movie: &[u8]) -> Option<HashMap<u32, Defaults>> {
    let mut result = HashMap::new();
    if let Some(mvex) = child(movie, b"mvex") {
        for entry in atoms(mvex)?.into_iter().filter(|a| &a.kind == b"trex") {
            if u32_at(entry.data, 0)? != 0 {
                return None;
            }
            result.insert(
                u32_at(entry.data, 4)?,
                Defaults {
                    description: u32_at(entry.data, 8)?,
                    duration: u32_at(entry.data, 12)?,
                    size: u32_at(entry.data, 16)?,
                    flags: u32_at(entry.data, 20)?,
                },
            );
        }
    }
    Some(result)
}
