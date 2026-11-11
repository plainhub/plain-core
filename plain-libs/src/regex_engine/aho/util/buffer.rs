use std::{vec, vec::Vec};
const DEFAULT_BUFFER_CAPACITY: usize = 64 * (1 << 10);
#[derive(Debug)]
pub(crate) struct Buffer {
    buf: Vec<u8>,
    min: usize,
    end: usize,
}
impl Buffer {
    pub(crate) fn new(min_buffer_len: usize) -> Buffer {
        let min = core::cmp::max(1, min_buffer_len);
        let capacity = core::cmp::max(min * 8, DEFAULT_BUFFER_CAPACITY);
        Buffer {
            buf: vec![0; capacity],
            min,
            end: 0,
        }
    }
    #[inline]
    pub(crate) fn buffer(&self) -> &[u8] {
        &self.buf[..self.end]
    }
    #[inline]
    pub(crate) fn min_buffer_len(&self) -> usize {
        self.min
    }
    fn free_buffer(&mut self) -> &mut [u8] {
        &mut self.buf[self.end..]
    }
    pub(crate) fn fill<R: std::io::Read>(&mut self, mut rdr: R) -> std::io::Result<bool> {
        let mut readany = false;
        loop {
            let readlen = rdr.read(self.free_buffer())?;
            if readlen == 0 {
                return Ok(readany);
            }
            readany = true;
            self.end += readlen;
            if self.buffer().len() >= self.min {
                return Ok(true);
            }
        }
    }
    pub(crate) fn roll(&mut self) {
        let roll_start = self
            .end
            .checked_sub(self.min)
            .expect("buffer capacity should be bigger than minimum amount");
        let roll_end = roll_start + self.min;
        assert!(roll_end <= self.end);
        self.buf.copy_within(roll_start..roll_end, 0);
        self.end = self.min;
    }
}
