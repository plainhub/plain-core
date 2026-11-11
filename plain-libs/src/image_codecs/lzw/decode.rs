use crate::image_codecs::lzw::error::StreamResult;
use crate::image_codecs::lzw::error::{BufferResult, LzwError, LzwStatus, VectorResult};
use crate::image_codecs::lzw::{
    BitOrder, Code, MAX_CODESIZE, MAX_ENTRIES, STREAM_BUF_SIZE, StreamBuf,
};
use std::io::{self, BufRead, Write};
use std::{boxed::Box, vec, vec::Vec};
pub struct Decoder {
    state: Box<dyn Stateful + Send + 'static>,
}
pub struct IntoStream<'d, W> {
    decoder: &'d mut Decoder,
    writer: W,
    buffer: Option<StreamBuf<'d>>,
    default_size: usize,
}
pub struct IntoVec<'d> {
    decoder: &'d mut Decoder,
    vector: &'d mut Vec<u8>,
}
trait Stateful {
    fn advance(&mut self, inp: &[u8], out: &mut [u8]) -> BufferResult;
    fn has_ended(&self) -> bool;
    fn restart(&mut self);
    fn reset(&mut self);
}
#[derive(Clone)]
struct Link {
    prev: Code,
    byte: u8,
    first: u8,
}
#[derive(Clone)]
struct DerivationBase {
    code: Code,
    first: u8,
}
#[derive(Default)]
struct MsbBuffer {
    bit_buffer: u64,
    code_mask: u16,
    code_size: u8,
    bits: u8,
}
#[derive(Default)]
struct LsbBuffer {
    bit_buffer: u64,
    code_mask: u16,
    code_size: u8,
    bits: u8,
}
trait CodeBuffer {
    fn new(min_size: u8) -> Self;
    fn reset(&mut self, min_size: u8);
    fn bump_code_size(&mut self);
    fn next_symbol(&mut self, inp: &mut &[u8]) -> Option<Code>;
    fn refill_bits(&mut self, inp: &mut &[u8]);
    fn peek_bits(&self, code: &mut [Code; BURST]) -> usize;
    fn consume_bits(&mut self, code_cnt: u8);
    fn max_code(&self) -> Code;
    fn code_size(&self) -> u8;
}
trait CodegenConstants {
    const YIELD_ON_FULL: bool;
}
struct DecodeState<CodeBuffer, Constants: CodegenConstants> {
    min_size: u8,
    table: Table,
    buffer: Buffer,
    last: Option<DerivationBase>,
    next_code: Code,
    clear_code: Code,
    end_code: Code,
    has_ended: bool,
    is_tiff: bool,
    implicit_reset: bool,
    code_buffer: CodeBuffer,
    constants: core::marker::PhantomData<Constants>,
}
const BURST: usize = 6;
struct Buffer {
    bytes: Box<[u8]>,
    read_mark: usize,
    write_mark: usize,
}
struct Table {
    inner: Vec<Link>,
    depths: Vec<u16>,
}
#[derive(Clone, Debug)]
pub struct Configuration {
    order: BitOrder,
    size: u8,
    tiff: bool,
    yield_on_full: bool,
}
impl Configuration {
    pub fn new(order: BitOrder, size: u8) -> Self {
        super::assert_decode_size(size);
        Configuration {
            order,
            size,
            tiff: false,
            yield_on_full: false,
        }
    }
    pub fn with_tiff_size_switch(order: BitOrder, size: u8) -> Self {
        super::assert_decode_size(size);
        Configuration {
            order,
            size,
            tiff: true,
            yield_on_full: false,
        }
    }
    pub fn with_yield_on_full_buffer(self, do_yield: bool) -> Self {
        Configuration {
            yield_on_full: do_yield,
            ..self
        }
    }
    pub fn build(self) -> Decoder {
        Decoder {
            state: Decoder::from_configuration(&self),
        }
    }
}
impl Decoder {
    pub fn new(order: BitOrder, size: u8) -> Self {
        Configuration::new(order, size).build()
    }
    pub fn with_tiff_size_switch(order: BitOrder, size: u8) -> Self {
        Configuration::with_tiff_size_switch(order, size).build()
    }
    fn from_configuration(configuration: &Configuration) -> Box<dyn Stateful + Send + 'static> {
        struct NoYield;
        struct YieldOnFull;
        impl CodegenConstants for NoYield {
            const YIELD_ON_FULL: bool = false;
        }
        impl CodegenConstants for YieldOnFull {
            const YIELD_ON_FULL: bool = true;
        }
        type Boxed = Box<dyn Stateful + Send + 'static>;
        match (configuration.order, configuration.yield_on_full) {
            (BitOrder::Lsb, false) => {
                let mut state =
                    Box::new(DecodeState::<LsbBuffer, NoYield>::new(configuration.size));
                state.is_tiff = configuration.tiff;
                state as Boxed
            }
            (BitOrder::Lsb, true) => {
                let mut state = Box::new(DecodeState::<LsbBuffer, YieldOnFull>::new(
                    configuration.size,
                ));
                state.is_tiff = configuration.tiff;
                state as Boxed
            }
            (BitOrder::Msb, false) => {
                let mut state =
                    Box::new(DecodeState::<MsbBuffer, NoYield>::new(configuration.size));
                state.is_tiff = configuration.tiff;
                state as Boxed
            }
            (BitOrder::Msb, true) => {
                let mut state = Box::new(DecodeState::<MsbBuffer, YieldOnFull>::new(
                    configuration.size,
                ));
                state.is_tiff = configuration.tiff;
                state as Boxed
            }
        }
    }
    pub fn decode_bytes(&mut self, inp: &[u8], out: &mut [u8]) -> BufferResult {
        self.state.advance(inp, out)
    }
    pub fn decode(&mut self, data: &[u8]) -> Result<Vec<u8>, LzwError> {
        let mut output = vec![];
        self.into_vec(&mut output).decode_all(data).status?;
        Ok(output)
    }
    pub fn into_stream<W: Write>(&mut self, writer: W) -> IntoStream<'_, W> {
        IntoStream {
            decoder: self,
            writer,
            buffer: None,
            default_size: STREAM_BUF_SIZE,
        }
    }
    pub fn into_vec<'lt>(&'lt mut self, vec: &'lt mut Vec<u8>) -> IntoVec<'lt> {
        IntoVec {
            decoder: self,
            vector: vec,
        }
    }
    pub fn has_ended(&self) -> bool {
        self.state.has_ended()
    }
    pub(crate) fn restart(&mut self) {
        self.state.restart();
    }
    pub fn reset(&mut self) {
        self.state.reset();
    }
}
impl<'d, W: Write> IntoStream<'d, W> {
    pub fn decode(&mut self, read: impl BufRead) -> StreamResult {
        self.decode_part(read, false)
    }
    pub fn decode_all(mut self, read: impl BufRead) -> StreamResult {
        self.decode_part(read, true)
    }
    pub fn set_buffer_size(&mut self, size: usize) {
        assert_ne!(size, 0, "Attempted to set empty buffer");
        self.default_size = size;
    }
    pub fn set_buffer(&mut self, buffer: &'d mut [u8]) {
        assert_ne!(buffer.len(), 0, "Attempted to set empty buffer");
        self.buffer = Some(StreamBuf::Borrowed(buffer));
    }
    fn decode_part(&mut self, mut read: impl BufRead, must_finish: bool) -> StreamResult {
        let IntoStream {
            decoder,
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
            let result = decoder.decode_bytes(data, &mut outbuf[..]);
            *read_bytes += result.consumed_in;
            *write_bytes += result.consumed_out;
            read.consume(result.consumed_in);
            let done = result.status.map_err(|err| {
                io::Error::new(io::ErrorKind::InvalidData, &*format!("{:?}", err))
            })?;
            if let LzwStatus::NoProgress = done {
                debug_assert_eq!(
                    result.consumed_out, 0,
                    "No progress means we have not decoded any data"
                );
                if must_finish {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "No more data but no end marker detected",
                    ));
                } else {
                    return Ok(Progress::Done);
                }
            }
            writer.write_all(&outbuf[..result.consumed_out])?;
            Ok(if let LzwStatus::Done = done {
                Progress::Done
            } else {
                Progress::Ok
            })
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
    pub fn decode(&mut self, read: &[u8]) -> VectorResult {
        self.decode_part(read, false)
    }
    pub fn decode_all(mut self, read: &[u8]) -> VectorResult {
        self.decode_part(read, true)
    }
    fn grab_buffer(&mut self) -> (&mut [u8], &mut Decoder) {
        const CHUNK_SIZE: usize = 1 << 12;
        let decoder = &mut self.decoder;
        let length = self.vector.len();
        self.vector.reserve(CHUNK_SIZE);
        self.vector.resize(length + CHUNK_SIZE, 0u8);
        (&mut self.vector[length..], decoder)
    }
    fn decode_part(&mut self, part: &[u8], must_finish: bool) -> VectorResult {
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
            let (outbuf, decoder) = self.grab_buffer();
            let result = decoder.decode_bytes(data, &mut outbuf[..]);
            *read_bytes += result.consumed_in;
            *write_bytes += result.consumed_out;
            data = &data[result.consumed_in..];
            let unfilled = outbuf.len() - result.consumed_out;
            let filled = self.vector.len() - unfilled;
            self.vector.truncate(filled);
            match result.status {
                Err(err) => Err(err),
                Ok(LzwStatus::NoProgress) if must_finish => Err(LzwError::InvalidCode),
                Ok(LzwStatus::NoProgress) | Ok(LzwStatus::Done) => Ok(Progress::Done),
                Ok(LzwStatus::Ok) => Ok(Progress::Ok),
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
impl<C: CodeBuffer, CgC: CodegenConstants> DecodeState<C, CgC> {
    fn new(min_size: u8) -> Self {
        DecodeState {
            min_size,
            table: Table::new(),
            buffer: Buffer::new(),
            last: None,
            clear_code: 1 << min_size,
            end_code: (1 << min_size) + 1,
            next_code: (1 << min_size) + 2,
            has_ended: false,
            is_tiff: false,
            implicit_reset: true,
            code_buffer: CodeBuffer::new(min_size),
            constants: core::marker::PhantomData,
        }
    }
    fn init_tables(&mut self) {
        self.code_buffer.reset(self.min_size);
        self.next_code = (1 << self.min_size) + 2;
        self.table.init(self.min_size);
    }
    fn reset_tables(&mut self) {
        self.code_buffer.reset(self.min_size);
        self.next_code = (1 << self.min_size) + 2;
        self.table.clear(self.min_size);
    }
}
impl<C: CodeBuffer, CgC: CodegenConstants> Stateful for DecodeState<C, CgC> {
    fn has_ended(&self) -> bool {
        self.has_ended
    }
    fn restart(&mut self) {
        self.has_ended = false;
    }
    fn reset(&mut self) {
        self.table.init(self.min_size);
        self.next_code = (1 << self.min_size) + 2;
        self.buffer.read_mark = 0;
        self.buffer.write_mark = 0;
        self.last = None;
        self.restart();
        self.code_buffer = CodeBuffer::new(self.min_size);
    }
    fn advance(&mut self, mut inp: &[u8], mut out: &mut [u8]) -> BufferResult {
        if self.has_ended {
            return BufferResult {
                consumed_in: 0,
                consumed_out: 0,
                status: Ok(LzwStatus::Done),
            };
        }
        let o_in = inp.len();
        let o_out = out.len();
        let mut code_link = None;
        let mut status = Ok(LzwStatus::Ok);
        match self.last.take() {
            None => match self.next_symbol(&mut inp) {
                Some(code) if code > self.next_code => status = Err(LzwError::InvalidCode),
                Some(code) if code == self.next_code => status = Err(LzwError::InvalidCode),
                None => status = Ok(LzwStatus::NoProgress),
                Some(init_code) => {
                    if init_code == self.clear_code {
                        self.init_tables();
                    } else if init_code == self.end_code {
                        self.has_ended = true;
                        status = Ok(LzwStatus::Done);
                    } else if self.table.is_empty() {
                        if self.implicit_reset {
                            self.init_tables();
                            self.buffer.fill_reconstruct(&self.table, init_code);
                            let link = self.table.at(init_code).clone();
                            code_link = Some(DerivationBase {
                                code: init_code,
                                first: link.first,
                            });
                        } else {
                            status = Err(LzwError::InvalidCode);
                        }
                    } else {
                        self.buffer.fill_reconstruct(&self.table, init_code);
                        let link = self.table.at(init_code).clone();
                        code_link = Some(DerivationBase {
                            code: init_code,
                            first: link.first,
                        });
                    }
                }
            },
            Some(tup) => code_link = Some(tup),
        };
        let mut have_yet_to_decode_data = false;
        if code_link.is_some() {
            let remain = self.buffer.buffer();
            if remain.len() > out.len() {
                if out.is_empty() {
                    status = Ok(LzwStatus::NoProgress);
                } else {
                    out.copy_from_slice(&remain[..out.len()]);
                    self.buffer.consume(out.len());
                    out = &mut [];
                }
            } else if remain.is_empty() {
                status = Ok(LzwStatus::NoProgress);
                have_yet_to_decode_data = true;
            } else {
                let consumed = remain.len();
                out[..consumed].copy_from_slice(remain);
                self.buffer.consume(consumed);
                out = &mut out[consumed..];
                have_yet_to_decode_data = false;
            }
        }
        let mut last_decoded: Option<&[u8]> = None;
        if self.buffer.buffer().is_empty() {
            let mut burst = [0; BURST];
            let mut burst_byte_len = [0u16; BURST];
            let mut burst_byte = [0u8; BURST];
            let mut target: [&mut [u8]; BURST] = Default::default();
            loop {
                if CgC::YIELD_ON_FULL && out.is_empty() {
                    break;
                }
                let mut deriv = match code_link.take() {
                    Some(link) => link,
                    None => {
                        break;
                    }
                };
                self.refill_bits(&mut inp);
                let cnt = self.code_buffer.peek_bits(&mut burst);
                if cnt == 0 {
                    if have_yet_to_decode_data {
                        status = Ok(LzwStatus::NoProgress);
                    }
                    code_link = Some(deriv);
                    break;
                }
                debug_assert!(
                    self.table.inner.len() >= MAX_ENTRIES - usize::from(self.is_tiff)
                        || (self.code_buffer.code_size() == 1 && self.next_code < 4)
                        || (self.code_buffer.code_size() == 2 && self.next_code == 4)
                        || self.code_buffer.max_code() - Code::from(self.is_tiff) >= self.next_code,
                    "Table: {}, code_size: {}, next_code: {}, table_condition: {}",
                    self.table.is_full(),
                    self.code_buffer.code_size(),
                    self.next_code,
                    self.code_buffer.max_code() - Code::from(self.is_tiff),
                );
                let mut burst_size = 0;
                let left_before_size_switch = (self.code_buffer.max_code()
                    - Code::from(self.is_tiff))
                .saturating_sub(self.next_code);
                for b in &burst[..cnt] {
                    if burst_size > 0 {
                        let len = burst_byte_len[burst_size - 1];
                        let (into, tail) = out.split_at_mut(usize::from(len));
                        target[burst_size - 1] = into;
                        out = tail;
                    }
                    burst_size += 1;
                    if burst_size > usize::from(left_before_size_switch) {
                        break;
                    }
                    let read_code = *b;
                    if read_code == self.clear_code
                        || read_code == self.end_code
                        || read_code >= self.next_code
                    {
                        break;
                    }
                    let len = self.table.depths[usize::from(read_code)];
                    if out.len() < usize::from(len) {
                        break;
                    }
                    if CgC::YIELD_ON_FULL {
                        if out.len() == usize::from(len) {
                            break;
                        }
                    }
                    burst_byte_len[burst_size - 1] = len;
                }
                self.code_buffer.consume_bits(burst_size as u8);
                have_yet_to_decode_data = false;
                let (&new_code, burst) = burst[..burst_size].split_last().unwrap();
                let burst_targets = &mut target[..burst_size - 1];
                if !self.table.is_full() {
                    self.next_code += burst_targets.len() as u16;
                }
                for ((&burst, target), byte) in
                    burst.iter().zip(&mut *burst_targets).zip(&mut burst_byte)
                {
                    *byte = self.table.reconstruct(burst, target);
                }
                self.table.derive_burst(&mut deriv, burst, &burst_byte[..]);
                if new_code == self.clear_code {
                    self.reset_tables();
                    last_decoded = None;
                    break;
                }
                if new_code == self.end_code {
                    self.has_ended = true;
                    status = Ok(LzwStatus::Done);
                    last_decoded = None;
                    break;
                }
                if new_code > self.next_code {
                    status = Err(LzwError::InvalidCode);
                    last_decoded = None;
                    break;
                }
                let required_len = if new_code == self.next_code {
                    self.table.depths[usize::from(deriv.code)] + 1
                } else {
                    self.table.depths[usize::from(new_code)]
                };
                let have_next_code = new_code == self.next_code;
                if have_next_code {
                    if let Some(new_last) = target[..burst_size - 1].last_mut() {
                        let slice = core::mem::replace(new_last, &mut []);
                        last_decoded = Some(&*slice);
                    }
                }
                let cha;
                let is_in_buffer = usize::from(required_len) > out.len();
                if is_in_buffer {
                    if have_next_code {
                        if let Some(last) = last_decoded.take() {
                            self.buffer.bytes[..last.len()].copy_from_slice(last);
                            self.buffer.write_mark = last.len();
                            self.buffer.read_mark = last.len();
                        }
                        cha = self.buffer.fill_cscsc();
                    } else {
                        last_decoded = None;
                        cha = self.buffer.fill_reconstruct(&self.table, new_code);
                    }
                } else {
                    let (target, tail) = out.split_at_mut(usize::from(required_len));
                    out = tail;
                    if have_next_code {
                        let source = match last_decoded.take() {
                            Some(last) => last,
                            None => &self.buffer.bytes[..self.buffer.write_mark],
                        };
                        cha = source.get(0).map(|x| *x).unwrap_or(0);
                        target[..source.len()].copy_from_slice(source);
                        target[source.len()..][0] = cha;
                    } else {
                        cha = self.table.reconstruct(new_code, target);
                    }
                    last_decoded = Some(target);
                }
                if !self.table.is_full() {
                    self.table.derive(&deriv, cha);
                    if self.next_code >= self.code_buffer.max_code() - Code::from(self.is_tiff)
                        && self.code_buffer.code_size() < MAX_CODESIZE
                    {
                        self.bump_code_size();
                    }
                    self.next_code += 1;
                }
                code_link = Some(DerivationBase {
                    code: new_code,
                    first: cha,
                });
                if is_in_buffer || out.is_empty() {
                    break;
                }
            }
        }
        if let Some(tail) = last_decoded {
            self.buffer.bytes[..tail.len()].copy_from_slice(tail);
            self.buffer.write_mark = tail.len();
            self.buffer.read_mark = tail.len();
        }
        if o_in > inp.len() {
            if let Ok(LzwStatus::NoProgress) = status {
                status = Ok(LzwStatus::Ok);
            }
        }
        self.last = code_link;
        BufferResult {
            consumed_in: o_in.wrapping_sub(inp.len()),
            consumed_out: o_out.wrapping_sub(out.len()),
            status,
        }
    }
}
impl<C: CodeBuffer, CgC: CodegenConstants> DecodeState<C, CgC> {
    fn next_symbol(&mut self, inp: &mut &[u8]) -> Option<Code> {
        self.code_buffer.next_symbol(inp)
    }
    fn bump_code_size(&mut self) {
        self.code_buffer.bump_code_size()
    }
    fn refill_bits(&mut self, inp: &mut &[u8]) {
        self.code_buffer.refill_bits(inp)
    }
}
impl CodeBuffer for MsbBuffer {
    fn new(min_size: u8) -> Self {
        MsbBuffer {
            code_size: min_size + 1,
            code_mask: (1u16 << (min_size + 1)) - 1,
            bit_buffer: 0,
            bits: 0,
        }
    }
    fn reset(&mut self, min_size: u8) {
        self.code_size = min_size + 1;
        self.code_mask = (1 << self.code_size) - 1;
    }
    fn next_symbol(&mut self, inp: &mut &[u8]) -> Option<Code> {
        if self.bits < self.code_size {
            self.refill_bits(inp);
        }
        if self.bits < self.code_size {
            return None;
        }
        let mask = u64::from(self.code_mask);
        let rotbuf = self.bit_buffer.rotate_left(self.code_size.into());
        self.bit_buffer = rotbuf & !mask;
        self.bits -= self.code_size;
        Some((rotbuf & mask) as u16)
    }
    fn bump_code_size(&mut self) {
        self.code_size += 1;
        self.code_mask = (self.code_mask << 1) | 1;
    }
    fn refill_bits(&mut self, inp: &mut &[u8]) {
        let wish_count = (64 - self.bits) / 8;
        let mut buffer = [0u8; 8];
        let new_bits = match inp.get(..usize::from(wish_count)) {
            Some(bytes) => {
                buffer[..usize::from(wish_count)].copy_from_slice(bytes);
                *inp = &inp[usize::from(wish_count)..];
                wish_count * 8
            }
            None => {
                let new_bits = inp.len() * 8;
                buffer[..inp.len()].copy_from_slice(inp);
                *inp = &[];
                new_bits as u8
            }
        };
        self.bit_buffer |= u64::from_be_bytes(buffer) >> self.bits;
        self.bits += new_bits;
    }
    fn peek_bits(&self, code: &mut [Code; BURST]) -> usize {
        let mut bit_buffer = self.bit_buffer;
        let mask = u64::from(self.code_mask);
        let mut consumed = 0;
        let mut cnt = 0;
        for b in code {
            let consumed_after = consumed + self.code_size;
            if consumed_after > self.bits {
                break;
            }
            cnt += 1;
            consumed = consumed_after;
            let rotbuf = bit_buffer.rotate_left(self.code_size.into());
            *b = (rotbuf & mask) as u16;
            bit_buffer = rotbuf;
        }
        cnt
    }
    fn consume_bits(&mut self, code_cnt: u8) {
        let bits = self.code_size * code_cnt;
        debug_assert!(bits <= self.bits);
        if bits >= self.bits {
            self.bit_buffer = 0;
        } else {
            self.bit_buffer = self.bit_buffer << bits;
        }
        self.bits = self.bits.wrapping_sub(bits);
    }
    fn max_code(&self) -> Code {
        self.code_mask
    }
    fn code_size(&self) -> u8 {
        self.code_size
    }
}
impl CodeBuffer for LsbBuffer {
    fn new(min_size: u8) -> Self {
        LsbBuffer {
            code_size: min_size + 1,
            code_mask: (1u16 << (min_size + 1)) - 1,
            bit_buffer: 0,
            bits: 0,
        }
    }
    fn reset(&mut self, min_size: u8) {
        self.code_size = min_size + 1;
        self.code_mask = (1 << self.code_size) - 1;
    }
    fn next_symbol(&mut self, inp: &mut &[u8]) -> Option<Code> {
        if self.bits < self.code_size {
            self.refill_bits(inp);
        }
        if self.bits < self.code_size {
            return None;
        }
        let mask = u64::from(self.code_mask);
        let code = self.bit_buffer & mask;
        self.bit_buffer >>= self.code_size;
        self.bits -= self.code_size;
        Some(code as u16)
    }
    fn bump_code_size(&mut self) {
        self.code_size += 1;
        self.code_mask = (self.code_mask << 1) | 1;
    }
    fn refill_bits(&mut self, inp: &mut &[u8]) {
        let wish_count = (64 - self.bits) / 8;
        let mut buffer = [0u8; 8];
        let new_bits = match inp.get(..usize::from(wish_count)) {
            Some(bytes) => {
                buffer[..usize::from(wish_count)].copy_from_slice(bytes);
                *inp = &inp[usize::from(wish_count)..];
                wish_count * 8
            }
            None => {
                let new_bits = inp.len() * 8;
                buffer[..inp.len()].copy_from_slice(inp);
                *inp = &[];
                new_bits as u8
            }
        };
        self.bit_buffer |= u64::from_be_bytes(buffer).swap_bytes() << self.bits;
        self.bits += new_bits;
    }
    fn peek_bits(&self, code: &mut [Code; BURST]) -> usize {
        let mut bit_buffer = self.bit_buffer;
        let mask = u64::from(self.code_mask);
        let mut consumed = 0;
        let mut cnt = 0;
        for b in code {
            let consumed_after = consumed + self.code_size;
            if consumed_after > self.bits {
                break;
            }
            cnt += 1;
            consumed = consumed_after;
            *b = (bit_buffer & mask) as u16;
            bit_buffer = bit_buffer >> self.code_size;
        }
        cnt
    }
    fn consume_bits(&mut self, code_cnt: u8) {
        let bits = self.code_size * code_cnt;
        debug_assert!(bits <= self.bits);
        if bits >= self.bits {
            self.bit_buffer = 0;
        } else {
            self.bit_buffer = self.bit_buffer >> bits;
        }
        self.bits = self.bits.wrapping_sub(bits);
    }
    fn max_code(&self) -> Code {
        self.code_mask
    }
    fn code_size(&self) -> u8 {
        self.code_size
    }
}
impl Buffer {
    fn new() -> Self {
        Buffer {
            bytes: vec![0; MAX_ENTRIES].into_boxed_slice(),
            read_mark: 0,
            write_mark: 0,
        }
    }
    fn fill_cscsc(&mut self) -> u8 {
        self.bytes[self.write_mark] = self.bytes[0];
        self.write_mark += 1;
        self.read_mark = 0;
        self.bytes[0]
    }
    fn fill_reconstruct(&mut self, table: &Table, code: Code) -> u8 {
        self.write_mark = 0;
        self.read_mark = 0;
        let depth = table.depths[usize::from(code)];
        let mut memory = core::mem::replace(&mut self.bytes, Box::default());
        let out = &mut memory[..usize::from(depth)];
        let last = table.reconstruct(code, out);
        self.bytes = memory;
        self.write_mark = usize::from(depth);
        last
    }
    fn buffer(&self) -> &[u8] {
        &self.bytes[self.read_mark..self.write_mark]
    }
    fn consume(&mut self, amt: usize) {
        self.read_mark += amt;
    }
}
impl Table {
    fn new() -> Self {
        Table {
            inner: Vec::with_capacity(MAX_ENTRIES),
            depths: Vec::with_capacity(MAX_ENTRIES),
        }
    }
    fn clear(&mut self, min_size: u8) {
        let static_count = usize::from(1u16 << u16::from(min_size)) + 2;
        self.inner.truncate(static_count);
        self.depths.truncate(static_count);
    }
    fn init(&mut self, min_size: u8) {
        self.inner.clear();
        self.depths.clear();
        for i in 0..(1u16 << u16::from(min_size)) {
            self.inner.push(Link::base(i as u8));
            self.depths.push(1);
        }
        self.inner.push(Link::base(0));
        self.depths.push(0);
        self.inner.push(Link::base(0));
        self.depths.push(0);
    }
    fn at(&self, code: Code) -> &Link {
        &self.inner[usize::from(code)]
    }
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    fn is_full(&self) -> bool {
        self.inner.len() >= MAX_ENTRIES
    }
    fn derive(&mut self, from: &DerivationBase, byte: u8) {
        let link = from.derive(byte);
        let depth = self.depths[usize::from(from.code)] + 1;
        self.inner.push(link);
        self.depths.push(depth);
    }
    fn derive_burst(&mut self, from: &mut DerivationBase, burst: &[Code], first: &[u8]) {
        let mut depth_of = from.code;
        for &code in burst {
            let depth = self.depths[usize::from(depth_of)] + 1;
            self.depths.push(depth);
            depth_of = code;
        }
        let extensions = burst.iter().zip(first);
        self.inner.extend(extensions.map(|(&code, &first)| {
            let link = from.derive(first);
            from.code = code;
            from.first = first;
            link
        }));
    }
    fn reconstruct(&self, code: Code, out: &mut [u8]) -> u8 {
        let mut code_iter = code;
        let table = &self.inner[..=usize::from(code)];
        let first = table[usize::from(code)].first;
        let len = code_iter;
        for ch in out.iter_mut().rev() {
            let entry = &table[usize::from(code_iter)];
            code_iter = core::cmp::min(len, entry.prev);
            *ch = entry.byte;
        }
        first
    }
}
impl Link {
    fn base(byte: u8) -> Self {
        Link {
            prev: 0,
            byte,
            first: byte,
        }
    }
}
impl DerivationBase {
    fn derive(&self, byte: u8) -> Link {
        Link {
            prev: self.code,
            byte,
            first: self.first,
        }
    }
}
