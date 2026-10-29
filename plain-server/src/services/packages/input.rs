use anyhow::{Result, anyhow, ensure};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    io::{self, BufReader, Read, Seek, SeekFrom},
    ops::Range,
};

pub(super) const READ_LIMIT: usize = 64 * 1024 * 1024;

pub(super) trait ReadAt {
    fn read_at(&self, offset: usize, bytes: &mut [u8]) -> Result<()>;
}

struct ReadBudget<R> {
    reader: R,
    remaining: usize,
}

impl<R: Read> Read for ReadBudget<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            return Err(io::Error::other("Resource read budget exceeded"));
        }
        let length = bytes.len().min(self.remaining);
        let read = self.reader.read(&mut bytes[..length])?;
        self.remaining -= read;
        Ok(read)
    }
}

impl<R: Seek> Seek for ReadBudget<R> {
    fn seek(&mut self, offset: SeekFrom) -> io::Result<u64> {
        self.reader.seek(offset)
    }
}

pub(super) struct ResourceReader<R> {
    reader: RefCell<BufReader<ReadBudget<R>>>,
    position: Cell<usize>,
}

impl<R: Read + Seek> ResourceReader<R> {
    pub(super) fn new(mut reader: R) -> Result<Self> {
        reader.seek(SeekFrom::Start(0))?;
        Ok(Self {
            reader: RefCell::new(BufReader::new(ReadBudget {
                reader,
                remaining: READ_LIMIT,
            })),
            position: Cell::new(0),
        })
    }
}

impl<R: Read + Seek> ReadAt for ResourceReader<R> {
    fn read_at(&self, offset: usize, bytes: &mut [u8]) -> Result<()> {
        let mut reader = self.reader.borrow_mut();
        reader.seek_relative(offset as i64 - self.position.get() as i64)?;
        reader.read_exact(bytes)?;
        self.position.set(offset + bytes.len());
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(super) enum Bytes<'a> {
    Slice(&'a [u8]),
    Reader {
        reader: &'a dyn ReadAt,
        offset: usize,
        length: usize,
    },
}

impl<'a> Bytes<'a> {
    pub(super) fn len(self) -> usize {
        match self {
            Self::Slice(bytes) => bytes.len(),
            Self::Reader { length, .. } => length,
        }
    }

    pub(super) fn slice(self, range: Range<usize>) -> Result<Self> {
        ensure!(
            range.start <= range.end && range.end <= self.len(),
            "APK range outside chunk"
        );
        Ok(match self {
            Self::Slice(bytes) => Self::Slice(&bytes[range]),
            Self::Reader { reader, offset, .. } => Self::Reader {
                reader,
                offset: offset + range.start,
                length: range.end - range.start,
            },
        })
    }

    pub(super) fn read(self, offset: usize, length: usize) -> Result<Cow<'a, [u8]>> {
        let end = offset
            .checked_add(length)
            .ok_or_else(|| anyhow!("APK offset overflow"))?;
        match self.slice(offset..end)? {
            Self::Slice(bytes) => Ok(Cow::Borrowed(bytes)),
            Self::Reader {
                reader,
                offset,
                length,
            } => {
                ensure!(length <= READ_LIMIT, "Resource read budget exceeded");
                let mut bytes = vec![0; length];
                reader.read_at(offset, &mut bytes)?;
                Ok(Cow::Owned(bytes))
            }
        }
    }

    fn array<const N: usize>(self, offset: usize) -> Result<[u8; N]> {
        let end = offset
            .checked_add(N)
            .ok_or_else(|| anyhow!("APK offset overflow"))?;
        match self.slice(offset..end)? {
            Self::Slice(bytes) => Ok(bytes.try_into()?),
            Self::Reader { reader, offset, .. } => {
                let mut bytes = [0; N];
                reader.read_at(offset, &mut bytes)?;
                Ok(bytes)
            }
        }
    }

    pub(super) fn byte(self, offset: usize) -> Result<u8> {
        Ok(self.array::<1>(offset)?[0])
    }

    pub(super) fn u16(self, offset: usize) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array(offset)?))
    }

    pub(super) fn u32(self, offset: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array(offset)?))
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/packages/input.rs"]
mod tests;
