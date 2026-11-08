use crate::compound_file::internal::{MiniAllocator, ObjType, consts};
use std::convert::TryFrom;
use std::io::{self, BufRead, Read, Seek, SeekFrom};

//===========================================================================//

use crate::compound_file::internal::stream_buffer::StreamBuffer;

/// Reads and writes at least this large go straight to the underlying file
/// rather than through the stream's buffer: copying them through the buffer
/// (and growing it to hold them) costs more than the buffering saves.
const DIRECT_IO_MIN: usize = 64 * 1024;

/// How much of a stream `read_to_end` reads per call to the underlying
/// file.
const READ_TO_END_CHUNK: usize = 16 * 1024 * 1024;

//===========================================================================//

/// A stream entry in a compound file, much like a filesystem file.
pub struct Stream<'a, F> {
    minialloc: &'a mut MiniAllocator<F>,
    stream_id: u32,
    total_len: u64,
    buffer: StreamBuffer,
    buf_offset_from_start: u64,
}

impl<'a, F> Stream<'a, F> {
    pub(crate) fn new(minialloc: &'a mut MiniAllocator<F>, stream_id: u32) -> Self {
        let total_len = minialloc.dir_entry(stream_id).stream_len;
        Stream {
            minialloc,
            stream_id,
            total_len,
            buffer: StreamBuffer::new(),
            buf_offset_from_start: 0,
        }
    }

    fn current_position(&self) -> u64 {
        self.buf_offset_from_start + (self.buffer.cursor() as u64)
    }
}

impl<F: Read + Seek> Stream<'_, F> {
    /// Reads into `buf` straight from the underlying file, from the current
    /// position on.  The buffer must hold no unread data; it is emptied.
    fn read_direct(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        debug_assert!(!self.buffer.has_remaining());
        let position = self.current_position();
        self.buf_offset_from_start = position;
        self.buffer.clear();
        let num_bytes = read_data_from_stream(self.minialloc, self.stream_id, position, buf)?;
        self.buf_offset_from_start += num_bytes as u64;
        Ok(num_bytes)
    }
}

impl<F: Read + Seek> BufRead for Stream<'_, F> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if !self.buffer.has_remaining() && self.current_position() < self.total_len {
            self.buf_offset_from_start += self.buffer.cursor() as u64;
            let remaining = self.total_len - self.buf_offset_from_start;
            let stream_id = self.stream_id;
            let offset = self.buf_offset_from_start;
            self.buffer.refill_with(remaining, |buf| {
                read_data_from_stream(self.minialloc, stream_id, offset, buf)
            })?;
        }
        Ok(self.buffer.remaining_slice())
    }

    fn consume(&mut self, amt: usize) {
        self.buffer.consume(amt);
    }
}

impl<F: Read + Seek> Read for Stream<'_, F> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.len() >= DIRECT_IO_MIN && !self.buffer.has_remaining() {
            return self.read_direct(buf);
        }
        let mut buffered_data = self.fill_buf()?;
        let num_bytes = buffered_data.read(buf)?;
        self.consume(num_bytes);
        Ok(num_bytes)
    }

    /// Reads the rest of the stream straight into `buf`, which is grown
    /// once to the exact size needed.
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        let buffered = self.buffer.remaining_slice();
        let buffered_len = buffered.len();
        buf.extend_from_slice(buffered);
        self.buffer.consume(buffered_len);
        let position = self.current_position();
        debug_assert!(position <= self.total_len);
        let remaining = usize::try_from(self.total_len - position).map_err(|_| {
            io::Error::new(
                io::ErrorKind::OutOfMemory,
                "stream is too large to read into memory",
            )
        })?;
        // A malformed file can claim any length for a stream; the chain
        // behind it bounds what can actually be read, so reserve no more
        // than that.
        let capacity = {
            let capacity = stream_capacity(self.minialloc, self.stream_id)?;
            usize::try_from(capacity.saturating_sub(position)).unwrap_or(usize::MAX)
        };
        buf.try_reserve(remaining.min(capacity)).map_err(|_| {
            io::Error::new(
                io::ErrorKind::OutOfMemory,
                "not enough memory to read the stream",
            )
        })?;
        // The reserved memory is filled a piece at a time, so that a
        // malformed file claiming an enormous stream length doesn't cost a
        // zero fill of all of it before the read fails.
        let mut total = 0;
        while total < remaining {
            // Past the chain's capacity, a one byte read reports the
            // short chain without growing the buffer any further.
            let chunk = (remaining - total)
                .min(READ_TO_END_CHUNK)
                .min(capacity.saturating_sub(total).max(1));
            let start = buf.len();
            buf.resize(start + chunk, 0);
            let num_bytes = match self.read_direct(&mut buf[start..]) {
                Ok(num_bytes) => num_bytes,
                Err(error) => {
                    buf.truncate(start);
                    return Err(error);
                }
            };
            buf.truncate(start + num_bytes);
            if num_bytes == 0 {
                break;
            }
            total += num_bytes;
        }
        Ok(buffered_len + total)
    }
}

fn stream_capacity<F: Read + Seek>(
    minialloc: &mut MiniAllocator<F>,
    stream_id: u32,
) -> io::Result<u64> {
    let (start_sector, stream_len) = {
        let dir_entry = minialloc.dir_entry(stream_id);
        (dir_entry.start_sector, dir_entry.stream_len)
    };
    if start_sector == consts::END_OF_CHAIN {
        Ok(0)
    } else if stream_len < consts::MINI_STREAM_CUTOFF as u64 {
        Ok(minialloc.open_mini_chain(start_sector)?.len())
    } else {
        Ok(minialloc.open_chain(start_sector)?.len())
    }
}

fn read_data_from_stream<F: Read + Seek>(
    minialloc: &mut MiniAllocator<F>,
    stream_id: u32,
    buf_offset_from_start: u64,
    buf: &mut [u8],
) -> io::Result<usize> {
    let (start_sector, stream_len) = {
        let dir_entry = minialloc.dir_entry(stream_id);
        debug_assert_eq!(dir_entry.obj_type, ObjType::Stream);
        (dir_entry.start_sector, dir_entry.stream_len)
    };
    let num_bytes = if buf_offset_from_start >= stream_len {
        0
    } else {
        let remaining = stream_len - buf_offset_from_start;
        if remaining < buf.len() as u64 {
            remaining as usize
        } else {
            buf.len()
        }
    };
    if num_bytes > 0 {
        if stream_len < consts::MINI_STREAM_CUTOFF as u64 {
            let mut chain = minialloc.open_mini_chain(start_sector)?;
            chain.seek(SeekFrom::Start(buf_offset_from_start))?;
            chain.read_exact(&mut buf[..num_bytes])?;
        } else {
            let mut chain = minialloc.open_chain(start_sector)?;
            chain.seek(SeekFrom::Start(buf_offset_from_start))?;
            chain.read_exact(&mut buf[..num_bytes])?;
        }
    }
    Ok(num_bytes)
}
