use crate::zip::unstable::LittleEndianReadExt;
use crate::zip::{
    ZIP64_BYTES_THR,
    extra_fields::UsedExtraField,
    result::{ZipResult, invalid},
};
use core::mem;
use std::io::{ErrorKind, Read, Write, copy, sink};
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Zip64Sizes {
    pub(crate) uncompressed_size: u64,
    pub(crate) compressed_size: u64,
}
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Zip64ExtendedInformation {
    pub(crate) sizes: Option<Zip64Sizes>,
    pub(crate) header_start: Option<u64>,
}
impl Zip64ExtendedInformation {
    pub(crate) const MAGIC: UsedExtraField = UsedExtraField::Zip64ExtendedInfo;
    pub(crate) fn local_header(
        is_large_file: bool,
        uncompressed_size: u64,
        compressed_size: u64,
    ) -> Option<Self> {
        let should_add_size = is_large_file
            || uncompressed_size >= ZIP64_BYTES_THR
            || compressed_size >= ZIP64_BYTES_THR;
        if !should_add_size {
            return None;
        }
        Some(Self {
            sizes: Some(Zip64Sizes {
                uncompressed_size,
                compressed_size,
            }),
            header_start: None,
        })
    }
    pub(crate) fn central_header(
        is_large_file: bool,
        uncompressed_size: u64,
        compressed_size: u64,
        header_start: u64,
    ) -> Option<Self> {
        let mut size: u16 = 0;
        let sizes = if is_large_file
            || uncompressed_size >= ZIP64_BYTES_THR
            || compressed_size >= ZIP64_BYTES_THR
        {
            size += mem::size_of::<u64>() as u16 + mem::size_of::<u64>() as u16;
            Some(Zip64Sizes {
                uncompressed_size,
                compressed_size,
            })
        } else {
            None
        };
        let header_start = if header_start != 0 && header_start >= ZIP64_BYTES_THR {
            size += mem::size_of::<u64>() as u16;
            Some(header_start)
        } else {
            None
        };
        if size == 0 {
            return None;
        }
        Some(Self {
            sizes,
            header_start,
        })
    }
    pub(crate) fn full_size(&self, is_local_header: bool) -> usize {
        mem::size_of::<UsedExtraField>() + mem::size_of::<u16>() + self.size(is_local_header)
    }
    pub(crate) fn size(&self, is_local_header: bool) -> usize {
        let mut size = 0;
        if self.sizes.is_some() {
            size += mem::size_of::<u64>() + mem::size_of::<u64>();
        }
        if !is_local_header && self.header_start.is_some() {
            size += mem::size_of::<u64>();
        }
        size
    }
    pub fn write<T: Write>(&self, writer: &mut T, is_local_header: bool) -> ZipResult<()> {
        writer.write_all(&Self::MAGIC.to_le_bytes())?;
        let size = self.size(is_local_header) as u16;
        writer.write_all(&size.to_le_bytes())?;
        if let Some(Zip64Sizes {
            uncompressed_size,
            compressed_size,
        }) = self.sizes
        {
            writer.write_all(&u64::to_le_bytes(uncompressed_size))?;
            writer.write_all(&u64::to_le_bytes(compressed_size))?;
        }
        if !is_local_header && let Some(header_start) = self.header_start {
            writer.write_all(&u64::to_le_bytes(header_start))?;
        }
        Ok(())
    }
    #[inline]
    fn read_field<R: Read>(
        reader: &mut R,
        len: u16,
        consumed_len: &mut usize,
        is_zip64: bool,
    ) -> ZipResult<Option<u64>> {
        if !is_zip64 {
            return Ok(None);
        }
        if *consumed_len + mem::size_of::<u64>() > len as usize {
            return Err(invalid!("ZIP64 extra field truncated"));
        }
        let value = match reader.read_u64_le() {
            Ok(value) => value,
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                return Err(invalid!("ZIP64 extra field truncated"));
            }
            Err(e) => return Err(e.into()),
        };
        *consumed_len += mem::size_of::<u64>();
        Ok(Some(value))
    }
    #[inline]
    pub(crate) fn parse<R: Read>(
        reader: &mut R,
        len: u16,
        entry_uncompressed_size: u32,
        entry_compressed_size: u32,
        entry_header_start: Option<u32>,
    ) -> ZipResult<Self> {
        let mut consumed_len = 0;
        let uncompressed_size = Self::read_field(
            reader,
            len,
            &mut consumed_len,
            u64::from(entry_uncompressed_size) == ZIP64_BYTES_THR,
        )?;
        let compressed_size = Self::read_field(
            reader,
            len,
            &mut consumed_len,
            u64::from(entry_compressed_size) == ZIP64_BYTES_THR,
        )?;
        let header_start = Self::read_field(
            reader,
            len,
            &mut consumed_len,
            entry_header_start.is_some_and(|start| u64::from(start) == ZIP64_BYTES_THR),
        )?;
        let sizes = if uncompressed_size.is_some() || compressed_size.is_some() {
            Some(Zip64Sizes {
                uncompressed_size: uncompressed_size
                    .unwrap_or_else(|| entry_uncompressed_size.into()),
                compressed_size: compressed_size.unwrap_or_else(|| entry_compressed_size.into()),
            })
        } else {
            None
        };
        let Some(leftover_len) = (len as usize).checked_sub(consumed_len) else {
            return Err(invalid!("ZIP64 extra-data field is the wrong length"));
        };
        let mut limited = reader.take(leftover_len as u64);
        if let Err(e) = copy(&mut limited, &mut sink()) {
            if e.kind() == ErrorKind::UnexpectedEof {
                return Err(invalid!("ZIP64 extra field truncated"));
            }
            return Err(e.into());
        }
        Ok(Self {
            sizes,
            header_start,
        })
    }
}
