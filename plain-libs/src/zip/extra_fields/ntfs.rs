use crate::zip::{
    result::{ZipError, ZipResult},
    unstable::LittleEndianReadExt,
};
use std::io::Read;
#[derive(Clone, Debug, PartialEq)]
pub struct Ntfs {
    mtime: u64,
    atime: u64,
    ctime: u64,
}
impl Ntfs {
    pub fn try_from_reader<R>(reader: &mut R, len: u16) -> ZipResult<Self>
    where
        R: Read,
    {
        if len != 32 {
            return Err(ZipError::UnsupportedArchive(
                "NTFS extra field has an unsupported length",
            ));
        }
        let _ = reader.read_u32_le()?;
        let tag = reader.read_u16_le()?;
        if tag != 0x0001 {
            return Err(ZipError::UnsupportedArchive(
                "NTFS extra field has an unsupported attribute tag",
            ));
        }
        let size = reader.read_u16_le()?;
        if size != 24 {
            return Err(ZipError::UnsupportedArchive(
                "NTFS extra field has an unsupported attribute size",
            ));
        }
        let mtime = reader.read_u64_le()?;
        let atime = reader.read_u64_le()?;
        let ctime = reader.read_u64_le()?;
        Ok(Self {
            mtime,
            atime,
            ctime,
        })
    }
    #[must_use]
    pub fn mtime(&self) -> u64 {
        self.mtime
    }
    #[must_use]
    pub fn atime(&self) -> u64 {
        self.atime
    }
    #[must_use]
    pub fn ctime(&self) -> u64 {
        self.ctime
    }
}
