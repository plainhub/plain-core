use self::{
    algorithm::CONFIGURATION_TABLE,
    hash_calc::{HashCalcVariant, RollHashCalc, StandardHashCalc},
    pending::Pending,
    sym_buf::SymBuf,
    trees_tbl::STATIC_LTREE,
    window::Window,
};
use crate::checksum::adler32::adler32;
use crate::compression::backend::{
    ADLER32_INITIAL_VALUE, CRC32_INITIAL_VALUE, DeflateFlush, MAX_WBITS, MIN_WBITS, ReturnCode,
    allocate::Allocator,
    c_api::{GzHeader, InternalState, ZChecksum, ZStream},
    crc32::{Crc32Fold, crc32},
    trace,
    weak_slice::{WeakArrayMut, WeakSliceMut},
};
use core::{ffi::CStr, marker::PhantomData, mem::MaybeUninit, ops::ControlFlow, ptr::NonNull};
mod algorithm;
mod compare256;
mod hash_calc;
mod longest_match;
mod pending;
mod slide_hash;
mod sym_buf;
mod trees_tbl;
mod window;
pub(crate) type Pos = u16;
#[repr(C)]
pub struct DeflateStream<'a> {
    pub(crate) next_in: *mut crate::compression::backend::c_api::Bytef,
    pub(crate) avail_in: crate::compression::backend::c_api::Uint,
    pub(crate) total_in: crate::compression::backend::c_api::ZSize,
    pub(crate) next_out: *mut crate::compression::backend::c_api::Bytef,
    pub(crate) avail_out: crate::compression::backend::c_api::Uint,
    pub(crate) total_out: crate::compression::backend::c_api::ZSize,
    pub(crate) msg: *const core::ffi::c_char,
    pub(crate) state: &'a mut State<'a>,
    pub(crate) alloc: Allocator<'a>,
    pub(crate) data_type: core::ffi::c_int,
    pub(crate) adler: crate::compression::backend::c_api::ZChecksum,
    pub(crate) reserved: crate::compression::backend::c_api::Ulong,
}
unsafe impl Sync for DeflateStream<'_> {}
unsafe impl Send for DeflateStream<'_> {}
impl<'a> DeflateStream<'a> {
    const _S: () = assert!(core::mem::size_of::<ZStream>() == core::mem::size_of::<Self>());
    const _A: () = assert!(core::mem::align_of::<ZStream>() == core::mem::align_of::<Self>());
    #[inline(always)]
    pub unsafe fn from_stream_mut(strm: *mut ZStream) -> Option<&'a mut Self> {
        {
            {
                let stream = unsafe { strm.as_ref() }?;
                if stream.zalloc.is_none() || stream.zfree.is_none() {
                    return None;
                }
                if stream.state.is_null() {
                    return None;
                }
            }
            unsafe { strm.cast::<DeflateStream>().as_mut() }
        }
    }

    fn as_z_stream_mut(&mut self) -> &mut ZStream {
        unsafe { &mut *(self as *mut DeflateStream as *mut ZStream) }
    }
}
pub(crate) const HASH_SIZE: usize = 65536;
const MAX_MEM_LEVEL: i32 = 9;
pub const DEF_MEM_LEVEL: i32 = if MAX_MEM_LEVEL > 8 { 8 } else { MAX_MEM_LEVEL };
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Method {
    #[default]
    Deflated = 8,
}
impl TryFrom<i32> for Method {
    type Error = ();
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            8 => Ok(Self::Deflated),
            _ => Err(()),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeflateConfig {
    pub level: i32,
    pub method: Method,
    pub window_bits: i32,
    pub mem_level: i32,
    pub strategy: Strategy,
}
impl DeflateConfig {}
impl Default for DeflateConfig {
    fn default() -> Self {
        Self {
            level: crate::compression::backend::c_api::Z_DEFAULT_COMPRESSION,
            method: Method::Deflated,
            window_bits: MAX_WBITS,
            mem_level: DEF_MEM_LEVEL,
            strategy: Strategy::Default,
        }
    }
}
pub fn init(stream: &mut ZStream, config: DeflateConfig) -> ReturnCode {
    let DeflateConfig {
        mut level,
        method: _,
        mut window_bits,
        mem_level,
        strategy,
    } = config;
    stream.msg = core::ptr::null_mut();
    if stream.zalloc.is_none() || stream.zfree.is_none() {
        stream.configure_default_rust_allocator()
    }
    if stream.zalloc.is_none() || stream.zfree.is_none() {
        return ReturnCode::StreamError;
    }
    if level == crate::compression::backend::c_api::Z_DEFAULT_COMPRESSION {
        level = 6;
    }
    let wrap = if window_bits < 0 {
        if window_bits < -MAX_WBITS {
            return ReturnCode::StreamError;
        }
        window_bits = -window_bits;
        0
    } else if window_bits > MAX_WBITS {
        window_bits -= 16;
        2
    } else {
        1
    };
    if (!(1..=MAX_MEM_LEVEL).contains(&mem_level))
        || !(MIN_WBITS..=MAX_WBITS).contains(&window_bits)
        || !(0..=9).contains(&level)
        || (window_bits == 8 && wrap != 1)
    {
        return ReturnCode::StreamError;
    }
    let window_bits = if window_bits == 8 {
        9
    } else {
        window_bits as usize
    };
    let alloc = Allocator {
        zalloc: stream.zalloc.unwrap(),
        zfree: stream.zfree.unwrap(),
        opaque: stream.opaque,
        _marker: PhantomData,
    };
    let lit_bufsize = 1 << (mem_level + 6);
    let allocs = DeflateAllocOffsets::new(window_bits, lit_bufsize);
    let Some(allocation_start) = alloc.allocate_slice_raw::<u8>(allocs.total_size) else {
        return ReturnCode::MemError;
    };
    let w_size = 1 << window_bits;
    let align_offset = (allocation_start.as_ptr() as usize).next_multiple_of(64)
        - (allocation_start.as_ptr() as usize);
    let buf = unsafe { allocation_start.as_ptr().add(align_offset) };
    let (window, prev, head, pending, sym_buf) = unsafe {
        let window_ptr: *mut u8 = buf.add(allocs.window_pos);
        window_ptr.write_bytes(0u8, 2 * w_size);
        let window = Window::from_raw_parts(window_ptr, window_bits);
        let prev_ptr = buf.add(allocs.prev_pos).cast::<Pos>();
        prev_ptr.write_bytes(0, w_size);
        let prev = WeakSliceMut::from_raw_parts_mut(prev_ptr, w_size);
        let head_ptr = buf.add(allocs.head_pos).cast::<[Pos; HASH_SIZE]>();
        head_ptr.write_bytes(0, 1);
        let head = WeakArrayMut::<Pos, HASH_SIZE>::from_ptr(head_ptr);
        let pending_ptr = buf.add(allocs.pending_pos).cast::<MaybeUninit<u8>>();
        let pending = Pending::from_raw_parts(pending_ptr, 4 * lit_bufsize);
        let sym_buf_ptr = buf.add(allocs.sym_buf_pos);
        let sym_buf = SymBuf::from_raw_parts(sym_buf_ptr, lit_bufsize);
        (window, prev, head, pending, sym_buf)
    };
    let state = State {
        status: Status::Init,
        w_size,
        window,
        prev,
        head,
        bit_writer: BitWriter::from_pending(pending),
        lit_bufsize,
        sym_buf,
        level: level as i8,
        strategy,
        last_flush: 0,
        wrap,
        strstart: 0,
        block_start: 0,
        block_open: 0,
        window_size: 0,
        insert: 0,
        matches: 0,
        opt_len: 0,
        static_len: 0,
        lookahead: 0,
        ins_h: 0,
        max_chain_length: 0,
        max_lazy_match: 0,
        good_match: 0,
        nice_match: 0,
        l_desc: TreeDesc::EMPTY,
        d_desc: TreeDesc::EMPTY,
        bl_desc: TreeDesc::EMPTY,
        crc_fold: Crc32Fold::new(),
        gzhead: None,
        gzindex: 0,
        match_start: 0,
        prev_match: 0,
        match_available: false,
        prev_length: 0,
        allocation_start,
        total_allocation_size: allocs.total_size,
        hash_calc_variant: HashCalcVariant::Standard,
        _cache_line_0: (),
        _cache_line_1: (),
        _cache_line_2: (),
        _cache_line_3: (),
        _padding_0: [0; 16],
    };
    let state_allocation = unsafe { buf.add(allocs.state_pos).cast::<State>() };
    unsafe { state_allocation.write(state) };
    stream.state = state_allocation.cast::<InternalState>();
    let Some(stream) = (unsafe { DeflateStream::from_stream_mut(stream) }) else {
        if cfg!(debug_assertions) {
            unreachable!("we should have initialized the stream properly");
        }
        return ReturnCode::StreamError;
    };
    reset(stream)
}

pub fn end<'a>(stream: &'a mut DeflateStream) -> Result<&'a mut ZStream, &'a mut ZStream> {
    let status = stream.state.status;
    let allocation_start = stream.state.allocation_start;
    let total_allocation_size = stream.state.total_allocation_size;
    let alloc = stream.alloc;
    let stream = stream.as_z_stream_mut();
    let _ = core::mem::replace(&mut stream.state, core::ptr::null_mut());
    unsafe { alloc.deallocate(allocation_start.as_ptr(), total_allocation_size) };
    match status {
        Status::Busy => Err(stream),
        _ => Ok(stream),
    }
}
pub fn reset(stream: &mut DeflateStream) -> ReturnCode {
    let ret = reset_keep(stream);
    if ret == ReturnCode::Ok {
        lm_init(stream.state);
    }
    ret
}
pub fn reset_keep(stream: &mut DeflateStream) -> ReturnCode {
    stream.total_in = 0;
    stream.total_out = 0;
    stream.msg = core::ptr::null_mut();
    stream.data_type = crate::compression::backend::c_api::Z_UNKNOWN;
    let state = &mut stream.state;
    state.bit_writer.pending.reset_keep();
    state.wrap = state.wrap.abs();
    state.status = match state.wrap {
        2 => Status::GZip,
        _ => Status::Init,
    };
    stream.adler = match state.wrap {
        2 => {
            state.crc_fold = Crc32Fold::new();
            CRC32_INITIAL_VALUE as _
        }
        _ => ADLER32_INITIAL_VALUE as _,
    };
    state.last_flush = -2;
    state.zng_tr_init();
    ReturnCode::Ok
}
fn lm_init(state: &mut State) {
    state.window_size = 2 * state.w_size;
    state.head.as_mut_slice().fill(0);
    lm_set_level(state, state.level);
    state.strstart = 0;
    state.block_start = 0;
    state.lookahead = 0;
    state.insert = 0;
    state.prev_length = 0;
    state.match_available = false;
    state.match_start = 0;
    state.ins_h = 0;
}
fn lm_set_level(state: &mut State, level: i8) {
    state.max_lazy_match = CONFIGURATION_TABLE[level as usize].max_lazy;
    state.good_match = CONFIGURATION_TABLE[level as usize].good_length;
    state.nice_match = CONFIGURATION_TABLE[level as usize].nice_length;
    state.max_chain_length = CONFIGURATION_TABLE[level as usize].max_chain;
    state.hash_calc_variant = HashCalcVariant::for_max_chain_length(state.max_chain_length);
    state.level = level;
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Value {
    a: u16,
    b: u16,
}
impl Value {
    pub(crate) fn freq_mut(&mut self) -> &mut u16 {
        &mut self.a
    }
    pub(crate) fn code_mut(&mut self) -> &mut u16 {
        &mut self.a
    }
    pub(crate) fn dad_mut(&mut self) -> &mut u16 {
        &mut self.b
    }
    pub(crate) fn len_mut(&mut self) -> &mut u16 {
        &mut self.b
    }
    #[inline(always)]
    pub(crate) const fn freq(self) -> u16 {
        self.a
    }
    #[inline(always)]
    pub(crate) const fn code(self) -> u16 {
        self.a
    }
    #[inline(always)]
    pub(crate) const fn dad(self) -> u16 {
        self.b
    }
    #[inline(always)]
    pub(crate) const fn len(self) -> u16 {
        self.b
    }
}
pub(crate) const LENGTH_CODES: usize = 29;
const LITERALS: usize = 256;
pub(crate) const L_CODES: usize = LITERALS + 1 + LENGTH_CODES;
pub(crate) const D_CODES: usize = 30;
const BL_CODES: usize = 19;
const HEAP_SIZE: usize = 2 * L_CODES + 1;
const MAX_BITS: usize = 15;
const MAX_BL_BITS: usize = 7;
pub(crate) const DIST_CODE_LEN: usize = 512;
struct BitWriter<'a> {
    pub(crate) pending: Pending<'a>,
    pub(crate) bit_buffer: u64,
    pub(crate) bits_valid: u8,
    pub(crate) bits_used: u8,
}
#[inline]
const fn encode_len(ltree: &[Value], lc: u8) -> (u64, usize) {
    let mut lc = lc as usize;
    let code = self::trees_tbl::LENGTH_CODE[lc] as usize;
    let c = code + LITERALS + 1;
    assert!(c < L_CODES, "bad l_code");
    let lnode = ltree[c];
    let mut match_bits: u64 = lnode.code() as u64;
    let mut match_bits_len = lnode.len() as usize;
    let extra = StaticTreeDesc::EXTRA_LBITS[code] as usize;
    if extra != 0 {
        lc -= self::trees_tbl::BASE_LENGTH[code] as usize;
        match_bits |= (lc as u64) << match_bits_len;
        match_bits_len += extra;
    }
    (match_bits, match_bits_len)
}
#[inline]
const fn encode_dist(dtree: &[Value], mut dist: u16) -> (u64, usize) {
    dist -= 1;
    let code = State::d_code(dist as usize) as usize;
    assert!(code < D_CODES, "bad d_code");
    let dnode = dtree[code];
    let mut match_bits = dnode.code() as u64;
    let mut match_bits_len = dnode.len() as usize;
    let extra = StaticTreeDesc::EXTRA_DBITS[code] as usize;
    if extra != 0 {
        dist -= self::trees_tbl::BASE_DIST[code];
        match_bits |= (dist as u64) << match_bits_len;
        match_bits_len += extra;
    }
    (match_bits, match_bits_len)
}
impl<'a> BitWriter<'a> {
    pub(crate) const BIT_BUF_SIZE: u8 = 64;
    fn from_pending(pending: Pending<'a>) -> Self {
        Self {
            pending,
            bit_buffer: 0,
            bits_valid: 0,
            bits_used: 0,
        }
    }
    fn flush_bits(&mut self) {
        debug_assert!(self.bits_valid <= 64);
        let removed = self.bits_valid.saturating_sub(7).next_multiple_of(8);
        let keep_bytes = self.bits_valid / 8;
        let src = &self.bit_buffer.to_le_bytes();
        self.pending.extend(&src[..keep_bytes as usize]);
        self.bits_valid -= removed;
        self.bit_buffer = self.bit_buffer.checked_shr(removed as u32).unwrap_or(0);
    }
    fn emit_align(&mut self) {
        debug_assert!(self.bits_valid <= 64);
        let keep_bytes = self.bits_valid.div_ceil(8);
        let src = &self.bit_buffer.to_le_bytes();
        self.pending.extend(&src[..keep_bytes as usize]);
        self.bits_used = match self.bits_valid {
            0 => 8,
            _ => ((self.bits_valid - 1) & 7) + 1,
        };
        self.bit_buffer = 0;
        self.bits_valid = 0;
        self.sent_bits_align();
    }
    fn send_bits_trace(&self, _value: u64, _len: u8) {
        trace!(" l {:>2} v {:>4x} ", _len, _value);
    }
    fn cmpr_bits_add(&mut self, _len: usize) {}
    fn cmpr_bits_align(&mut self) {}
    fn sent_bits_add(&mut self, _len: usize) {}
    fn sent_bits_align(&mut self) {}
    #[inline(always)]
    fn send_bits(&mut self, val: u64, len: u8) {
        debug_assert!(len <= 64);
        debug_assert!(self.bits_valid <= 64);
        let total_bits = len + self.bits_valid;
        self.send_bits_trace(val, len);
        self.sent_bits_add(len as usize);
        if total_bits < Self::BIT_BUF_SIZE {
            self.bit_buffer |= val << self.bits_valid;
            self.bits_valid = total_bits;
        } else {
            self.send_bits_overflow(val, total_bits);
        }
    }
    fn send_bits_overflow(&mut self, val: u64, total_bits: u8) {
        if self.bits_valid == Self::BIT_BUF_SIZE {
            self.pending.extend(&self.bit_buffer.to_le_bytes());
            self.bit_buffer = val;
            self.bits_valid = total_bits - Self::BIT_BUF_SIZE;
        } else {
            self.bit_buffer |= val << self.bits_valid;
            self.pending.extend(&self.bit_buffer.to_le_bytes());
            self.bit_buffer = val >> (Self::BIT_BUF_SIZE - self.bits_valid);
            self.bits_valid = total_bits - Self::BIT_BUF_SIZE;
        }
    }
    fn send_code(&mut self, code: usize, tree: &[Value]) {
        let node = tree[code];
        self.send_bits(node.code() as u64, node.len() as u8)
    }
    pub fn align(&mut self) {
        self.emit_tree(BlockType::StaticTrees, false);
        self.emit_end_block(&STATIC_LTREE, false);
        self.flush_bits();
    }
    pub(crate) fn emit_tree(&mut self, block_type: BlockType, is_last_block: bool) {
        let header_bits = ((block_type as u64) << 1) | (is_last_block as u64);
        self.send_bits(header_bits, 3);
        trace!("\n--- Emit Tree: Last: {}\n", is_last_block as u8);
    }
    pub(crate) fn emit_end_block_and_align(&mut self, ltree: &[Value], is_last_block: bool) {
        self.emit_end_block(ltree, is_last_block);
        if is_last_block {
            self.emit_align();
        }
    }
    fn emit_end_block(&mut self, ltree: &[Value], _is_last_block: bool) {
        const END_BLOCK: usize = 256;
        self.send_code(END_BLOCK, ltree);
        trace!(
            "\n+++ Emit End Block: Last: {} Pending: {} Total Out: {}\n",
            _is_last_block as u8,
            self.pending.pending().len(),
            "<unknown>"
        );
    }
    pub(crate) fn emit_lit(&mut self, ltree: &[Value], c: u8) -> u16 {
        self.send_code(c as usize, ltree);
        ltree[c as usize].len()
    }
    pub(crate) fn emit_dist(
        &mut self,
        ltree: &[Value],
        dtree: &[Value],
        lc: u8,
        dist: u16,
    ) -> usize {
        let (mut match_bits, mut match_bits_len) = encode_len(ltree, lc);
        let (dist_match_bits, dist_match_bits_len) = encode_dist(dtree, dist);
        match_bits |= dist_match_bits << match_bits_len;
        match_bits_len += dist_match_bits_len;
        self.send_bits(match_bits, match_bits_len as u8);
        match_bits_len
    }
    pub(crate) fn emit_dist_static(&mut self, lc: u8, dist: u16) -> usize {
        let precomputed_len = trees_tbl::STATIC_LTREE_ENCODINGS[lc as usize];
        let mut match_bits = precomputed_len.code() as u64;
        let mut match_bits_len = precomputed_len.len() as usize;
        let dtree = self::trees_tbl::STATIC_DTREE.as_slice();
        let (dist_match_bits, dist_match_bits_len) = encode_dist(dtree, dist);
        match_bits |= dist_match_bits << match_bits_len;
        match_bits_len += dist_match_bits_len;
        self.send_bits(match_bits, match_bits_len as u8);
        match_bits_len
    }
    fn compress_block_help(&mut self, sym_buf: &SymBuf, ltree: &[Value], dtree: &[Value]) {
        for (dist, lc) in sym_buf.iter() {
            match dist {
                0 => self.emit_lit(ltree, lc) as usize,
                _ => self.emit_dist(ltree, dtree, lc, dist),
            };
        }
        self.emit_end_block(ltree, false)
    }
    fn send_tree(&mut self, tree: &[Value], bl_tree: &[Value], max_code: usize) {
        let mut prevlen: isize = -1;
        let mut curlen;
        let mut nextlen = tree[0].len();
        let mut count = 0;
        let mut max_count = 7;
        let mut min_count = 4;
        if nextlen == 0 {
            max_count = 138;
            min_count = 3;
        }
        for n in 0..=max_code {
            curlen = nextlen;
            nextlen = tree[n + 1].len();
            count += 1;
            if count < max_count && curlen == nextlen {
                continue;
            } else if count < min_count {
                loop {
                    self.send_code(curlen as usize, bl_tree);
                    count -= 1;
                    if count == 0 {
                        break;
                    }
                }
            } else if curlen != 0 {
                if curlen as isize != prevlen {
                    self.send_code(curlen as usize, bl_tree);
                    count -= 1;
                }
                assert!((3..=6).contains(&count), " 3_6?");
                self.send_code(REP_3_6, bl_tree);
                self.send_bits(count - 3, 2);
            } else if count <= 10 {
                self.send_code(REPZ_3_10, bl_tree);
                self.send_bits(count - 3, 3);
            } else {
                self.send_code(REPZ_11_138, bl_tree);
                self.send_bits(count - 11, 7);
            }
            count = 0;
            prevlen = curlen as isize;
            if nextlen == 0 {
                max_count = 138;
                min_count = 3;
            } else if curlen == nextlen {
                max_count = 6;
                min_count = 3;
            } else {
                max_count = 7;
                min_count = 4;
            }
        }
    }
}
#[repr(C, align(64))]
pub(crate) struct State<'a> {
    status: Status,
    last_flush: i8,
    pub(crate) wrap: i8,
    pub(crate) strategy: Strategy,
    pub(crate) level: i8,
    pub(crate) block_open: u8,
    pub(crate) hash_calc_variant: HashCalcVariant,
    pub(crate) match_available: bool,
    pub(crate) good_match: u16,
    pub(crate) nice_match: u16,
    pub(crate) match_start: Pos,
    pub(crate) prev_match: Pos,
    pub(crate) strstart: usize,
    pub(crate) window: Window<'a>,
    pub(crate) w_size: usize,
    pub(crate) lookahead: usize,
    _cache_line_0: (),
    pub(crate) prev: WeakSliceMut<'a, u16>,
    pub(crate) head: WeakArrayMut<'a, u16, HASH_SIZE>,
    pub(crate) prev_length: u16,
    pub(crate) max_chain_length: u16,
    pub(crate) max_lazy_match: u16,
    pub(crate) matches: u8,
    pub(crate) block_start: isize,
    pub(crate) sym_buf: SymBuf<'a>,
    _cache_line_1: (),
    lit_bufsize: usize,
    pub(crate) window_size: usize,
    bit_writer: BitWriter<'a>,
    _cache_line_2: (),
    opt_len: usize,
    static_len: usize,
    pub(crate) insert: usize,
    pub(crate) ins_h: u32,
    gzhead: Option<&'a mut GzHeader>,
    gzindex: usize,
    _padding_0: [u8; 16],
    _cache_line_3: (),
    crc_fold: crate::compression::backend::crc32::Crc32Fold,
    allocation_start: NonNull<u8>,
    total_allocation_size: usize,
    l_desc: TreeDesc<HEAP_SIZE>,
    d_desc: TreeDesc<{ 2 * D_CODES + 1 }>,
    bl_desc: TreeDesc<{ 2 * BL_CODES + 1 }>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Strategy {
    #[default]
    Default = 0,
    Filtered = 1,
    HuffmanOnly = 2,
    Rle = 3,
    Fixed = 4,
}
impl TryFrom<i32> for Strategy {
    type Error = ();
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Strategy::Default),
            1 => Ok(Strategy::Filtered),
            2 => Ok(Strategy::HuffmanOnly),
            3 => Ok(Strategy::Rle),
            4 => Ok(Strategy::Fixed),
            _ => Err(()),
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
enum DataType {
    Binary = 0,
    Text = 1,
    Unknown = 2,
}
impl<'a> State<'a> {
    pub const BIT_BUF_SIZE: u8 = BitWriter::BIT_BUF_SIZE;
    pub(crate) fn w_bits(&self) -> u32 {
        self.w_size.trailing_zeros()
    }
    pub(crate) fn w_mask(&self) -> usize {
        self.w_size - 1
    }
    pub(crate) fn max_dist(&self) -> usize {
        self.w_size - MIN_LOOKAHEAD
    }
    pub(crate) fn max_insert_length(&self) -> usize {
        self.max_lazy_match as usize
    }
    pub(crate) fn pending_buf_size(&self) -> usize {
        self.lit_bufsize * 4
    }
    #[inline(always)]
    pub(crate) fn update_hash(&self, h: u32, val: u32) -> u32 {
        match self.hash_calc_variant {
            HashCalcVariant::Standard => StandardHashCalc::update_hash(h, val),
            HashCalcVariant::Roll => RollHashCalc::update_hash(h, val),
        }
    }
    #[inline(always)]
    pub(crate) fn quick_insert_string(&mut self, string: usize) -> u16 {
        match self.hash_calc_variant {
            HashCalcVariant::Standard => StandardHashCalc::quick_insert_string(self, string),
            HashCalcVariant::Roll => RollHashCalc::quick_insert_string(self, string),
        }
    }
    #[inline(always)]
    pub(crate) fn insert_string(&mut self, string: usize, count: usize) {
        match self.hash_calc_variant {
            HashCalcVariant::Standard => StandardHashCalc::insert_string(self, string, count),
            HashCalcVariant::Roll => RollHashCalc::insert_string(self, string, count),
        }
    }
    #[inline(always)]
    pub(crate) fn tally_lit(&mut self, unmatched: u8) -> bool {
        Self::tally_lit_help(&mut self.sym_buf, &mut self.l_desc, unmatched)
    }
    pub(crate) fn tally_lit_help(
        sym_buf: &mut SymBuf,
        l_desc: &mut TreeDesc<HEAP_SIZE>,
        unmatched: u8,
    ) -> bool {
        const _VERIFY: () = {
            assert!(
                u8::MAX as usize <= STD_MAX_MATCH - STD_MIN_MATCH,
                "tally_lit: bad literal"
            );
        };
        sym_buf.push_lit(unmatched);
        *l_desc.dyn_tree[unmatched as usize].freq_mut() += 1;
        sym_buf.should_flush_block()
    }
    const fn d_code(dist: usize) -> u8 {
        const _VERIFY: () = {
            let mut i = 0;
            while i < trees_tbl::DIST_CODE.len() {
                assert!(trees_tbl::DIST_CODE[i] < D_CODES as u8);
                i += 1;
            }
        };
        let index = if dist < 256 { dist } else { 256 + (dist >> 7) };
        self::trees_tbl::DIST_CODE[index]
    }
    #[inline(always)]
    pub(crate) fn tally_dist(&mut self, mut dist: usize, len: usize) -> bool {
        self.sym_buf.push_dist(dist as u16, len as u8);
        self.matches = self.matches.saturating_add(1);
        dist -= 1;
        assert!(dist < self.max_dist(), "tally_dist: bad match");
        let index = self::trees_tbl::LENGTH_CODE[len] as usize + LITERALS + 1;
        *self.l_desc.dyn_tree[index].freq_mut() += 1;
        *self.d_desc.dyn_tree[Self::d_code(dist) as usize].freq_mut() += 1;
        self.sym_buf.should_flush_block()
    }
    fn detect_data_type(dyn_tree: &[Value]) -> DataType {
        const NON_TEXT: u64 = 0xf3ffc07f;
        let mut mask = NON_TEXT;
        for value in &dyn_tree[0..32] {
            if (mask & 1) != 0 && value.freq() != 0 {
                return DataType::Binary;
            }
            mask >>= 1;
        }
        if dyn_tree[9].freq() != 0 || dyn_tree[10].freq() != 0 || dyn_tree[13].freq() != 0 {
            return DataType::Text;
        }
        if dyn_tree[32..LITERALS].iter().any(|v| v.freq() != 0) {
            return DataType::Text;
        }
        DataType::Binary
    }
    fn compress_block_static_trees(&mut self) {
        let ltree = self::trees_tbl::STATIC_LTREE.as_slice();
        for (dist, lc) in self.sym_buf.iter() {
            match dist {
                0 => self.bit_writer.emit_lit(ltree, lc) as usize,
                _ => self.bit_writer.emit_dist_static(lc, dist),
            };
        }
        self.bit_writer.emit_end_block(ltree, false)
    }
    fn compress_block_dynamic_trees(&mut self) {
        self.bit_writer.compress_block_help(
            &self.sym_buf,
            &self.l_desc.dyn_tree,
            &self.d_desc.dyn_tree,
        );
    }
    fn header(&self) -> u16 {
        const PRESET_DICT: u16 = 0x20;
        const Z_DEFLATED: u16 = 8;
        let dict = match self.strstart {
            0 => 0,
            _ => PRESET_DICT,
        };
        let h = ((Z_DEFLATED + ((self.w_bits() as u16 - 8) << 4)) << 8)
            | (self.level_flags() << 6)
            | dict;
        h + 31 - (h % 31)
    }
    fn level_flags(&self) -> u16 {
        if self.strategy >= Strategy::HuffmanOnly || self.level < 2 {
            0
        } else if self.level < 6 {
            1
        } else if self.level == 6 {
            2
        } else {
            3
        }
    }
    fn zng_tr_init(&mut self) {
        self.l_desc.stat_desc = &StaticTreeDesc::L;
        self.d_desc.stat_desc = &StaticTreeDesc::D;
        self.bl_desc.stat_desc = &StaticTreeDesc::BL;
        self.bit_writer.bit_buffer = 0;
        self.bit_writer.bits_valid = 0;
        self.bit_writer.bits_used = 0;
        self.init_block();
    }
    fn init_block(&mut self) {
        for value in &mut self.l_desc.dyn_tree[..L_CODES] {
            *value.freq_mut() = 0;
        }
        for value in &mut self.d_desc.dyn_tree[..D_CODES] {
            *value.freq_mut() = 0;
        }
        for value in &mut self.bl_desc.dyn_tree[..BL_CODES] {
            *value.freq_mut() = 0;
        }
        const END_BLOCK: usize = 256;
        *self.l_desc.dyn_tree[END_BLOCK].freq_mut() = 1;
        self.opt_len = 0;
        self.static_len = 0;
        self.sym_buf.clear();
        self.matches = 0;
    }
}
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Init = 1,
    GZip = 4,
    Extra = 5,
    Name = 6,
    Comment = 7,
    Hcrc = 8,
    Busy = 2,
    Finish = 3,
}
const fn rank_flush(f: i8) -> i8 {
    ((f) * 2) - (if (f) > 4 { 9 } else { 0 })
}
#[derive(Debug)]
pub(crate) enum BlockState {
    NeedMore = 0,
    BlockDone = 1,
    FinishStarted = 2,
    FinishDone = 3,
}
pub(crate) const MAX_STORED: usize = 65535;
pub(crate) fn read_buf_window(stream: &mut DeflateStream, offset: usize, size: usize) -> usize {
    let len = Ord::min(stream.avail_in as usize, size);
    if len == 0 {
        return 0;
    }
    stream.avail_in -= len as u32;
    if stream.state.wrap == 2 {
        let window = &mut stream.state.window;
        unsafe { window.copy_and_initialize(offset..offset + len, stream.next_in) };
        let data = &stream.state.window.filled()[offset..][..len];
        stream.state.crc_fold.fold(data, CRC32_INITIAL_VALUE);
    } else if stream.state.wrap == 1 {
        let window = &mut stream.state.window;
        unsafe { window.copy_and_initialize(offset..offset + len, stream.next_in) };
        let data = &stream.state.window.filled()[offset..][..len];
        stream.adler = adler32(stream.adler as u32, data) as _;
    } else {
        let window = &mut stream.state.window;
        unsafe { window.copy_and_initialize(offset..offset + len, stream.next_in) };
    }
    stream.next_in = stream.next_in.wrapping_add(len);
    stream.total_in = stream
        .total_in
        .wrapping_add(len as crate::compression::backend::c_api::ZSize);
    len
}
pub(crate) enum BlockType {
    StoredBlock = 0,
    StaticTrees = 1,
    DynamicTrees = 2,
}
pub(crate) fn zng_tr_stored_block(
    state: &mut State,
    window_range: core::ops::Range<usize>,
    is_last: bool,
) {
    state.bit_writer.emit_tree(BlockType::StoredBlock, is_last);
    state.bit_writer.emit_align();
    state.bit_writer.cmpr_bits_align();
    let input_block: &[u8] = &state.window.filled()[window_range];
    let stored_len = input_block.len() as u16;
    state.bit_writer.pending.extend(&stored_len.to_le_bytes());
    state
        .bit_writer
        .pending
        .extend(&(!stored_len).to_le_bytes());
    state.bit_writer.cmpr_bits_add(32);
    state.bit_writer.sent_bits_add(32);
    if stored_len > 0 {
        state.bit_writer.pending.extend(input_block);
        state.bit_writer.cmpr_bits_add((stored_len << 3) as usize);
        state.bit_writer.sent_bits_add((stored_len << 3) as usize);
    }
}
pub(crate) const STD_MIN_MATCH: usize = 3;
pub(crate) const STD_MAX_MATCH: usize = 258;
pub(crate) const WANT_MIN_MATCH: usize = 4;
pub(crate) const MIN_LOOKAHEAD: usize = STD_MAX_MATCH + STD_MIN_MATCH + 1;
#[inline]
pub(crate) fn fill_window(stream: &mut DeflateStream) {
    debug_assert!(stream.state.lookahead < MIN_LOOKAHEAD);
    let wsize = stream.state.w_size;
    loop {
        let state = &mut *stream.state;
        let mut more = state.window_size - state.lookahead - state.strstart;
        if state.strstart >= wsize + state.max_dist() {
            let (old, new) = state.window.filled_mut()[..2 * wsize].split_at_mut(wsize);
            old.copy_from_slice(new);
            if state.match_start >= wsize as u16 {
                state.match_start -= wsize as u16;
            } else {
                state.match_start = 0;
                state.prev_length = 0;
            }
            state.strstart -= wsize;
            state.block_start = state.block_start.wrapping_sub_unsigned(wsize);
            state.insert = Ord::min(state.insert, state.strstart);
            self::slide_hash::slide_hash(state);
            more += wsize;
        }
        if stream.avail_in == 0 {
            break;
        }
        assert!(more >= 2, "more < 2");
        let n = read_buf_window(stream, stream.state.strstart + stream.state.lookahead, more);
        let state = &mut *stream.state;
        state.lookahead += n;
        if state.lookahead + state.insert >= STD_MIN_MATCH {
            let string = state.strstart - state.insert;
            if state.max_chain_length > 1024 {
                let v0 = state.window.filled()[string] as u32;
                let v1 = state.window.filled()[string + 1] as u32;
                state.ins_h = state.update_hash(v0, v1);
            } else if string >= 1 {
                state.quick_insert_string(string + 2 - STD_MIN_MATCH);
            }
            let mut count = state.insert;
            if state.lookahead == 1 {
                count -= 1;
            }
            if count > 0 {
                state.insert_string(string, count);
                state.insert -= count;
            }
        }
        if !(stream.state.lookahead < MIN_LOOKAHEAD && stream.avail_in != 0) {
            break;
        }
    }
    assert!(
        stream.state.strstart <= stream.state.window_size - MIN_LOOKAHEAD,
        "not enough room for search"
    );
}
pub(crate) struct StaticTreeDesc {
    pub(crate) static_tree: &'static [Value],
    extra_bits: &'static [u8],
    extra_base: usize,
    elems: usize,
    max_length: u16,
}
impl StaticTreeDesc {
    const EMPTY: Self = Self {
        static_tree: &[],
        extra_bits: &[],
        extra_base: 0,
        elems: 0,
        max_length: 0,
    };
    const EXTRA_LBITS: [u8; LENGTH_CODES] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    const EXTRA_DBITS: [u8; D_CODES] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];
    const EXTRA_BLBITS: [u8; BL_CODES] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 7];
    const BL_ORDER: [u8; BL_CODES] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    pub(crate) const L: Self = Self {
        static_tree: &self::trees_tbl::STATIC_LTREE,
        extra_bits: &Self::EXTRA_LBITS,
        extra_base: LITERALS + 1,
        elems: L_CODES,
        max_length: MAX_BITS as u16,
    };
    pub(crate) const D: Self = Self {
        static_tree: &self::trees_tbl::STATIC_DTREE,
        extra_bits: &Self::EXTRA_DBITS,
        extra_base: 0,
        elems: D_CODES,
        max_length: MAX_BITS as u16,
    };
    pub(crate) const BL: Self = Self {
        static_tree: &[],
        extra_bits: &Self::EXTRA_BLBITS,
        extra_base: 0,
        elems: BL_CODES,
        max_length: MAX_BL_BITS as u16,
    };
}
#[derive(Clone)]
pub(crate) struct TreeDesc<const N: usize> {
    dyn_tree: [Value; N],
    max_code: usize,
    stat_desc: &'static StaticTreeDesc,
}
impl<const N: usize> TreeDesc<N> {
    const EMPTY: Self = Self {
        dyn_tree: [Value::new(0, 0); N],
        max_code: 0,
        stat_desc: &StaticTreeDesc::EMPTY,
    };
}
fn build_tree<const N: usize>(state: &mut State, desc: &mut TreeDesc<N>) {
    let tree = &mut desc.dyn_tree;
    let stree = desc.stat_desc.static_tree;
    let elements = desc.stat_desc.elems;
    let mut heap = Heap::new();
    let mut max_code = heap.initialize(&mut tree[..elements]);
    while heap.heap_len < 2 {
        heap.heap_len += 1;
        let node = if max_code < 2 {
            max_code += 1;
            max_code
        } else {
            0
        };
        debug_assert!(node >= 0);
        let node = node as usize;
        heap.heap[heap.heap_len] = node as u32;
        *tree[node].freq_mut() = 1;
        heap.depth[node] = 0;
        state.opt_len -= 1;
        if !stree.is_empty() {
            state.static_len -= stree[node].len() as usize;
        }
    }
    debug_assert!(max_code >= 0);
    let max_code = max_code as usize;
    desc.max_code = max_code;
    let mut n = heap.heap_len / 2;
    while n >= 1 {
        heap.pqdownheap(tree, n);
        n -= 1;
    }
    heap.construct_huffman_tree(tree, elements);
    let bl_count = gen_bitlen(state, &mut heap, desc);
    gen_codes(&mut desc.dyn_tree, max_code, &bl_count);
}
fn gen_bitlen<const N: usize>(
    state: &mut State,
    heap: &mut Heap,
    desc: &mut TreeDesc<N>,
) -> [u16; MAX_BITS + 1] {
    let tree = &mut desc.dyn_tree;
    let max_code = desc.max_code;
    let stree = desc.stat_desc.static_tree;
    let extra = desc.stat_desc.extra_bits;
    let base = desc.stat_desc.extra_base;
    let max_length = desc.stat_desc.max_length;
    let mut bl_count = [0u16; MAX_BITS + 1];
    *tree[heap.heap[heap.heap_max] as usize].len_mut() = 0;
    let mut overflow: i32 = 0;
    for &n in &heap.heap[heap.heap_max + 1..HEAP_SIZE] {
        let n = n as usize;
        let mut bits = tree[usize::from(tree[n].dad())].len() + 1;
        if bits > max_length {
            bits = max_length;
            overflow += 1;
        }
        *tree[n].len_mut() = bits;
        if n > max_code {
            continue;
        }
        bl_count[usize::from(bits)] += 1;
        let mut xbits = 0;
        if n >= base {
            xbits = extra[n - base] as usize;
        }
        let f = tree[n].freq() as usize;
        state.opt_len += f * (bits as usize + xbits);
        if !stree.is_empty() {
            state.static_len += f * (stree[n].len() as usize + xbits);
        }
    }
    if overflow == 0 {
        return bl_count;
    }
    loop {
        let mut bits = max_length as usize - 1;
        while bl_count[bits] == 0 {
            bits -= 1;
        }
        bl_count[bits] -= 1;
        bl_count[bits + 1] += 2;
        bl_count[max_length as usize] -= 1;
        overflow -= 2;
        if overflow <= 0 {
            break;
        }
    }
    let mut h = HEAP_SIZE;
    for bits in (1..=max_length).rev() {
        let mut n = bl_count[bits as usize];
        while n != 0 {
            h -= 1;
            let m = heap.heap[h] as usize;
            if m > max_code {
                continue;
            }
            if tree[m].len() != bits {
                state.opt_len += (bits * tree[m].freq()) as usize;
                state.opt_len -= (tree[m].len() * tree[m].freq()) as usize;
                *tree[m].len_mut() = bits;
            }
            n -= 1;
        }
    }
    bl_count
}

