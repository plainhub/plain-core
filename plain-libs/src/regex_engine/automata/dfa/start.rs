use crate::regex_engine::automata::util::wire::{self, DeserializeError, Endian, SerializeError};
use core::mem::size_of;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartKind {
    Both,
    Unanchored,
    Anchored,
}
impl StartKind {
    pub(crate) fn from_bytes(slice: &[u8]) -> Result<(StartKind, usize), DeserializeError> {
        wire::check_slice_len(slice, size_of::<u32>(), "start kind bytes")?;
        let (n, nr) = wire::try_read_u32(slice, "start kind integer")?;
        match n {
            0 => Ok((StartKind::Both, nr)),
            1 => Ok((StartKind::Unanchored, nr)),
            2 => Ok((StartKind::Anchored, nr)),
            _ => Err(DeserializeError::generic("unrecognized start kind")),
        }
    }
    pub(crate) fn write_to<E: Endian>(&self, dst: &mut [u8]) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("start kind"));
        }
        let n = match *self {
            StartKind::Both => 0,
            StartKind::Unanchored => 1,
            StartKind::Anchored => 2,
        };
        E::write_u32(n, dst);
        Ok(nwrite)
    }
    pub(crate) fn write_to_len(&self) -> usize {
        size_of::<u32>()
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn has_unanchored(&self) -> bool {
        matches!(*self, StartKind::Both | StartKind::Unanchored)
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn has_anchored(&self) -> bool {
        matches!(*self, StartKind::Both | StartKind::Anchored)
    }
}
