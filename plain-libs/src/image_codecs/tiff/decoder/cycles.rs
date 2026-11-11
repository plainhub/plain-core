use crate::image_codecs::tiff::{TiffError, TiffFormatError, tags::IfdPointer};
use std::collections::HashMap;
#[derive(Default, Debug)]
pub struct IfdCycles {
    component_union: HashMap<ComponentId, ComponentId>,
    links: HashMap<IfdPointer, u64>,
    chains: HashMap<IfdPointer, ComponentId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ComponentId(u64);

impl IfdCycles {
    pub fn new() -> Self {
        IfdCycles::default()
    }

    pub fn insert_next(
        &mut self,
        from: IfdPointer,
        to: Option<IfdPointer>,
    ) -> Result<bool, TiffError> {
        let to_offset = to.map_or(0, |p| p.0);

        // For some reason we got
        match self.links.get(&from) {
            Some(existing) if *existing == to_offset => return Ok(false),
            // We got here twice with two different reads of the same IFD. That is .. unusual and
            // we interpret it as a cycle.
            Some(_) => return Err(TiffError::FormatError(TiffFormatError::CycleInOffsets)),
            None => self.links.insert(from, to_offset),
        };

        self.ensure_node(from);

        if let Some(to) = to {
            self.ensure_node(to);

            let parent = self.nominal_component(from);
            let child = self.nominal_component(to);

            if parent == child {
                return Err(TiffError::FormatError(TiffFormatError::CycleInOffsets));
            }

            self.component_union.insert(child, parent);
        }

        Ok(true)
    }

    fn ensure_node(&mut self, ifd: IfdPointer) {
        self.chains.entry(ifd).or_insert_with(|| {
            let id = ComponentId(self.component_union.len() as u64);
            self.component_union.insert(id, id);
            id
        });
    }

    fn nominal_component(&mut self, node: IfdPointer) -> ComponentId {
        let id = self.chains[&node];

        let nomimal = {
            let mut iter = id;

            loop {
                let parent = self.component_union[&iter];

                if parent == iter {
                    break parent;
                }

                iter = parent;
            }
        };

        if nomimal != id {
            // Compress.
            let mut iter = id;

            loop {
                let parent = self.component_union[&iter];

                if parent == iter {
                    break;
                }

                self.component_union.insert(iter, nomimal);
                iter = parent;
            }
        }

        nomimal
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/image_codecs/tiff/decoder/cycles.rs"]
mod tests;
