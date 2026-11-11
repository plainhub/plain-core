use crate::zip::result::invalid;
use crate::zip::result::{ZipError, ZipResult};
use crate::zip::unstable::LittleEndianReadExt;
use core::mem;
use std::io::Read;
# [rustfmt :: skip] # [repr (u8)] # [derive (Debug , Copy , Clone , PartialEq , Eq)] enum ExtendedTimestampFlags { Modified = 0b0000_0001_u8 , Accessed = 0b0000_0010_u8 , Created = 0b0000_0100_u8 }
impl ExtendedTimestampFlags {
    #[inline(always)]
    fn matching(flags: u8, matching_flag: Self) -> bool {
        flags & u8::from(matching_flag) != 0
    }
}
impl From<ExtendedTimestampFlags> for u8 {
    fn from(extended_timestamp: ExtendedTimestampFlags) -> u8 {
        extended_timestamp as u8
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct ExtendedTimestamp {
    modified: Option<u32>,
    accessed: Option<u32>,
    created: Option<u32>,
}
impl ExtendedTimestamp {
    const MAX_LENGTH: u16 = (mem::size_of::<u8>()
        + mem::size_of::<u32>()
        + mem::size_of::<u32>()
        + mem::size_of::<u32>()) as u16;
    pub fn try_from_reader<R: Read>(
        reader: &mut R,
        len: u16,
        is_local_header: bool,
    ) -> ZipResult<Self> {
        if len == 0 {
            return Err(invalid!("Extended timestamp field is empty"));
        }
        let mut flags = [0u8];
        reader.read_exact(&mut flags)?;
        let bytes_to_read = (len as usize)
            .checked_sub(flags.len())
            .ok_or_else(|| invalid!("Extended timestamp field too short for flags"))?;
        if is_local_header {
            let flags = flags[0];
            return Self::parse_local_header(reader, len, flags, bytes_to_read);
        }
        Self::parse_central_header(reader, bytes_to_read)
    }
    fn parse_local_header<R: Read>(
        reader: &mut R,
        len: u16,
        flags: u8,
        mut bytes_to_read: usize,
    ) -> ZipResult<Self> {
        if len != 5 && u32::from(len) != 1 + 4 * flags.count_ones() {
            return Err(ZipError::Io(std::io::Error::other(format!(
                "flags and len don't match in extended timestamp field len={len} flags={flags:08b}"
            ))));
        }
        let modified =
            if (ExtendedTimestampFlags::matching(flags, ExtendedTimestampFlags::Modified)
                && bytes_to_read >= mem::size_of::<u32>())
                || len == Self::MAX_LENGTH
            {
                bytes_to_read = bytes_to_read
                .checked_sub(mem::size_of::<u32>())
                .ok_or_else(|| {
                    invalid!(
                        "Extended timestamp field too short for mod_time len={} flags={flags:08b}",
                        len
                    )
                })?;
                Some(reader.read_u32_le()?)
            } else {
                None
            };
        let accessed =
            if (ExtendedTimestampFlags::matching(flags, ExtendedTimestampFlags::Accessed)
                && bytes_to_read >= mem::size_of::<u32>())
                || len == Self::MAX_LENGTH
            {
                bytes_to_read = bytes_to_read
                .checked_sub(mem::size_of::<u32>())
                .ok_or_else(|| {
                    invalid!(
                        "Extended timestamp field too short for ac_time len={} flags={flags:08b}",
                        len
                    )
                })?;
                Some(reader.read_u32_le()?)
            } else {
                None
            };
        let created = if (ExtendedTimestampFlags::matching(flags, ExtendedTimestampFlags::Created)
            && bytes_to_read >= mem::size_of::<u32>())
            || len == Self::MAX_LENGTH
        {
            bytes_to_read = bytes_to_read
                .checked_sub(mem::size_of::<u32>())
                .ok_or_else(|| {
                    invalid!(
                        "Extended timestamp field too short for cr_time len={} flags={flags:08b}",
                        len
                    )
                })?;
            Some(reader.read_u32_le()?)
        } else {
            None
        };
        if bytes_to_read > 0 {
            let n = std::io::copy(&mut reader.take(bytes_to_read as u64), &mut std::io::sink())?;
            if n != bytes_to_read as u64 {
                return Err(invalid!(
                    "unexpected EOF in local header of Extended Timestamp extra field"
                ));
            }
        }
        Ok(Self {
            modified,
            accessed,
            created,
        })
    }
    fn parse_central_header<R: Read>(reader: &mut R, mut bytes_to_read: usize) -> ZipResult<Self> {
        let modified = if bytes_to_read >= mem::size_of::<u32>() {
            bytes_to_read -= mem::size_of::<u32>();
            Some(reader.read_u32_le()?)
        } else {
            None
        };
        if bytes_to_read > 0 {
            let n = std::io::copy(&mut reader.take(bytes_to_read as u64), &mut std::io::sink())?;
            if n != bytes_to_read as u64 {
                return Err(invalid!(
                    "unexpected EOF in central header of Extended Timestamp extra field"
                ));
            }
        }
        Ok(Self {
            modified,
            accessed: None,
            created: None,
        })
    }
    #[must_use]
    pub fn mod_time(&self) -> Option<u32> {
        self.modified
    }
    #[must_use]
    pub fn ac_time(&self) -> Option<u32> {
        self.accessed
    }
    #[must_use]
    pub fn cr_time(&self) -> Option<u32> {
        self.created
    }
}
