use crate::zip::format::magic::Magic;
use core::mem;
use std::io::{self, Write};
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ZipDataDescriptor {
    ZipDataDescriptorBlock(ZipDataDescriptorBlock),
    Zip64DataDescriptorBlock(Zip64DataDescriptorBlock),
}
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ZipDataDescriptorBlock {
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
}
impl ZipDataDescriptorBlock {
    pub const SIZE: usize = mem::size_of::<u32>()
        + mem::size_of::<u32>()
        + mem::size_of::<u32>()
        + mem::size_of::<u32>();
    pub const MAGIC: Magic = Magic::DATA_DESCRIPTOR_SIGNATURE;
    pub fn parse(buff: &[u8; Self::SIZE]) -> Result<Self, &'static str> {
        let magic = Magic::from_le_bytes([buff[0], buff[1], buff[2], buff[3]]);
        if magic != Self::MAGIC {
            return Err("Invalid data descriptor header");
        }
        let crc32 = u32::from_le_bytes([buff[4], buff[5], buff[6], buff[7]]);
        let compressed_size = u32::from_le_bytes([buff[8], buff[9], buff[10], buff[11]]);
        let uncompressed_size = u32::from_le_bytes([buff[12], buff[13], buff[14], buff[15]]);
        Ok(Self {
            crc32,
            compressed_size,
            uncompressed_size,
        })
    }
    pub fn write<W: Write>(&self, w: &mut W) -> io::Result<()> {
        w.write_all(&Self::MAGIC.to_le_bytes())?;
        w.write_all(&self.crc32.to_le_bytes())?;
        w.write_all(&self.compressed_size.to_le_bytes())?;
        w.write_all(&self.uncompressed_size.to_le_bytes())?;
        Ok(())
    }
}
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Zip64DataDescriptorBlock {
    pub crc32: u32,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
}
impl Zip64DataDescriptorBlock {
    pub const SIZE: usize = mem::size_of::<u32>()
        + mem::size_of::<u32>()
        + mem::size_of::<u64>()
        + mem::size_of::<u64>();
    pub const MAGIC: Magic = Magic::DATA_DESCRIPTOR_SIGNATURE;
    pub fn parse(buff: &[u8; Self::SIZE]) -> Result<Self, &'static str> {
        let magic = Magic::from_le_bytes([buff[0], buff[1], buff[2], buff[3]]);
        if magic != Self::MAGIC {
            return Err("Invalid zip64 data descriptor header");
        }
        let crc32 = u32::from_le_bytes([buff[4], buff[5], buff[6], buff[7]]);
        let compressed_size = u64::from_le_bytes([
            buff[8], buff[9], buff[10], buff[11], buff[12], buff[13], buff[14], buff[15],
        ]);
        let uncompressed_size = u64::from_le_bytes([
            buff[16], buff[17], buff[18], buff[19], buff[20], buff[21], buff[22], buff[23],
        ]);
        Ok(Self {
            crc32,
            compressed_size,
            uncompressed_size,
        })
    }
    pub fn write<W: Write>(&self, w: &mut W) -> io::Result<()> {
        w.write_all(&Self::MAGIC.to_le_bytes())?;
        w.write_all(&self.crc32.to_le_bytes())?;
        w.write_all(&self.compressed_size.to_le_bytes())?;
        w.write_all(&self.uncompressed_size.to_le_bytes())?;
        Ok(())
    }
}