fn gen_codes(tree: &mut [Value], max_code: usize, bl_count: &[u16]) {
    let mut next_code = [0; MAX_BITS + 1];
    let mut code = 0;
    for bits in 1..=MAX_BITS {
        code = (code + bl_count[bits - 1]) << 1;
        next_code[bits] = code;
    }
    assert!(
        code + bl_count[MAX_BITS] - 1 == (1 << MAX_BITS) - 1,
        "inconsistent bit counts"
    );
    trace!("\ngen_codes: max_code {max_code} ");
    for n in 0..=max_code {
        let len = tree[n].len();
        if len == 0 {
            continue;
        }
        assert!((1..=15).contains(&len), "code length must be 1-15");
        *tree[n].code_mut() = next_code[len as usize].reverse_bits() >> (16 - len);
        next_code[len as usize] += 1;
        if tree != self::trees_tbl::STATIC_LTREE.as_slice() {
            trace!(
                "\nn {:>3} {} l {:>2} c {:>4x} ({:x}) ",
                n,
                if isgraph(n as u8) {
                    char::from_u32(n as u32).unwrap()
                } else {
                    ' '
                },
                len,
                tree[n].code(),
                next_code[len as usize] - 1
            );
        }
    }
}
const REP_3_6: usize = 16;
const REPZ_3_10: usize = 17;
const REPZ_11_138: usize = 18;
fn scan_tree(bl_desc: &mut TreeDesc<{ 2 * BL_CODES + 1 }>, tree: &mut [Value], max_code: usize) {
    let mut prevlen = -1isize;
    let mut curlen: isize;
    let mut nextlen = tree[0].len();
    let mut count = 0;
    let mut max_count = 7;
    let mut min_count = 4;
    if nextlen == 0 {
        max_count = 138;
        min_count = 3;
    }
    *tree[max_code + 1].len_mut() = 0xffff;
    let bl_tree = &mut bl_desc.dyn_tree;
    for n in 0..=max_code {
        curlen = nextlen as isize;
        nextlen = tree[n + 1].len();
        count += 1;
        if count < max_count && curlen == nextlen as isize {
            continue;
        } else if count < min_count {
            *bl_tree[curlen as usize].freq_mut() += count;
        } else if curlen != 0 {
            if curlen != prevlen {
                *bl_tree[curlen as usize].freq_mut() += 1;
            }
            *bl_tree[REP_3_6].freq_mut() += 1;
        } else if count <= 10 {
            *bl_tree[REPZ_3_10].freq_mut() += 1;
        } else {
            *bl_tree[REPZ_11_138].freq_mut() += 1;
        }
        count = 0;
        prevlen = curlen;
        if nextlen == 0 {
            max_count = 138;
            min_count = 3;
        } else if curlen == nextlen as isize {
            max_count = 6;
            min_count = 3;
        } else {
            max_count = 7;
            min_count = 4;
        }
    }
}
fn send_all_trees(state: &mut State, lcodes: usize, dcodes: usize, blcodes: usize) {
    assert!(
        lcodes >= 257 && dcodes >= 1 && blcodes >= 4,
        "not enough codes"
    );
    assert!(
        lcodes <= L_CODES && dcodes <= D_CODES && blcodes <= BL_CODES,
        "too many codes"
    );
    trace!("\nbl counts: ");
    state.bit_writer.send_bits(lcodes as u64 - 257, 5);
    state.bit_writer.send_bits(dcodes as u64 - 1, 5);
    state.bit_writer.send_bits(blcodes as u64 - 4, 4);
    for rank in 0..blcodes {
        trace!("\nbl code {:>2} ", StaticTreeDesc::BL_ORDER[rank]);
        state.bit_writer.send_bits(
            state.bl_desc.dyn_tree[StaticTreeDesc::BL_ORDER[rank] as usize].len() as u64,
            3,
        );
    }
    trace!("\nbl tree: sent {}", state.bit_writer.bits_sent);
    state
        .bit_writer
        .send_tree(&state.l_desc.dyn_tree, &state.bl_desc.dyn_tree, lcodes - 1);
    trace!("\nlit tree: sent {}", state.bit_writer.bits_sent);
    state
        .bit_writer
        .send_tree(&state.d_desc.dyn_tree, &state.bl_desc.dyn_tree, dcodes - 1);
    trace!("\ndist tree: sent {}", state.bit_writer.bits_sent);
}
fn build_bl_tree(state: &mut State) -> usize {
    scan_tree(
        &mut state.bl_desc,
        &mut state.l_desc.dyn_tree,
        state.l_desc.max_code,
    );
    scan_tree(
        &mut state.bl_desc,
        &mut state.d_desc.dyn_tree,
        state.d_desc.max_code,
    );
    {
        let mut tmp = TreeDesc::EMPTY;
        core::mem::swap(&mut tmp, &mut state.bl_desc);
        build_tree(state, &mut tmp);
        core::mem::swap(&mut tmp, &mut state.bl_desc);
    }
    let mut max_blindex = BL_CODES - 1;
    while max_blindex >= 3 {
        let index = StaticTreeDesc::BL_ORDER[max_blindex] as usize;
        if state.bl_desc.dyn_tree[index].len() != 0 {
            break;
        }
        max_blindex -= 1;
    }
    state.opt_len += 3 * (max_blindex + 1) + 5 + 5 + 4;
    trace!(
        "\ndyn trees: dyn {}, stat {}",
        state.opt_len, state.static_len
    );
    max_blindex
}
fn zng_tr_flush_block(
    stream: &mut DeflateStream,
    window_offset: Option<usize>,
    stored_len: u32,
    last: bool,
) {
    let mut opt_lenb;
    let static_lenb;
    let mut max_blindex = 0;
    let state = &mut stream.state;
    if state.sym_buf.is_empty() {
        opt_lenb = 0;
        static_lenb = 0;
        state.static_len = 7;
    } else if state.level > 0 {
        if stream.data_type == DataType::Unknown as i32 {
            stream.data_type = State::detect_data_type(&state.l_desc.dyn_tree) as i32;
        }
        {
            let mut tmp = TreeDesc::EMPTY;
            core::mem::swap(&mut tmp, &mut state.l_desc);
            build_tree(state, &mut tmp);
            core::mem::swap(&mut tmp, &mut state.l_desc);
            trace!(
                "\nlit data: dyn {}, stat {}",
                state.opt_len, state.static_len
            );
        }
        {
            let mut tmp = TreeDesc::EMPTY;
            core::mem::swap(&mut tmp, &mut state.d_desc);
            build_tree(state, &mut tmp);
            core::mem::swap(&mut tmp, &mut state.d_desc);
            trace!(
                "\ndist data: dyn {}, stat {}",
                state.opt_len, state.static_len
            );
        }
        max_blindex = build_bl_tree(state);
        opt_lenb = (state.opt_len + 3 + 7) >> 3;
        static_lenb = (state.static_len + 3 + 7) >> 3;
        trace!(
            "\nopt {}({}) stat {}({}) stored {} lit {} ",
            opt_lenb,
            state.opt_len,
            static_lenb,
            state.static_len,
            stored_len,
            state.sym_buf.iter().count()
        );
        if static_lenb <= opt_lenb || state.strategy == Strategy::Fixed {
            opt_lenb = static_lenb;
        }
    } else {
        assert!(window_offset.is_some(), "lost buf");
        opt_lenb = stored_len as usize + 5;
        static_lenb = stored_len as usize + 5;
    }
    if stored_len as usize + 4 <= opt_lenb && window_offset.is_some() {
        let window_offset = window_offset.unwrap();
        let range = window_offset..window_offset + stored_len as usize;
        zng_tr_stored_block(state, range, last);
    } else if static_lenb == opt_lenb {
        state.bit_writer.emit_tree(BlockType::StaticTrees, last);
        state.compress_block_static_trees();
    } else {
        state.bit_writer.emit_tree(BlockType::DynamicTrees, last);
        send_all_trees(
            state,
            state.l_desc.max_code + 1,
            state.d_desc.max_code + 1,
            max_blindex + 1,
        );
        state.compress_block_dynamic_trees();
    }
    state.init_block();
    if last {
        state.bit_writer.emit_align();
    }
}
pub(crate) fn flush_block_only(stream: &mut DeflateStream, is_last: bool) {
    zng_tr_flush_block(
        stream,
        (stream.state.block_start >= 0).then_some(stream.state.block_start as usize),
        (stream.state.strstart as isize - stream.state.block_start) as u32,
        is_last,
    );
    stream.state.block_start = stream.state.strstart as isize;
    flush_pending(stream)
}
fn flush_bytes(stream: &mut DeflateStream, mut bytes: &[u8]) -> ControlFlow<ReturnCode> {
    let mut state = &mut stream.state;
    let mut beg = state.bit_writer.pending.pending().len();
    while state.bit_writer.pending.remaining() < bytes.len() {
        let copy = state.bit_writer.pending.remaining();
        state.bit_writer.pending.extend(&bytes[..copy]);
        stream.adler = crc32(
            stream.adler as u32,
            &state.bit_writer.pending.pending()[beg..],
        ) as ZChecksum;
        state.gzindex += copy;
        flush_pending(stream);
        state = &mut stream.state;
        if !state.bit_writer.pending.pending().is_empty() {
            state.last_flush = -1;
            return ControlFlow::Break(ReturnCode::Ok);
        }
        beg = 0;
        bytes = &bytes[copy..];
    }
    state.bit_writer.pending.extend(bytes);
    stream.adler = crc32(
        stream.adler as u32,
        &state.bit_writer.pending.pending()[beg..],
    ) as ZChecksum;
    state.gzindex = 0;
    ControlFlow::Continue(())
}
pub fn deflate(stream: &mut DeflateStream, flush: DeflateFlush) -> ReturnCode {
    if stream.next_out.is_null()
        || (stream.avail_in != 0 && stream.next_in.is_null())
        || (stream.state.status == Status::Finish && flush != DeflateFlush::Finish)
    {
        let err = ReturnCode::StreamError;
        stream.msg = err.error_message();
        return err;
    }
    if stream.avail_out == 0 {
        let err = ReturnCode::BufError;
        stream.msg = err.error_message();
        return err;
    }
    let old_flush = stream.state.last_flush;
    stream.state.last_flush = flush as i8;
    if !stream.state.bit_writer.pending.pending().is_empty() {
        flush_pending(stream);
        if stream.avail_out == 0 {
            stream.state.last_flush = -1;
            return ReturnCode::Ok;
        }
    } else if stream.avail_in == 0
        && rank_flush(flush as i8) <= rank_flush(old_flush)
        && flush != DeflateFlush::Finish
    {
        let err = ReturnCode::BufError;
        stream.msg = err.error_message();
        return err;
    }
    if stream.state.status == Status::Finish && stream.avail_in != 0 {
        let err = ReturnCode::BufError;
        stream.msg = err.error_message();
        return err;
    }
    if stream.state.status == Status::Init && stream.state.wrap == 0 {
        stream.state.status = Status::Busy;
    }
    if stream.state.status == Status::Init {
        let header = stream.state.header();
        stream
            .state
            .bit_writer
            .pending
            .extend(&header.to_be_bytes());
        if stream.state.strstart != 0 {
            let adler = stream.adler as u32;
            stream.state.bit_writer.pending.extend(&adler.to_be_bytes());
        }
        stream.adler = ADLER32_INITIAL_VALUE as _;
        stream.state.status = Status::Busy;
        flush_pending(stream);
        if !stream.state.bit_writer.pending.pending().is_empty() {
            stream.state.last_flush = -1;
            return ReturnCode::Ok;
        }
    }
    if stream.state.status == Status::GZip {
        stream.state.crc_fold = Crc32Fold::new();
        stream.state.bit_writer.pending.extend(&[31, 139, 8]);
        let extra_flags = if stream.state.level == 9 {
            2
        } else if stream.state.strategy >= Strategy::HuffmanOnly || stream.state.level < 2 {
            4
        } else {
            0
        };
        match &stream.state.gzhead {
            None => {
                let bytes = [0, 0, 0, 0, 0, extra_flags, GzHeader::OS_CODE];
                stream.state.bit_writer.pending.extend(&bytes);
                stream.state.status = Status::Busy;
                flush_pending(stream);
                if !stream.state.bit_writer.pending.pending().is_empty() {
                    stream.state.last_flush = -1;
                    return ReturnCode::Ok;
                }
            }
            Some(gzhead) => {
                stream.state.bit_writer.pending.extend(&[gzhead.flags()]);
                let bytes = (gzhead.time as u32).to_le_bytes();
                stream.state.bit_writer.pending.extend(&bytes);
                stream
                    .state
                    .bit_writer
                    .pending
                    .extend(&[extra_flags, gzhead.os as u8]);
                if !gzhead.extra.is_null() {
                    let bytes = (gzhead.extra_len as u16).to_le_bytes();
                    stream.state.bit_writer.pending.extend(&bytes);
                }
                if gzhead.hcrc != 0 {
                    stream.adler = crc32(
                        stream.adler as u32,
                        stream.state.bit_writer.pending.pending(),
                    ) as ZChecksum
                }
                stream.state.gzindex = 0;
                stream.state.status = Status::Extra;
            }
        }
    }
    if stream.state.status == Status::Extra {
        if let Some(gzhead) = stream.state.gzhead.as_ref() {
            if !gzhead.extra.is_null() {
                let gzhead_extra = gzhead.extra;
                let extra = unsafe {
                    core::slice::from_raw_parts(
                        gzhead_extra.add(stream.state.gzindex),
                        (gzhead.extra_len & 0xffff) as usize - stream.state.gzindex,
                    )
                };
                if let ControlFlow::Break(err) = flush_bytes(stream, extra) {
                    return err;
                }
            }
        }
        stream.state.status = Status::Name;
    }
    if stream.state.status == Status::Name {
        if let Some(gzhead) = stream.state.gzhead.as_ref() {
            if !gzhead.name.is_null() {
                let gzhead_name = unsafe { CStr::from_ptr(gzhead.name.cast()) };
                let bytes = gzhead_name.to_bytes_with_nul();
                if let ControlFlow::Break(err) = flush_bytes(stream, bytes) {
                    return err;
                }
            }
            stream.state.status = Status::Comment;
        }
    }
    if stream.state.status == Status::Comment {
        if let Some(gzhead) = stream.state.gzhead.as_ref() {
            if !gzhead.comment.is_null() {
                let gzhead_comment = unsafe { CStr::from_ptr(gzhead.comment.cast()) };
                let bytes = gzhead_comment.to_bytes_with_nul();
                if let ControlFlow::Break(err) = flush_bytes(stream, bytes) {
                    return err;
                }
            }
            stream.state.status = Status::Hcrc;
        }
    }
    if stream.state.status == Status::Hcrc {
        if let Some(gzhead) = stream.state.gzhead.as_ref() {
            if gzhead.hcrc != 0 {
                let bytes = (stream.adler as u16).to_le_bytes();
                if let ControlFlow::Break(err) = flush_bytes(stream, &bytes) {
                    return err;
                }
            }
        }
        stream.state.status = Status::Busy;
        flush_pending(stream);
        if !stream.state.bit_writer.pending.pending().is_empty() {
            stream.state.last_flush = -1;
            return ReturnCode::Ok;
        }
    }
    let state = &mut stream.state;
    if stream.avail_in != 0
        || state.lookahead != 0
        || (flush != DeflateFlush::NoFlush && state.status != Status::Finish)
    {
        let bstate = self::algorithm::run(stream, flush);
        let state = &mut stream.state;
        if matches!(bstate, BlockState::FinishStarted | BlockState::FinishDone) {
            state.status = Status::Finish;
        }
        match bstate {
            BlockState::NeedMore | BlockState::FinishStarted => {
                if stream.avail_out == 0 {
                    state.last_flush = -1;
                }
                return ReturnCode::Ok;
            }
            BlockState::BlockDone => {
                match flush {
                    DeflateFlush::NoFlush => unreachable!("condition of inner surrounding if"),
                    DeflateFlush::PartialFlush => {
                        state.bit_writer.align();
                    }
                    DeflateFlush::SyncFlush => {
                        zng_tr_stored_block(state, 0..0, false);
                    }
                    DeflateFlush::FullFlush => {
                        zng_tr_stored_block(state, 0..0, false);
                        state.head.as_mut_slice().fill(0);
                        if state.lookahead == 0 {
                            state.strstart = 0;
                            state.block_start = 0;
                            state.insert = 0;
                        }
                    }
                    DeflateFlush::Block => {}
                    DeflateFlush::Finish => unreachable!("condition of outer surrounding if"),
                }
                flush_pending(stream);
                if stream.avail_out == 0 {
                    stream.state.last_flush = -1;
                    return ReturnCode::Ok;
                }
            }
            BlockState::FinishDone => {}
        }
    }
    if flush != DeflateFlush::Finish {
        return ReturnCode::Ok;
    }
    if stream.state.wrap == 2 {
        let crc_fold = core::mem::take(&mut stream.state.crc_fold);
        stream.adler = crc_fold.finish() as ZChecksum;
        let adler = stream.adler as u32;
        stream.state.bit_writer.pending.extend(&adler.to_le_bytes());
        let total_in = stream.total_in as u32;
        stream
            .state
            .bit_writer
            .pending
            .extend(&total_in.to_le_bytes());
    } else if stream.state.wrap == 1 {
        let adler = stream.adler as u32;
        stream.state.bit_writer.pending.extend(&adler.to_be_bytes());
    }
    flush_pending(stream);
    if stream.state.wrap > 0 {
        stream.state.wrap = -stream.state.wrap;
    }
    if stream.state.bit_writer.pending.pending().is_empty() {
        assert_eq!(stream.state.bit_writer.bits_valid, 0, "bi_buf not flushed");
        return ReturnCode::StreamEnd;
    }
    ReturnCode::Ok
}
pub(crate) fn flush_pending(stream: &mut DeflateStream) {
    let state = &mut stream.state;
    state.bit_writer.flush_bits();
    let pending = state.bit_writer.pending.pending();
    let len = Ord::min(pending.len(), stream.avail_out as usize);
    if len == 0 {
        return;
    }
    trace!("\n[FLUSH {len} bytes]");
    unsafe { core::ptr::copy_nonoverlapping(pending.as_ptr(), stream.next_out, len) };
    stream.next_out = stream.next_out.wrapping_add(len);
    stream.total_out += len as crate::compression::backend::c_api::ZSize;
    stream.avail_out -= len as crate::compression::backend::c_api::Uint;
    state.bit_writer.pending.advance(len);
}

