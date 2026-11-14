#![forbid(unsafe_code)]

use std::io::{Read, Result, Write};

pub trait ByteOrder {
    const LITTLE: bool;

    fn read_u16(bytes: &[u8]) -> u16 {
        let bytes = bytes[..2].try_into().unwrap();
        if Self::LITTLE {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        }
    }

    fn read_u32(bytes: &[u8]) -> u32 {
        let bytes = bytes[..4].try_into().unwrap();
        if Self::LITTLE {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        }
    }

    fn read_u64(bytes: &[u8]) -> u64 {
        let bytes = bytes[..8].try_into().unwrap();
        if Self::LITTLE {
            u64::from_le_bytes(bytes)
        } else {
            u64::from_be_bytes(bytes)
        }
    }
}

pub enum LittleEndian {}
pub enum BigEndian {}

impl ByteOrder for LittleEndian {
    const LITTLE: bool = true;
}
impl ByteOrder for BigEndian {
    const LITTLE: bool = false;
}

pub trait ReadBytesExt: Read {
    fn read_fixed<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut bytes = [0; N];
        self.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_fixed::<1>()?[0])
    }

    fn read_u16<B: ByteOrder>(&mut self) -> Result<u16> {
        Ok(B::read_u16(&self.read_fixed::<2>()?))
    }

    fn read_u24<B: ByteOrder>(&mut self) -> Result<u32> {
        let [a, b, c] = self.read_fixed()?;
        Ok(if B::LITTLE {
            u32::from_le_bytes([a, b, c, 0])
        } else {
            u32::from_be_bytes([0, a, b, c])
        })
    }

    fn read_u32<B: ByteOrder>(&mut self) -> Result<u32> {
        Ok(B::read_u32(&self.read_fixed::<4>()?))
    }

    fn read_u64<B: ByteOrder>(&mut self) -> Result<u64> {
        Ok(B::read_u64(&self.read_fixed::<8>()?))
    }

    fn read_u128<B: ByteOrder>(&mut self) -> Result<u128> {
        let bytes = self.read_fixed()?;
        Ok(if B::LITTLE {
            u128::from_le_bytes(bytes)
        } else {
            u128::from_be_bytes(bytes)
        })
    }

    fn read_i16<B: ByteOrder>(&mut self) -> Result<i16> {
        self.read_u16::<B>().map(|n| n as i16)
    }
    fn read_i32<B: ByteOrder>(&mut self) -> Result<i32> {
        self.read_u32::<B>().map(|n| n as i32)
    }

    fn read_uint<B: ByteOrder>(&mut self, count: usize) -> Result<u64> {
        assert!(
            (1..=8).contains(&count),
            "integer width must be between one and eight bytes"
        );
        let mut bytes = [0_u8; 8];
        self.read_exact(&mut bytes[..count])?;
        let bytes = &bytes[..count];
        Ok(if B::LITTLE {
            bytes
                .iter()
                .rev()
                .fold(0, |number, byte| number << 8 | u64::from(*byte))
        } else {
            bytes
                .iter()
                .fold(0, |number, byte| number << 8 | u64::from(*byte))
        })
    }
}

impl<R: Read + ?Sized> ReadBytesExt for R {}

pub trait WriteBytesExt: Write {
    fn write_u8(&mut self, value: u8) -> Result<()> {
        self.write_all(&[value])
    }
    fn write_u32<B: ByteOrder>(&mut self, value: u32) -> Result<()> {
        self.write_all(&if B::LITTLE {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        })
    }
}

impl<W: Write + ?Sized> WriteBytesExt for W {}

#[cfg(test)]
#[path = "../../tests/unit/binary.rs"]
mod tests;
