use crate::zip::result::{ZipResult, invalid};
use crate::zip::unstable::LittleEndianReadExt;
use core::mem;
use std::io::{Read, Write};
#[derive(Clone, Debug, PartialEq)]
pub struct UnicodeExtraField {
    pub(crate) crc32: u32,
    pub(crate) content: Box<[u8]>,
}
impl UnicodeExtraField {
    pub(crate) fn try_from_reader<R: Read>(reader: &mut R, len: u16) -> ZipResult<Self> {
        reader.read_exact(&mut [0u8])?;
        let crc32 = reader.read_u32_le()?;
        let content_len = (len as usize)
            .checked_sub(mem::size_of::<u8>() + mem::size_of::<u32>())
            .ok_or(invalid!("Unicode extra field is too small"))?;
        let mut content = vec![0u8; content_len].into_boxed_slice();
        reader.read_exact(&mut content)?;
        Ok(Self { crc32, content })
    }
    pub(crate) fn full_size(&self) -> usize {
        mem::size_of::<u16>() + mem::size_of::<u16>() + self.size()
    }
    pub(crate) fn size(&self) -> usize {
        mem::size_of::<u8>() + mem::size_of::<u32>() + self.content.len()
    }
    pub(crate) fn write<W: Write>(&self, writer: &mut W) -> ZipResult<()> {
        let size = self.size() as u16;
        writer.write_all(&size.to_le_bytes())?;
        let version = 1u8;
        writer.write_all(&version.to_le_bytes())?;
        writer.write_all(&self.crc32.to_le_bytes())?;
        writer.write_all(&self.content)?;
        Ok(())
    }
}
