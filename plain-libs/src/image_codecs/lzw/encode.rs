use crate::image_codecs::lzw::error::StreamResult;
use crate::image_codecs::lzw::error::{BufferResult, LzwError, LzwStatus, VectorResult};
use crate::image_codecs::lzw::{
    BitOrder, Code, MAX_CODESIZE, MAX_ENTRIES, STREAM_BUF_SIZE, StreamBuf,
};
use std::io::{self, BufRead, Write};
use std::{boxed::Box, vec::Vec};
pub struct Encoder {
    state: Box<dyn Stateful + Send + 'static>,
}
pub struct IntoStream<'d, W> {
    encoder: &'d mut Encoder,
    writer: W,
    buffer: Option<StreamBuf<'d>>,
    default_size: usize,
}
pub struct IntoVec<'d> {
    encoder: &'d mut Encoder,
    vector: &'d mut Vec<u8>,
}
trait Stateful {
    fn advance(&mut self, inp: &[u8], out: &mut [u8]) -> BufferResult;
    fn mark_ended(&mut self) -> bool;
    fn restart(&mut self);
    fn reset(&mut self);
}
struct EncodeState<B: Buffer> {
    min_size: u8,
    tree: Tree,
    has_ended: bool,
    is_tiff: bool,
    current_code: Code,
    clear_code: Code,
    buffer: B,
}
struct MsbBuffer {
    code_size: u8,
    buffer: u64,
    bits_in_buffer: u8,
}
struct LsbBuffer {
    code_size: u8,
    buffer: u64,
    bits_in_buffer: u8,
}
trait Buffer {
    fn new(size: u8) -> Self;
    fn reset(&mut self, min_size: u8);
    fn clear(&mut self, min_size: u8);
    fn buffer_code(&mut self, code: Code);
    fn push_out(&mut self, out: &mut &mut [u8]) -> bool;
    fn flush_out(&mut self, out: &mut &mut [u8]) -> bool;
    fn buffer_pad(&mut self);
    fn bump_code_size(&mut self);
    fn max_code(&self) -> Code;
    fn code_size(&self) -> u8;
}
#[derive(Default)]
struct Tree {
    simples: Vec<Simple>,
    complex: Vec<Full>,
    keys: Vec<CompressedKey>,
}
#[derive(Clone, Copy)]
enum FullKey {
    NoSuccessor,
    Simple(u16),
    Full(u16),
}
#[derive(Clone, Copy)]
struct CompressedKey(u16);
const SHORT: usize = 16;
#[derive(Clone, Copy)]
struct Simple {
    codes: [Code; SHORT],
    chars: [u8; SHORT],
    count: u8,
}
#[derive(Clone, Copy)]
struct Full {
    char_continuation: [Code; 256],
}
#[derive(Clone, Debug)]
pub struct Configuration {
    order: BitOrder,
    size: u8,
    tiff: bool,
}
impl Configuration {
    pub fn new(order: BitOrder, size: u8) -> Self {
        super::assert_encode_size(size);
        Configuration {
            order,
            size,
            tiff: false,
        }
    }
    pub fn with_tiff_size_switch(order: BitOrder, size: u8) -> Self {
        super::assert_encode_size(size);
        Configuration {
            order,
            size,
            tiff: true,
        }
    }
    pub fn build(self) -> Encoder {
        Encoder {
            state: Encoder::from_configuration(&self),
        }
    }
}
impl Encoder {
    pub fn new(order: BitOrder, size: u8) -> Self {
        Configuration::new(order, size).build()
    }
    pub fn with_tiff_size_switch(order: BitOrder, size: u8) -> Self {
        Configuration::with_tiff_size_switch(order, size).build()
    }
    fn from_configuration(cfg: &Configuration) -> Box<dyn Stateful + Send + 'static> {
        match cfg.order {
            BitOrder::Lsb => {
                let mut state = EncodeState::<LsbBuffer>::new(cfg.size);
                state.is_tiff = cfg.tiff;
                Box::new(state)
            }
            BitOrder::Msb => {
                let mut state = EncodeState::<MsbBuffer>::new(cfg.size);
                state.is_tiff = cfg.tiff;
                Box::new(state)
            }
        }
    }
    pub fn encode_bytes(&mut self, inp: &[u8], out: &mut [u8]) -> BufferResult {
        self.state.advance(inp, out)
    }
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<u8>, LzwError> {
        let mut output = Vec::new();
        self.into_vec(&mut output).encode_all(data).status?;
        Ok(output)
    }
    pub fn into_stream<W: Write>(&mut self, writer: W) -> IntoStream<'_, W> {
        IntoStream {
            encoder: self,
            writer,
            buffer: None,
            default_size: STREAM_BUF_SIZE,
        }
    }
    pub fn into_vec<'lt>(&'lt mut self, vec: &'lt mut Vec<u8>) -> IntoVec<'lt> {
        IntoVec {
            encoder: self,
            vector: vec,
        }
    }
    pub fn finish(&mut self) {
        self.state.mark_ended();
    }
    pub(crate) fn restart(&mut self) {
        self.state.restart()
    }
    pub fn reset(&mut self) {
        self.state.reset()
    }
}
impl<'d, W: Write> IntoStream<'d, W> {
    pub fn encode(&mut self, read: impl BufRead) -> StreamResult {
        self.encode_part(read, false)
    }
    pub fn encode_all(mut self, read: impl BufRead) -> StreamResult {
        self.encode_part(read, true)
    }
    pub fn set_buffer_size(&mut self, size: usize) {
        assert_ne!(size, 0, "Attempted to set empty buffer");
        self.default_size = size;
    }
    pub fn set_buffer(&mut self, buffer: &'d mut [u8]) {
        assert_ne!(buffer.len(), 0, "Attempted to set empty buffer");
        self.buffer = Some(StreamBuf::Borrowed(buffer));
    }
    fn encode_part(&mut self, mut read: impl BufRead, finish: bool) -> StreamResult {
        let IntoStream {
            encoder,
            writer,
            buffer,
            default_size,
        } = self;
        enum Progress {
            Ok,
            Done,
        }
        let mut bytes_read = 0;
        let mut bytes_written = 0;
        let read_bytes = &mut bytes_read;
        let write_bytes = &mut bytes_written;
        let outbuf: &mut [u8] =
            match { buffer.get_or_insert_with(|| StreamBuf::Owned(vec![0u8; *default_size])) } {
                StreamBuf::Borrowed(slice) => &mut *slice,
                StreamBuf::Owned(vec) => &mut *vec,
            };
        assert!(!outbuf.is_empty());
        let once = move || {
            let data = read.fill_buf()?;
            if data.is_empty() {
                if finish {
                    encoder.finish();
                } else {
                    return Ok(Progress::Done);
                }
            }
            let result = encoder.encode_bytes(data, &mut outbuf[..]);
            *read_bytes += result.consumed_in;
            *write_bytes += result.consumed_out;
            read.consume(result.consumed_in);
            let done = result.status.map_err(|err| {
                io::Error::new(io::ErrorKind::InvalidData, &*format!("{:?}", err))
            })?;
            if let LzwStatus::Done = done {
                writer.write_all(&outbuf[..result.consumed_out])?;
                return Ok(Progress::Done);
            }
            if let LzwStatus::NoProgress = done {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "No more data but no end marker detected",
                ));
            }
            writer.write_all(&outbuf[..result.consumed_out])?;
            Ok(Progress::Ok)
        };
        let status = core::iter::repeat_with(once)
            .scan((), |(), result| match result {
                Ok(Progress::Ok) => Some(Ok(())),
                Err(err) => Some(Err(err)),
                Ok(Progress::Done) => None,
            })
            .fuse()
            .collect();
        StreamResult {
            bytes_read,
            bytes_written,
            status,
        }
    }
}
impl IntoVec<'_> {
    pub fn encode(&mut self, read: &[u8]) -> VectorResult {
        self.encode_part(read, false)
    }
    pub fn encode_all(mut self, read: &[u8]) -> VectorResult {
        self.encode_part(read, true)
    }
    fn grab_buffer(&mut self) -> (&mut [u8], &mut Encoder) {
        const CHUNK_SIZE: usize = 1 << 12;
        let decoder = &mut self.encoder;
        let length = self.vector.len();
        self.vector.reserve(CHUNK_SIZE);
        self.vector.resize(length + CHUNK_SIZE, 0u8);
        (&mut self.vector[length..], decoder)
    }
    fn encode_part(&mut self, part: &[u8], finish: bool) -> VectorResult {
        let mut result = VectorResult {
            consumed_in: 0,
            consumed_out: 0,
            status: Ok(LzwStatus::Ok),
        };
        enum Progress {
            Ok,
            Done,
        }
        let read_bytes = &mut result.consumed_in;
        let write_bytes = &mut result.consumed_out;
        let mut data = part;
        let once = move || {
            let (outbuf, encoder) = self.grab_buffer();
            if finish {
                encoder.finish();
            }
            let result = encoder.encode_bytes(data, &mut outbuf[..]);
            *read_bytes += result.consumed_in;
            *write_bytes += result.consumed_out;
            data = &data[result.consumed_in..];
            let unfilled = outbuf.len() - result.consumed_out;
            let filled = self.vector.len() - unfilled;
            self.vector.truncate(filled);
            let done = result.status?;
            if let LzwStatus::Done = done {
                Ok(Progress::Done)
            } else {
                Ok(Progress::Ok)
            }
        };
        let status: Result<(), _> = core::iter::repeat_with(once)
            .scan((), |(), result| match result {
                Ok(Progress::Ok) => Some(Ok(())),
                Err(err) => Some(Err(err)),
                Ok(Progress::Done) => None,
            })
            .fuse()
            .collect();
        if let Err(err) = status {
            result.status = Err(err);
        }
        result
    }
}
impl<B: Buffer> EncodeState<B> {
    fn new(min_size: u8) -> Self {
        let clear_code = 1 << min_size;
        let mut tree = Tree::default();
        tree.init(min_size);
        let mut state = EncodeState {
            min_size,
            tree,
            has_ended: false,
            is_tiff: false,
            current_code: clear_code,
            clear_code,
            buffer: B::new(min_size),
        };
        state.buffer_code(clear_code);
        state
    }
}
impl<B: Buffer> Stateful for EncodeState<B> {
    fn advance(&mut self, mut inp: &[u8], mut out: &mut [u8]) -> BufferResult {
        let c_in = inp.len();
        let c_out = out.len();
        let mut status = Ok(LzwStatus::Ok);
        'encoding: loop {
            if self.push_out(&mut out) {
                break;
            }
            if inp.is_empty() && self.has_ended {
                let end = self.end_code();
                if self.current_code != end {
                    if self.current_code != self.clear_code {
                        self.buffer_code(self.current_code);
                        if self.tree.keys.len() + usize::from(self.is_tiff)
                            > usize::from(self.buffer.max_code())
                            && self.buffer.code_size() < MAX_CODESIZE
                        {
                            self.buffer.bump_code_size();
                        }
                    }
                    self.buffer_code(end);
                    self.current_code = end;
                    self.buffer_pad();
                }
                break;
            }
            let mut next_code = None;
            let mut bytes = inp.iter();
            while let Some(&byte) = bytes.next() {
                if self.min_size < 8 && byte >= 1 << self.min_size {
                    status = Err(LzwError::InvalidCode);
                    break 'encoding;
                }
                inp = bytes.as_slice();
                match self.tree.iterate(self.current_code, byte) {
                    Ok(code) => self.current_code = code,
                    Err(_) => {
                        next_code = Some(self.current_code);
                        self.current_code = u16::from(byte);
                        break;
                    }
                }
            }
            match next_code {
                None => break,
                Some(code) => {
                    self.buffer_code(code);
                    if self.tree.keys.len() + usize::from(self.is_tiff)
                        > usize::from(self.buffer.max_code()) + 1
                        && self.buffer.code_size() < MAX_CODESIZE
                    {
                        self.buffer.bump_code_size();
                    }
                    if self.tree.keys.len() > MAX_ENTRIES {
                        self.buffer_code(self.clear_code);
                        self.tree.reset(self.min_size);
                        self.buffer.clear(self.min_size);
                    }
                }
            }
        }
        if inp.is_empty() && self.current_code == self.end_code() {
            if !self.flush_out(&mut out) {
                status = Ok(LzwStatus::Done);
            }
        }
        BufferResult {
            consumed_in: c_in - inp.len(),
            consumed_out: c_out - out.len(),
            status,
        }
    }
    fn mark_ended(&mut self) -> bool {
        core::mem::replace(&mut self.has_ended, true)
    }
    fn restart(&mut self) {
        self.has_ended = false;
    }
    fn reset(&mut self) {
        self.restart();
        self.current_code = self.clear_code;
        self.tree.reset(self.min_size);
        self.buffer.reset(self.min_size);
        self.buffer_code(self.clear_code);
    }
}
impl<B: Buffer> EncodeState<B> {
    fn push_out(&mut self, out: &mut &mut [u8]) -> bool {
        self.buffer.push_out(out)
    }
    fn flush_out(&mut self, out: &mut &mut [u8]) -> bool {
        self.buffer.flush_out(out)
    }
    fn end_code(&self) -> Code {
        self.clear_code + 1
    }
    fn buffer_pad(&mut self) {
        self.buffer.buffer_pad();
    }
    fn buffer_code(&mut self, code: Code) {
        self.buffer.buffer_code(code);
    }
}
impl Buffer for MsbBuffer {
    fn new(min_size: u8) -> Self {
        MsbBuffer {
            code_size: min_size + 1,
            buffer: 0,
            bits_in_buffer: 0,
        }
    }
    fn reset(&mut self, min_size: u8) {
        self.code_size = min_size + 1;
        self.buffer = 0;
        self.bits_in_buffer = 0;
    }
    fn clear(&mut self, min_size: u8) {
        self.code_size = min_size + 1;
    }
    fn buffer_code(&mut self, code: Code) {
        let shift = 64 - self.bits_in_buffer - self.code_size;
        self.buffer |= u64::from(code) << shift;
        self.bits_in_buffer += self.code_size;
    }
    fn push_out(&mut self, out: &mut &mut [u8]) -> bool {
        if self.bits_in_buffer + 2 * self.code_size < 64 {
            return false;
        }
        self.flush_out(out)
    }
    fn flush_out(&mut self, out: &mut &mut [u8]) -> bool {
        let want = usize::from(self.bits_in_buffer / 8);
        let count = want.min((*out).len());
        let (bytes, tail) = core::mem::replace(out, &mut []).split_at_mut(count);
        *out = tail;
        for b in bytes {
            *b = ((self.buffer & 0xff00_0000_0000_0000) >> 56) as u8;
            self.buffer <<= 8;
            self.bits_in_buffer -= 8;
        }
        count < want
    }
    fn buffer_pad(&mut self) {
        let to_byte = self.bits_in_buffer.wrapping_neg() & 0x7;
        self.bits_in_buffer += to_byte;
    }
    fn bump_code_size(&mut self) {
        self.code_size += 1;
    }
    fn max_code(&self) -> Code {
        (1 << self.code_size) - 1
    }
    fn code_size(&self) -> u8 {
        self.code_size
    }
}
impl Buffer for LsbBuffer {
    fn new(min_size: u8) -> Self {
        LsbBuffer {
            code_size: min_size + 1,
            buffer: 0,
            bits_in_buffer: 0,
        }
    }
    fn reset(&mut self, min_size: u8) {
        self.code_size = min_size + 1;
        self.buffer = 0;
        self.bits_in_buffer = 0;
    }
    fn clear(&mut self, min_size: u8) {
        self.code_size = min_size + 1;
    }
    fn buffer_code(&mut self, code: Code) {
        self.buffer |= u64::from(code) << self.bits_in_buffer;
        self.bits_in_buffer += self.code_size;
    }
    fn push_out(&mut self, out: &mut &mut [u8]) -> bool {
        if self.bits_in_buffer + 2 * self.code_size < 64 {
            return false;
        }
        self.flush_out(out)
    }
    fn flush_out(&mut self, out: &mut &mut [u8]) -> bool {
        let want = usize::from(self.bits_in_buffer / 8);
        let count = want.min((*out).len());
        let (bytes, tail) = core::mem::replace(out, &mut []).split_at_mut(count);
        *out = tail;
        for b in bytes {
            *b = (self.buffer & 0x0000_0000_0000_00ff) as u8;
            self.buffer >>= 8;
            self.bits_in_buffer -= 8;
        }
        count < want
    }
    fn buffer_pad(&mut self) {
        let to_byte = self.bits_in_buffer.wrapping_neg() & 0x7;
        self.bits_in_buffer += to_byte;
    }
    fn bump_code_size(&mut self) {
        self.code_size += 1;
    }
    fn max_code(&self) -> Code {
        (1 << self.code_size) - 1
    }
    fn code_size(&self) -> u8 {
        self.code_size
    }
}
impl Tree {
    fn init(&mut self, min_size: u8) {
        self.keys
            .resize((1 << min_size) + 2, FullKey::NoSuccessor.into());
        self.complex.push(Full {
            char_continuation: [0; 256],
        });
        let map_of_begin = self.complex.last_mut().unwrap();
        for ch in 0u16..256 {
            map_of_begin.char_continuation[usize::from(ch)] = ch;
        }
        self.keys[1 << min_size] = FullKey::Full(0).into();
    }
    fn reset(&mut self, min_size: u8) {
        self.simples.clear();
        self.keys.truncate((1 << min_size) + 2);
        self.complex.truncate(1);
        for k in self.keys[..(1 << min_size) + 2].iter_mut() {
            *k = FullKey::NoSuccessor.into();
        }
        self.keys[1 << min_size] = FullKey::Full(0).into();
    }
    fn at_key(&self, code: Code, ch: u8) -> Option<Code> {
        let key = self.keys[usize::from(code)];
        match FullKey::from(key) {
            FullKey::NoSuccessor => None,
            FullKey::Simple(idx) => {
                let nexts = &self.simples[usize::from(idx)];
                let successors = nexts
                    .codes
                    .iter()
                    .zip(nexts.chars.iter())
                    .take(usize::from(nexts.count));
                for (&scode, &sch) in successors {
                    if sch == ch {
                        return Some(scode);
                    }
                }
                None
            }
            FullKey::Full(idx) => {
                let full = &self.complex[usize::from(idx)];
                let precode = full.char_continuation[usize::from(ch)];
                if usize::from(precode) < MAX_ENTRIES {
                    Some(precode)
                } else {
                    None
                }
            }
        }
    }
    fn iterate(&mut self, code: Code, ch: u8) -> Result<Code, Code> {
        if let Some(next) = self.at_key(code, ch) {
            Ok(next)
        } else {
            Err(self.append(code, ch))
        }
    }
    fn append(&mut self, code: Code, ch: u8) -> Code {
        let next: Code = self.keys.len() as u16;
        let key = self.keys[usize::from(code)];
        match FullKey::from(key) {
            FullKey::NoSuccessor => {
                let new_key = FullKey::Simple(self.simples.len() as u16);
                self.simples.push(Simple::default());
                let simples = self.simples.last_mut().unwrap();
                simples.codes[0] = next;
                simples.chars[0] = ch;
                simples.count = 1;
                self.keys[usize::from(code)] = new_key.into();
            }
            FullKey::Simple(idx) if usize::from(self.simples[usize::from(idx)].count) < SHORT => {
                let nexts = &mut self.simples[usize::from(idx)];
                let nidx = usize::from(nexts.count);
                nexts.chars[nidx] = ch;
                nexts.codes[nidx] = next;
                nexts.count += 1;
            }
            FullKey::Simple(idx) => {
                let new_key = FullKey::Full(self.complex.len() as u16);
                let simples = &self.simples[usize::from(idx)];
                self.complex.push(Full {
                    char_continuation: [Code::max_value(); 256],
                });
                let full = self.complex.last_mut().unwrap();
                for (&pch, &pcont) in simples.chars.iter().zip(simples.codes.iter()) {
                    full.char_continuation[usize::from(pch)] = pcont;
                }
                self.keys[usize::from(code)] = new_key.into();
            }
            FullKey::Full(idx) => {
                let full = &mut self.complex[usize::from(idx)];
                full.char_continuation[usize::from(ch)] = next;
            }
        }
        self.keys.push(FullKey::NoSuccessor.into());
        next
    }
}
impl Default for FullKey {
    fn default() -> Self {
        FullKey::NoSuccessor
    }
}
impl Default for Simple {
    fn default() -> Self {
        Simple {
            codes: [0; SHORT],
            chars: [0; SHORT],
            count: 0,
        }
    }
}
impl From<CompressedKey> for FullKey {
    fn from(CompressedKey(key): CompressedKey) -> Self {
        match (key >> MAX_CODESIZE) & 0xf {
            0 => FullKey::Full(key & 0xfff),
            1 => FullKey::Simple(key & 0xfff),
            _ => FullKey::NoSuccessor,
        }
    }
}
impl From<FullKey> for CompressedKey {
    fn from(full: FullKey) -> Self {
        CompressedKey(match full {
            FullKey::NoSuccessor => 0x2000,
            FullKey::Simple(code) => 0x1000 | code,
            FullKey::Full(code) => code,
        })
    }
}
