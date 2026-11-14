pub(crate) mod cicp;
pub use self::cicp::{
    Cicp, CicpColorPrimaries, CicpMatrixCoefficients, CicpTransferCharacteristics,
    CicpVideoFullRangeFlag,
};
use crate::binary::{BigEndian, LittleEndian, ReadBytesExt};
use std::{
    io::{Cursor, Read},
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
        Self::from_exif_chunk_inner(chunk).map(|res| res.0)
    }

    #[must_use]
    fn from_exif_chunk_inner(chunk: &[u8]) -> Option<(Self, u64, ExifEndian)> {
        let mut reader = Cursor::new(chunk);
        let mut magic = [0; 4];
        reader.read_exact(&mut magic).ok()?;
        match magic {
            [0x49, 0x49, 42, 0] => {
                return Self::locate_orientation_entry::<LittleEndian>(&mut reader)
                    .map(|(orient, offset)| (orient, offset, ExifEndian::Little));
            }
            [0x4d, 0x4d, 0, 42] => {
                return Self::locate_orientation_entry::<BigEndian>(&mut reader)
                    .map(|(orient, offset)| (orient, offset, ExifEndian::Big));
            }
            _ => {}
        }
        None
    }
    fn locate_orientation_entry<B>(reader: &mut Cursor<&[u8]>) -> Option<(Self, u64)>
    where
        B: crate::binary::ByteOrder,
    {
        let ifd_offset = reader.read_u32::<B>().ok()?;
        reader.set_position(u64::from(ifd_offset));
        let entries = reader.read_u16::<B>().ok()?;
        for _ in 0..entries {
            let tag = reader.read_u16::<B>().ok()?;
            let format = reader.read_u16::<B>().ok()?;
            let count = reader.read_u32::<B>().ok()?;
            let value = reader.read_u16::<B>().ok()?;
            let _padding = reader.read_u16::<B>().ok()?;
            if tag == 0x112 && format == 3 && count == 1 {
                let offset = reader.position() - 4;
                let orientation = Self::from_exif(value.min(255) as u8);
                return orientation.map(|orient| (orient, offset));
            }
        }
        None
    }
}
#[derive(Debug, Copy, Clone)]
enum ExifEndian {
    Big,
    Little,
}
#[derive(Clone, Copy)]
pub enum LoopCount {
    Infinite,
    Finite(NonZeroU32),
}
mod luminance;
