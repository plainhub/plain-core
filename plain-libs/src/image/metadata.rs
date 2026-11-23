pub(crate) mod cicp;
pub use self::cicp::{
    Cicp, CicpColorPrimaries, CicpMatrixCoefficients, CicpTransferCharacteristics,
    CicpVideoFullRangeFlag,
};
use crate::binary::{BigEndian, LittleEndian, ReadBytesExt};
use std::{
    io::{Cursor, Read, Seek, SeekFrom},
    num::NonZeroU32,
};
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(any(), derive(serde::Serialize, serde::Deserialize))]
pub enum Orientation {
    NoTransforms,
    Rotate90,
    Rotate180,
    Rotate270,
    FlipHorizontal,
    FlipVertical,
    Rotate90FlipH,
    Rotate270FlipH,
}
impl Orientation {
    #[must_use]
    pub fn from_exif(exif_orientation: u8) -> Option<Self> {
        match exif_orientation {
            1 => Some(Self::NoTransforms),
            2 => Some(Self::FlipHorizontal),
            3 => Some(Self::Rotate180),
            4 => Some(Self::FlipVertical),
            5 => Some(Self::Rotate90FlipH),
            6 => Some(Self::Rotate90),
            7 => Some(Self::Rotate270FlipH),
            8 => Some(Self::Rotate270),
            0 | 9.. => None,
        }
    }
    #[must_use]
    pub fn to_exif(self) -> u8 {
        match self {
            Self::NoTransforms => 1,
            Self::FlipHorizontal => 2,
            Self::Rotate180 => 3,
            Self::FlipVertical => 4,
            Self::Rotate90FlipH => 5,
            Self::Rotate90 => 6,
            Self::Rotate270FlipH => 7,
            Self::Rotate270 => 8,
        }
    }
    #[must_use]
    pub fn from_exif_chunk(chunk: &[u8]) -> Option<Self> {
        Self::from_exif_reader(&mut Cursor::new(chunk))
    }

    pub(crate) fn from_exif_reader(reader: &mut (impl Read + Seek)) -> Option<Self> {
        let mut magic = [0; 4];
        reader.read_exact(&mut magic).ok()?;
        match magic {
            [0x49, 0x49, 42, 0] => Self::locate_orientation_entry::<LittleEndian>(reader),
            [0x4d, 0x4d, 0, 42] => Self::locate_orientation_entry::<BigEndian>(reader),
            _ => None,
        }
    }
    fn locate_orientation_entry<B>(reader: &mut (impl Read + Seek)) -> Option<Self>
    where
        B: crate::binary::ByteOrder,
    {
        let ifd_offset = reader.read_u32::<B>().ok()?;
        reader.seek(SeekFrom::Start(u64::from(ifd_offset))).ok()?;
        let entries = reader.read_u16::<B>().ok()?;
        for _ in 0..entries {
            let tag = reader.read_u16::<B>().ok()?;
            let format = reader.read_u16::<B>().ok()?;
            let count = reader.read_u32::<B>().ok()?;
            let value = reader.read_u16::<B>().ok()?;
            let _padding = reader.read_u16::<B>().ok()?;
            if tag == 0x112 && format == 3 && count == 1 {
                return Self::from_exif(value.min(255) as u8);
            }
        }
        None
    }
}
#[derive(Clone, Copy)]
pub enum LoopCount {
    Infinite,
    Finite(NonZeroU32),
}
mod luminance;