#[derive(Clone)]
struct Heap {
    heap: [u32; 2 * L_CODES + 1],
    heap_len: usize,
    heap_max: usize,
    depth: [u8; 2 * L_CODES + 1],
}
impl Heap {
    fn initialize(&mut self, tree: &mut [Value]) -> isize {
        let mut max_code = -1;
        self.heap_len = 0;
        self.heap_max = HEAP_SIZE;
        for (n, node) in tree.iter_mut().enumerate() {
            if node.freq() > 0 {
                self.heap_len += 1;
                self.heap[self.heap_len] = n as u32;
                max_code = n as isize;
                self.depth[n] = 0;
            } else {
                *node.len_mut() = 0;
            }
        }
        max_code
    }
    const SMALLEST: usize = 1;
    fn pqdownheap(&mut self, tree: &[Value], mut k: usize) {
        macro_rules! freq_and_depth {
            ($ i : expr) => {
                (tree[$i as usize].freq() as u32) << 8 | self.depth[$i as usize] as u32
            };
        }
        let v = self.heap[k];
        let v_val = freq_and_depth!(v);
        let mut j = k << 1;
        while j <= self.heap_len {
            let mut j_val = freq_and_depth!(self.heap[j]);
            if j < self.heap_len {
                let j1_val = freq_and_depth!(self.heap[j + 1]);
                if j1_val <= j_val {
                    j += 1;
                    j_val = j1_val;
                }
            }
            if v_val <= j_val {
                break;
            }
            self.heap[k] = self.heap[j];
            k = j;
            j <<= 1;
        }
        self.heap[k] = v;
    }
    fn pqremove(&mut self, tree: &[Value]) -> u32 {
        let top = self.heap[Self::SMALLEST];
        self.heap[Self::SMALLEST] = self.heap[self.heap_len];
        self.heap_len -= 1;
        self.pqdownheap(tree, Self::SMALLEST);
        top
    }
    fn construct_huffman_tree(&mut self, tree: &mut [Value], mut node: usize) {
        loop {
            let n = self.pqremove(tree) as usize;
            let m = self.heap[Heap::SMALLEST] as usize;
            self.heap_max -= 1;
            self.heap[self.heap_max] = n as u32;
            self.heap_max -= 1;
            self.heap[self.heap_max] = m as u32;
            *tree[node].freq_mut() = tree[n].freq() + tree[m].freq();
            self.depth[node] = Ord::max(self.depth[n], self.depth[m]) + 1;
            *tree[n].dad_mut() = node as u16;
            *tree[m].dad_mut() = node as u16;
            self.heap[Heap::SMALLEST] = node as u32;
            node += 1;
            self.pqdownheap(tree, Heap::SMALLEST);
            if self.heap_len < 2 {
                break;
            }
        }
        self.heap_max -= 1;
        self.heap[self.heap_max] = self.heap[Heap::SMALLEST];
    }
}

