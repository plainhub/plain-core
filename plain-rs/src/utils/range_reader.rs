use std::io::{self, Read, Seek, SeekFrom};

pub struct RangeReader<R> {
    reader: R,
    base: u64,
    length: u64,
    position: u64,
}
impl<R: Seek> RangeReader<R> {
    pub fn new(mut reader: R, base: u64, length: u64) -> io::Result<Self> {
        base.checked_add(length)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "File range overflow"))?;
        reader.seek(SeekFrom::Start(base))?;
        Ok(Self {
            reader,
            base,
            length,
            position: 0,
        })
    }
}
impl<R: Read> Read for RangeReader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = usize::try_from((self.length - self.position).min(bytes.len() as u64)).unwrap();
        if count == 0 {
            return Ok(0);
        }
        let count = self.reader.read(&mut bytes[..count])?;
        self.position += count as u64;
        Ok(count)
    }
}
impl<R: Seek> Seek for RangeReader<R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(value) => i128::from(value),
            SeekFrom::End(value) => i128::from(self.length) + i128::from(value),
            SeekFrom::Current(value) => i128::from(self.position) + i128::from(value),
        };
        if position < 0 || position > i128::from(self.length) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Seek outside file range",
            ));
        }
        let position = position as u64;
        self.reader.seek(SeekFrom::Start(self.base + position))?;
        self.position = position;
        Ok(position)
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-rs/unit/utils/range_reader.rs"]
mod tests;
