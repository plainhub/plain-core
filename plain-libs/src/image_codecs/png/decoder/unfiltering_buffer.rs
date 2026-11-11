use super::stream::{DecodingError, FormatErrorInner};
use super::zlib::UnfilterBuf;
use crate::image_codecs::png::Info;
use crate::image_codecs::png::common::BytesPerPixel;
use crate::image_codecs::png::filter::{RowFilter, unfilter};
pub(crate) struct UnfilteringBuffer {
    data_stream: Vec<u8>,
    prev_row: PrevRow,
    current_start: usize,
    filled: usize,
    available: usize,
    shift_back_limit: usize,
    remaining_bytes: u64,
    scratch_buffer: Vec<u8>,
}
impl UnfilteringBuffer {
    pub const GROWTH_BYTES: usize = 8 * 1024;
    #[cfg(not(debug_assertions))]
    fn debug_assert_invariants(&self) {}
    #[cfg(debug_assertions)]
    fn debug_assert_invariants(&self) {
        if let PrevRow::InPlace(prev_start) = &self.prev_row {
            debug_assert!(*prev_start <= self.current_start);
        }
        debug_assert!(self.current_start <= self.filled);
        debug_assert!(self.available <= self.filled);
        debug_assert!(self.filled <= self.data_stream.len());
    }
    pub fn new(info: &Info<'_>) -> Self {
        let data_stream_capacity = {
            let max_data = info
                .checked_raw_row_length()
                .and_then(|v| v.checked_mul(info.height.min(128) as usize))
                .and_then(|v| checked_next_multiple_of(v, 256))
                .unwrap_or(usize::MAX);
            max_data.min(128 * 1024)
        };
        let shift_back_limit = {
            let rowlen_pot = info
                .checked_raw_row_length()
                .and_then(|v| v.checked_mul(4))
                .and_then(|v| checked_next_multiple_of(v, 64))
                .unwrap_or(isize::MAX as usize);
            rowlen_pot.max(128 * 1024)
        };
        let result = Self {
            data_stream: Vec::with_capacity(data_stream_capacity),
            prev_row: PrevRow::None,
            current_start: 0,
            filled: 0,
            available: 0,
            shift_back_limit,
            remaining_bytes: u64::MAX,
            scratch_buffer: Vec::new(),
        };
        result.debug_assert_invariants();
        result
    }
    pub fn reset_prev_row(&mut self) {
        if let PrevRow::Scratch(buf) = &mut self.prev_row {
            self.scratch_buffer = std::mem::take(buf);
        }
        self.prev_row = PrevRow::None;
        self.debug_assert_invariants();
    }
    pub fn start_frame(&mut self, frame_bytes: u64) {
        self.data_stream.clear();
        self.prev_row = PrevRow::None;
        self.current_start = 0;
        self.filled = 0;
        self.available = 0;
        self.remaining_bytes = frame_bytes;
    }
    pub fn remaining_bytes(&self) -> u64 {
        self.remaining_bytes
    }
    pub fn prev_row(&self) -> &[u8] {
        self.prev_row
            .as_slice(&self.data_stream[..self.current_start])
    }
    pub fn mutable_len_of_curr_row(&self) -> usize {
        self.available.saturating_sub(self.current_start)
    }
    pub fn readable_len_of_curr_row(&self) -> usize {
        self.filled - self.current_start
    }
    pub fn with_unfilled_buffer<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut UnfilterBuf<'_>) -> T,
    {
        let discard_size = self.available.min(match &self.prev_row {
            PrevRow::None | PrevRow::Scratch(_) => self.current_start,
            PrevRow::InPlace(prev_start) => *prev_start,
        });
        if discard_size >= self.shift_back_limit && self.filled >= self.data_stream.len() / 2 {
            self.shift_buffer_left(discard_size);
        }
        if self.filled + Self::GROWTH_BYTES > self.data_stream.len() {
            self.data_stream.resize(self.filled + Self::GROWTH_BYTES, 0);
        }
        if self.remaining_bytes < usize::MAX as u64
            && self.filled.saturating_add(self.remaining_bytes as usize) < self.data_stream.len()
        {
            self.data_stream
                .resize(self.filled + self.remaining_bytes as usize, 0);
        }
        let old_filled = self.filled;
        let ret = f(&mut UnfilterBuf {
            buffer: &mut self.data_stream,
            filled: &mut self.filled,
            available: &mut self.available,
        });
        assert!(self.filled >= old_filled);
        self.remaining_bytes -= (self.filled - old_filled) as u64;
        if self.remaining_bytes == 0 {
            self.available = self.filled;
        }
        self.debug_assert_invariants();
        ret
    }
    fn shift_buffer_left(&mut self, discard_size: usize) {
        assert!(discard_size <= self.available);
        if let Some(16..) = self.data_stream.len().checked_sub(self.filled) {
            self.data_stream.copy_within(discard_size..self.filled, 0);
        } else {
            self.data_stream.copy_within(discard_size.., 0);
        }
        self.current_start -= discard_size;
        self.available -= discard_size;
        self.filled -= discard_size;
        match &mut self.prev_row {
            PrevRow::None | PrevRow::Scratch(_) => (),
            PrevRow::InPlace(prev_start) => *prev_start -= discard_size,
        }
    }
    fn curr_row_filter(&self) -> Result<RowFilter, DecodingError> {
        let filter = self.data_stream[self.current_start];
        RowFilter::from_u8(filter).ok_or(DecodingError::Format(
            FormatErrorInner::UnknownFilterMethod(filter).into(),
        ))
    }
    pub fn unfilter_curr_row_in_place(
        &mut self,
        rowlen: usize,
        bpp: BytesPerPixel,
    ) -> Result<(), DecodingError> {
        debug_assert!(rowlen >= 2);
        debug_assert!(self.mutable_len_of_curr_row() >= rowlen);
        let filter = self.curr_row_filter()?;
        let (prev, row) = self.data_stream.split_at_mut(self.current_start);
        let prev: &[u8] = self.prev_row.as_slice(prev);
        let row = &mut row[1..rowlen];
        debug_assert!(prev.is_empty() || prev.len() == row.len());
        unfilter(filter, bpp, prev, row);
        self.reset_prev_row();
        self.prev_row = PrevRow::InPlace(self.current_start + 1);
        self.current_start += rowlen;
        self.debug_assert_invariants();
        Ok(())
    }
    pub fn unfilter_curr_row_using_scratch_buffer(
        &mut self,
        rowlen: usize,
        bpp: BytesPerPixel,
    ) -> Result<(), DecodingError> {
        debug_assert!(rowlen >= 2);
        debug_assert!(self.readable_len_of_curr_row() >= rowlen);
        debug_assert!(self.mutable_len_of_curr_row() < rowlen);
        let filter = self.curr_row_filter()?;
        let mut row = std::mem::take(&mut self.scratch_buffer);
        row.resize(rowlen - 1, 0);
        row.as_mut_slice()
            .copy_from_slice(&self.data_stream[self.current_start + 1..][..rowlen - 1]);
        let prev = self.prev_row();
        debug_assert!(prev.is_empty() || prev.len() == (rowlen - 1));
        unfilter(filter, bpp, prev, &mut row);
        self.reset_prev_row();
        self.prev_row = PrevRow::Scratch(row);
        self.current_start += rowlen;
        self.debug_assert_invariants();
        Ok(())
    }
}
enum PrevRow {
    None,
    InPlace(usize),
    Scratch(Vec<u8>),
}
impl PrevRow {
    fn as_slice<'a>(&'a self, buf: &'a [u8]) -> &'a [u8] {
        match self {
            PrevRow::None => &[],
            PrevRow::InPlace(prev_start) => &buf[*prev_start..],
            PrevRow::Scratch(scratch) => scratch.as_slice(),
        }
    }
}
fn checked_next_multiple_of(val: usize, factor: usize) -> Option<usize> {
    if factor == 0 {
        return None;
    }
    let remainder = val % factor;
    if remainder > 0 {
        val.checked_add(factor - remainder)
    } else {
        Some(val)
    }
}