struct DeflateAllocOffsets {
    total_size: usize,
    state_pos: usize,
    window_pos: usize,
    pending_pos: usize,
    sym_buf_pos: usize,
    prev_pos: usize,
    head_pos: usize,
}
impl DeflateAllocOffsets {}

impl<'a> DeflateStream<'a> {
    pub fn new(config: DeflateConfig) -> Self {
        let mut inner = crate::compression::backend::c_api::ZStream::default();
        let ret = crate::compression::backend::deflate::init(&mut inner, config);
        assert_eq!(ret, ReturnCode::Ok);
        unsafe { core::mem::transmute(inner) }
    }
}

impl Value {
    pub(crate) const fn new(a: u16, b: u16) -> Self {
        Self { a, b }
    }
}

impl Heap {
    fn new() -> Self {
        Self {
            heap: [0; 2 * L_CODES + 1],
            heap_len: 0,
            heap_max: 0,
            depth: [0; 2 * L_CODES + 1],
        }
    }
}

impl DeflateAllocOffsets {
    fn new(window_bits: usize, lit_bufsize: usize) -> Self {
        use core::mem::size_of;
        const ALIGN_SIZE: usize = 64;
        const LIT_BUFS: usize = 4;
        let mut curr_size = 0usize;
        let state_size = size_of::<State>();
        let window_size = (1 << window_bits) * 2;
        let prev_size = (1 << window_bits) * size_of::<Pos>();
        let head_size = HASH_SIZE * size_of::<Pos>();
        let pending_size = lit_bufsize * LIT_BUFS;
        let sym_buf_size = lit_bufsize * (LIT_BUFS - 1);
        let state_pos = curr_size.next_multiple_of(ALIGN_SIZE);
        curr_size = state_pos + state_size;
        let window_pos = curr_size.next_multiple_of(ALIGN_SIZE);
        curr_size = window_pos + window_size;
        let prev_pos = curr_size.next_multiple_of(ALIGN_SIZE);
        curr_size = prev_pos + prev_size;
        let head_pos = curr_size.next_multiple_of(ALIGN_SIZE);
        curr_size = head_pos + head_size;
        let pending_pos = curr_size.next_multiple_of(ALIGN_SIZE);
        curr_size = pending_pos + pending_size;
        let sym_buf_pos = curr_size.next_multiple_of(ALIGN_SIZE);
        curr_size = sym_buf_pos + sym_buf_size;
        let total_size = (curr_size + (ALIGN_SIZE - 1)).next_multiple_of(ALIGN_SIZE);
        Self {
            total_size,
            state_pos,
            window_pos,
            pending_pos,
            sym_buf_pos,
            prev_pos,
            head_pos,
        }
    }
}
