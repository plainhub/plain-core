#![forbid(unsafe_code)]

use std::io::{self, Read, Write};

pub(crate) trait ByteOrder {
    fn read_u16(bytes: [u8; 2]) -> u16;
    fn read_u24(bytes: [u8; 3]) -> u32;
    fn read_u32(bytes: [u8; 4]) -> u32;
    fn write_u16(value: u16) -> [u8; 2];
}

pub(crate) enum LittleEndian {}
pub(crate) enum BigEndian {}

impl ByteOrder for LittleEndian {
    fn read_u16(bytes: [u8; 2]) -> u16 {
        u16::from_le_bytes(bytes)
    }
    fn read_u24(bytes: [u8; 3]) -> u32 {
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0])
    }
    fn read_u32(bytes: [u8; 4]) -> u32 {
        u32::from_le_bytes(bytes)
    }
    fn write_u16(value: u16) -> [u8; 2] {
        value.to_le_bytes()
    }
}

impl ByteOrder for BigEndian {
    fn read_u16(bytes: [u8; 2]) -> u16 {
        u16::from_be_bytes(bytes)
    }
    fn read_u24(bytes: [u8; 3]) -> u32 {
        u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]])
    }
    fn read_u32(bytes: [u8; 4]) -> u32 {
        u32::from_be_bytes(bytes)
    }
    fn write_u16(value: u16) -> [u8; 2] {
        value.to_be_bytes()
    }
}

pub(crate) trait ReadBytesExt: Read {
    fn read_u8(&mut self) -> io::Result<u8> {
        let mut bytes = [0];
        self.read_exact(&mut bytes)?;
        Ok(bytes[0])
    }
    fn read_u16<B: ByteOrder>(&mut self) -> io::Result<u16> {
        let mut bytes = [0; 2];
        self.read_exact(&mut bytes)?;
        Ok(B::read_u16(bytes))
    }
    fn read_u24<B: ByteOrder>(&mut self) -> io::Result<u32> {
        let mut bytes = [0; 3];
        self.read_exact(&mut bytes)?;
        Ok(B::read_u24(bytes))
    }
    fn read_u32<B: ByteOrder>(&mut self) -> io::Result<u32> {
        let mut bytes = [0; 4];
        self.read_exact(&mut bytes)?;
        Ok(B::read_u32(bytes))
    }
}
impl<R: Read + ?Sized> ReadBytesExt for R {}

pub(crate) trait WriteBytesExt: Write {
    fn write_u16<B: ByteOrder>(&mut self, value: u16) -> io::Result<()> {
        self.write_all(&B::write_u16(value))
    }
}
impl<W: Write + ?Sized> WriteBytesExt for W {}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/byte_order.rs"]
mod tests;
