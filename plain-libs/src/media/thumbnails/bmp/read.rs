use std::io::{self, Read};

pub(super) trait ReadBytes: Read + Sized {
    fn read_number<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let mut bytes = [0; N];
        self.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    fn read_u8(&mut self) -> io::Result<u8> {
        Ok(self.read_number::<1>()?[0])
    }

    fn read_u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.read_number()?))
    }

    fn read_u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.read_number()?))
    }

    fn read_i32(&mut self) -> io::Result<i32> {
        Ok(i32::from_le_bytes(self.read_number()?))
    }

    // Adapted from image 0.25.10 src/io.rs; see LICENSE-MIT.
    fn read_exact_vec(&mut self, bytes: &mut Vec<u8>, length: usize) -> io::Result<()> {
        let initial = bytes.len();
        bytes.try_reserve(length)?;
        match self.take(length as u64).read_to_end(bytes) {
            Ok(read) if read == length => Ok(()),
            failure => {
                bytes.truncate(initial);
                Err(failure.err().unwrap_or(io::ErrorKind::UnexpectedEof.into()))
            }
        }
    }
}

impl<R: Read> ReadBytes for R {}

// Adapted from image 0.25.10 src/utils/mod.rs; see LICENSE-MIT.
pub(super) fn vec_try_with_capacity<T>(
    capacity: usize,
) -> Result<Vec<T>, std::collections::TryReserveError> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity)?;
    Ok(bytes)
}
