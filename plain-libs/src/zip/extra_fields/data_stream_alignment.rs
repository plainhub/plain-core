use crate::zip::extra_fields::UsedExtraField;
use crate::zip::result::{ZipResult, invalid};
use crate::zip::unstable::LittleEndianReadExt;
use core::mem;
use std::io::{Read, Write};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataStreamAlignment {
    pub(crate) header_id: u16,
    pub(crate) alignment: u16,
    pub(crate) pad_len: u16,
}
impl DataStreamAlignment {
    #[must_use]
    pub fn new(padding_len: u16, alignment: u16) -> Option<Self> {
        Self::new_with_header_id(
            UsedExtraField::DataStreamAlignment.as_u16(),
            padding_len,
            alignment,
        )
    }
    #[must_use]
    pub fn android(padding_len: u16, alignment: u16) -> Option<Self> {
        Self::new_with_header_id(
            UsedExtraField::AndroidZipAlignment.as_u16(),
            padding_len,
            alignment,
        )
    }
    #[must_use]
    pub fn new_with_header_id(header_id: u16, padding_len: u16, alignment: u16) -> Option<Self> {
        if padding_len >= 6 {
            Some(Self {
                header_id,
                alignment,
                pad_len: padding_len.saturating_sub(4),
            })
        } else {
            None
        }
    }
    #[must_use]
    pub const fn header_id(&self) -> u16 {
        self.header_id
    }
    #[must_use]
    pub const fn alignment(&self) -> u16 {
        self.alignment
    }
    #[must_use]
    pub const fn pad_len(&self) -> u16 {
        self.pad_len
    }
    pub(crate) fn try_from_reader<R: Read>(
        reader: &mut R,
        header_id: u16,
        len: u16,
    ) -> ZipResult<Self> {
        if len < 2 {
            return Err(invalid!("Alignment extra field too short"));
        }
        let alignment = reader.read_u16_le()?;
        let mut remaining = (len - 2) as usize;
        let mut buf = [0u8; 1024];
        while remaining > 0 {
            let to_read = remaining.min(buf.len());
            reader.read_exact(&mut buf[..to_read])?;
            remaining -= to_read;
        }
        Ok(Self {
            header_id,
            alignment,
            pad_len: len,
        })
    }
    pub(crate) fn full_size(&self, is_local_header: bool) -> usize {
        if is_local_header {
            mem::size_of::<u16>() + mem::size_of::<u16>() + self.pad_len as usize
        } else {
            0
        }
    }
    pub(crate) fn write<W: Write>(
        &self,
        writer: &mut W,
        is_local_header: bool,
    ) -> std::io::Result<()> {
        if !is_local_header {
            return Ok(());
        }
        writer.write_all(&self.header_id.to_le_bytes())?;
        writer.write_all(&self.pad_len.to_le_bytes())?;
        writer.write_all(&self.alignment.to_le_bytes())?;
        let pad_len = self.pad_len.saturating_sub(2);
        let zeros = [0u8; 1024];
        let mut remaining = pad_len as usize;
        while remaining > 0 {
            let n = remaining.min(zeros.len());
            writer.write_all(&zeros[..n])?;
            remaining -= n;
        }
        Ok(())
    }
}
