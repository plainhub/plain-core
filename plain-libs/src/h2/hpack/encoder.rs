use super::table::{Index, Table};
use super::{Header, huffman};
use crate::http::header::{HeaderName, HeaderValue};
use bytes::{BufMut, BytesMut};
const DEFAULT_MAX_ALLOWED_SIZE: usize = 4 * 1024;
#[derive(Debug)]
pub struct Encoder {
    table: Table,
    max_allowed_size: usize,
    size_update: Option<SizeUpdate>,
    scratch: BytesMut,
}
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum SizeUpdate {
    One(usize),
    Two(usize, usize),
}
impl Encoder {
    pub fn new(max_size: usize, capacity: usize) -> Encoder {
        let max_size = max_size.min(DEFAULT_MAX_ALLOWED_SIZE);
        Encoder {
            table: Table::new(max_size, capacity),
            max_allowed_size: DEFAULT_MAX_ALLOWED_SIZE,
            size_update: None,
            scratch: BytesMut::new(),
        }
    }
    pub(crate) fn take_scratch(&mut self) -> BytesMut {
        std::mem::take(&mut self.scratch)
    }
    pub(crate) fn return_scratch(&mut self, scratch: BytesMut) {
        self.scratch = scratch;
    }
    pub fn update_max_size(&mut self, val: usize) {
        let val = val.min(self.max_allowed_size);
        match self.size_update {
            Some(SizeUpdate::One(old)) => {
                if val > old {
                    if old > self.table.max_size() {
                        self.size_update = Some(SizeUpdate::One(val));
                    } else {
                        self.size_update = Some(SizeUpdate::Two(old, val));
                    }
                } else {
                    self.size_update = Some(SizeUpdate::One(val));
                }
            }
            Some(SizeUpdate::Two(min, _)) => {
                if val < min {
                    self.size_update = Some(SizeUpdate::One(val));
                } else {
                    self.size_update = Some(SizeUpdate::Two(min, val));
                }
            }
            None => {
                if val != self.table.max_size() {
                    self.size_update = Some(SizeUpdate::One(val));
                }
            }
        }
    }
    pub fn encode<I>(&mut self, headers: I, dst: &mut BytesMut)
    where
        I: IntoIterator<Item = Header<Option<HeaderName>>>,
    {
        let span = tracing::trace_span!("hpack::encode");
        let _e = span.enter();
        self.encode_size_updates(dst);
        let mut last_index = None;
        for header in headers {
            match header.reify() {
                Ok(header) => {
                    let index = self.table.index(header);
                    self.encode_header(&index, dst);
                    last_index = Some(index);
                }
                Err(value) => {
                    self . encode_header_without_name (last_index . as_ref () . unwrap_or_else (| | { panic ! ("encoding header without name, but no previous index to use for name") ; }) , & value , dst ,) ;
                }
            }
        }
    }
    fn encode_size_updates(&mut self, dst: &mut BytesMut) {
        match self.size_update.take() {
            Some(SizeUpdate::One(val)) => {
                self.table.resize(val);
                encode_size_update(val, dst);
            }
            Some(SizeUpdate::Two(min, max)) => {
                self.table.resize(min);
                self.table.resize(max);
                encode_size_update(min, dst);
                encode_size_update(max, dst);
            }
            None => {}
        }
    }
    fn encode_header(&mut self, index: &Index, dst: &mut BytesMut) {
        match *index {
            Index::Indexed(idx, _) => {
                encode_int(idx, 7, 0x80, dst);
            }
            Index::Name(idx, _) => {
                let header = self.table.resolve(index);
                encode_not_indexed(idx, header.value_slice(), header.is_sensitive(), dst);
            }
            Index::Inserted(_) => {
                let header = self.table.resolve(index);
                assert!(!header.is_sensitive());
                dst.put_u8(0b0100_0000);
                encode_str(header.name().as_slice(), dst);
                encode_str(header.value_slice(), dst);
            }
            Index::InsertedValue(idx, _) => {
                let header = self.table.resolve(index);
                assert!(!header.is_sensitive());
                encode_int(idx, 6, 0b0100_0000, dst);
                encode_str(header.value_slice(), dst);
            }
            Index::NotIndexed(_) => {
                let header = self.table.resolve(index);
                encode_not_indexed2(
                    header.name().as_slice(),
                    header.value_slice(),
                    header.is_sensitive(),
                    dst,
                );
            }
        }
    }
    fn encode_header_without_name(
        &mut self,
        last: &Index,
        value: &HeaderValue,
        dst: &mut BytesMut,
    ) {
        match *last {
            Index::Indexed(..)
            | Index::Name(..)
            | Index::Inserted(..)
            | Index::InsertedValue(..) => {
                let idx = self.table.resolve_idx(last);
                encode_not_indexed(idx, value.as_ref(), value.is_sensitive(), dst);
            }
            Index::NotIndexed(_) => {
                let last = self.table.resolve(last);
                encode_not_indexed2(
                    last.name().as_slice(),
                    value.as_ref(),
                    value.is_sensitive(),
                    dst,
                );
            }
        }
    }
}
impl Default for Encoder {
    fn default() -> Encoder {
        Encoder::new(4096, 0)
    }
}
fn encode_size_update(val: usize, dst: &mut BytesMut) {
    encode_int(val, 5, 0b0010_0000, dst)
}
fn encode_not_indexed(name: usize, value: &[u8], sensitive: bool, dst: &mut BytesMut) {
    if sensitive {
        encode_int(name, 4, 0b10000, dst);
    } else {
        encode_int(name, 4, 0, dst);
    }
    encode_str(value, dst);
}
fn encode_not_indexed2(name: &[u8], value: &[u8], sensitive: bool, dst: &mut BytesMut) {
    if sensitive {
        dst.put_u8(0b10000);
    } else {
        dst.put_u8(0);
    }
    encode_str(name, dst);
    encode_str(value, dst);
}
fn encode_str(val: &[u8], dst: &mut BytesMut) {
    if !val.is_empty() {
        let idx = position(dst);
        dst.put_u8(0);
        huffman::encode(val, dst);
        let huff_len = position(dst) - (idx + 1);
        if encode_int_one_byte(huff_len, 7) {
            dst[idx] = 0x80 | huff_len as u8;
        } else {
            const PLACEHOLDER_LEN: usize = 8;
            let mut buf = [0u8; PLACEHOLDER_LEN];
            let head_len = {
                let mut head_dst = &mut buf[..];
                encode_int(huff_len, 7, 0x80, &mut head_dst);
                PLACEHOLDER_LEN - head_dst.remaining_mut()
            };
            dst.put_slice(&buf[1..head_len]);
            for i in 0..huff_len {
                let src_i = idx + 1 + (huff_len - (i + 1));
                let dst_i = idx + head_len + (huff_len - (i + 1));
                dst[dst_i] = dst[src_i];
            }
            for i in 0..head_len {
                dst[idx + i] = buf[i];
            }
        }
    } else {
        dst.put_u8(0);
    }
}
fn encode_int<B: BufMut>(mut value: usize, prefix_bits: usize, first_byte: u8, dst: &mut B) {
    if encode_int_one_byte(value, prefix_bits) {
        dst.put_u8(first_byte | value as u8);
        return;
    }
    let low = (1 << prefix_bits) - 1;
    value -= low;
    dst.put_u8(first_byte | low as u8);
    while value >= 128 {
        dst.put_u8(0b1000_0000 | value as u8);
        value >>= 7;
    }
    dst.put_u8(value as u8);
}
fn encode_int_one_byte(value: usize, prefix_bits: usize) -> bool {
    value < (1 << prefix_bits) - 1
}
fn position(buf: &BytesMut) -> usize {
    buf.len()
}
