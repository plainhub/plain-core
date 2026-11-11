use crate::checksum::adler32::adler32;
use core::ffi::{c_char, c_int};
use core::marker::PhantomData;
use core::ops::ControlFlow;
mod bitreader;
mod inffixed_tbl;
mod inftrees;
mod window;
mod writer;
pub use self::window::Window;
use self::{
    bitreader::BitReader,
    inftrees::{CodeType, InflateTable, inflate_table},
};
use crate::compression::backend::allocate::Allocator;
use crate::compression::backend::c_api::InternalState;
use crate::compression::backend::crc32::{Crc32Fold, crc32};
use crate::compression::backend::{
    Code, DEF_WBITS, InflateFlush, MAX_WBITS, MIN_WBITS, ReturnCode,
    c_api::{GzHeader, Z_DEFLATED, ZChecksum, ZSize, ZStream},
    inflate::writer::Writer,
    traceln,
};
use crate::cpu_features::CpuFeatures;
const INFLATE_STRICT: bool = false;
#[repr(C)]
pub struct InflateStream<'a> {
    pub(crate) next_in: *mut crate::compression::backend::c_api::Bytef,
    pub(crate) avail_in: crate::compression::backend::c_api::Uint,
    pub(crate) total_in: crate::compression::backend::c_api::ZSize,
    pub(crate) next_out: *mut crate::compression::backend::c_api::Bytef,
    pub(crate) avail_out: crate::compression::backend::c_api::Uint,
    pub(crate) total_out: crate::compression::backend::c_api::ZSize,
    pub(crate) msg: *mut c_char,
    pub(crate) state: &'a mut State<'a>,
    pub(crate) alloc: Allocator<'a>,
    pub(crate) data_type: c_int,
    pub(crate) adler: crate::compression::backend::c_api::ZChecksum,
    pub(crate) reserved: crate::compression::backend::c_api::Ulong,
}
unsafe impl Sync for InflateStream<'_> {}
unsafe impl Send for InflateStream<'_> {}
impl<'a> InflateStream<'a> {
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
            unsafe { strm.cast::<InflateStream>().as_mut() }
        }
    }
    fn as_z_stream_mut(&mut self) -> &mut ZStream {
        unsafe { &mut *(self as *mut _ as *mut ZStream) }
    }
    pub fn new(config: InflateConfig) -> Self {
        let mut inner = crate::compression::backend::c_api::ZStream::default();
        let ret = crate::compression::backend::inflate::init(&mut inner, config);
        assert_eq!(ret, ReturnCode::Ok);
        unsafe { core::mem::transmute(inner) }
    }
}
const MAX_BITS: u8 = 15;
const MAX_DIST_EXTRA_BITS: u8 = 13;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Mode {
    Head,
    Flags,
    Time,
    Os,
    ExLen,
    Extra,
    Name,
    Comment,
    HCrc,
    Sync,
    Mem,
    Length,
    Type,
    TypeDo,
    Stored,
    CopyBlock,
    Check,
    Len_,
    Len,
    Lit,
    LenExt,
    Dist,
    DistExt,
    Match,
    Table,
    LenLens,
    CodeLens,
    DictId,
    Dict,
    Done,
    Bad,
}
#[derive(Default, Clone, Copy)]
enum Codes {
    #[default]
    Fixed,
    Codes,
    Len,
    Dist,
}
#[derive(Default, Clone, Copy)]
struct Table {
    codes: Codes,
    bits: usize,
}
#[derive(Clone, Copy)]
struct Flags(u8);
impl Default for Flags {
    fn default() -> Self {
        Self::SANE
    }
}
impl Flags {
    const IS_LAST_BLOCK: Self = Self(0b0000_0001);
    const HAVE_DICT: Self = Self(0b0000_0010);
    const SANE: Self = Self(0b0000_0100);
    pub(crate) const fn contains(self, other: Self) -> bool {
        debug_assert!(other.0.count_ones() == 1);
        self.0 & other.0 != 0
    }
    #[inline(always)]
    pub(crate) fn update(&mut self, other: Self, value: bool) {
        if value {
            *self = Self(self.0 | other.0);
        } else {
            *self = Self(self.0 & !other.0);
        }
    }
}
#[repr(C, align(64))]
pub(crate) struct State<'a> {
    mode: Mode,
    flags: Flags,
    wbits: u8,
    wrap: u8,
    flush: InflateFlush,
    window: Window<'a>,
    ncode: usize,
    nlen: usize,
    ndist: usize,
    have: usize,
    next: usize,
    bit_reader: BitReader<'a>,
    writer: Writer<'a>,
    total: usize,
    length: usize,
    offset: usize,
    extra: usize,
    back: usize,
    was: usize,
    chunksize: usize,
    in_available: usize,
    out_available: usize,
    gzip_flags: i32,
    checksum: u32,
    crc_fold: Crc32Fold,
    error_message: Option<&'static str>,
    head: Option<&'a mut GzHeader>,
    dmax: usize,
    len_table: Table,
    dist_table: Table,
    codes_codes: [Code; crate::compression::backend::ENOUGH_LENS],
    len_codes: [Code; crate::compression::backend::ENOUGH_LENS],
    dist_codes: [Code; crate::compression::backend::ENOUGH_DISTS],
    lens: [u16; 320],
    work: [u16; 288],
    allocation_start: *mut u8,
    total_allocation_size: usize,
}
impl<'a> State<'a> {
    fn new(reader: &'a [u8], writer: Writer<'a>) -> Self {
        let in_available = reader.len();
        let out_available = writer.capacity();
        Self {
            flush: InflateFlush::NoFlush,
            flags: Flags::default(),
            wrap: 0,
            mode: Mode::Head,
            length: 0,
            len_table: Table::default(),
            dist_table: Table::default(),
            wbits: 0,
            offset: 0,
            extra: 0,
            back: 0,
            was: 0,
            chunksize: 0,
            in_available,
            out_available,
            bit_reader: BitReader::new(reader),
            writer,
            total: 0,
            window: Window::empty(),
            head: None,
            lens: [0u16; 320],
            work: [0u16; 288],
            ncode: 0,
            nlen: 0,
            ndist: 0,
            have: 0,
            next: 0,
            error_message: None,
            checksum: 0,
            crc_fold: Crc32Fold::new(),
            dmax: 0,
            gzip_flags: 0,
            codes_codes: [Code::default(); crate::compression::backend::ENOUGH_LENS],
            len_codes: [Code::default(); crate::compression::backend::ENOUGH_LENS],
            dist_codes: [Code::default(); crate::compression::backend::ENOUGH_DISTS],
            allocation_start: core::ptr::null_mut(),
            total_allocation_size: 0,
        }
    }
    fn len_table_ref(&self) -> &[Code] {
        match self.len_table.codes {
            Codes::Fixed => &self::inffixed_tbl::LENFIX,
            Codes::Codes => &self.codes_codes,
            Codes::Len => &self.len_codes,
            Codes::Dist => &self.dist_codes,
        }
    }
    fn dist_table_ref(&self) -> &[Code] {
        match self.dist_table.codes {
            Codes::Fixed => &self::inffixed_tbl::DISTFIX,
            Codes::Codes => &self.codes_codes,
            Codes::Len => &self.len_codes,
            Codes::Dist => &self.dist_codes,
        }
    }
    fn len_table_get(&self, index: usize) -> Code {
        self.len_table_ref()[index]
    }
    fn dist_table_get(&self, index: usize) -> Code {
        self.dist_table_ref()[index]
    }
}
const fn zswap32(q: u32) -> u32 {
    u32::from_be(q.to_le())
}
const INFLATE_FAST_MIN_HAVE: usize = 15;
const INFLATE_FAST_MIN_LEFT: usize = 260;
impl State<'_> {
    fn len_and_friends(&mut self) -> ControlFlow<ReturnCode, ()> {
        let avail_in = self.bit_reader.bytes_remaining();
        let avail_out = self.writer.remaining();
        if avail_in >= INFLATE_FAST_MIN_HAVE && avail_out >= INFLATE_FAST_MIN_LEFT {
            unsafe { inflate_fast_help(self, 0) };
            match self.mode {
                Mode::Len => {}
                _ => return ControlFlow::Continue(()),
            }
        }
        let mut mode;
        let mut writer;
        let mut bit_reader;
        macro_rules! load {
            () => {
                mode = self.mode;
                writer = core::mem::replace(&mut self.writer, Writer::new(&mut []));
                bit_reader = self.bit_reader;
            };
        }
        macro_rules! restore {
            () => {
                self.mode = mode;
                self.writer = writer;
                self.bit_reader = bit_reader;
            };
        }
        load!();
        let len_table = match self.len_table.codes {
            Codes::Fixed => &self::inffixed_tbl::LENFIX[..],
            Codes::Codes => &self.codes_codes,
            Codes::Len => &self.len_codes,
            Codes::Dist => &self.dist_codes,
        };
        let dist_table = match self.dist_table.codes {
            Codes::Fixed => &self::inffixed_tbl::DISTFIX[..],
            Codes::Codes => &self.codes_codes,
            Codes::Len => &self.len_codes,
            Codes::Dist => &self.dist_codes,
        };
        loop {
            mode = 'top: {
                match mode {
                    Mode::Len => {
                        let avail_in = bit_reader.bytes_remaining();
                        let avail_out = writer.remaining();
                        if avail_in >= INFLATE_FAST_MIN_HAVE && avail_out >= INFLATE_FAST_MIN_LEFT {
                            restore!();
                            unsafe { inflate_fast_help(self, 0) };
                            return ControlFlow::Continue(());
                        }
                        self.back = 0;
                        let mut here;
                        loop {
                            let bits = bit_reader.bits(self.len_table.bits);
                            here = len_table[bits as usize];
                            if here.bits <= bit_reader.bits_in_buffer() {
                                break;
                            }
                            if let Err(return_code) = bit_reader.pull_byte() {
                                restore!();
                                return ControlFlow::Break(return_code);
                            };
                        }
                        if here.op != 0 && here.op & 0xf0 == 0 {
                            let last = here;
                            loop {
                                let bits = bit_reader.bits((last.bits + last.op) as usize) as u16;
                                here = len_table[(last.val + (bits >> last.bits)) as usize];
                                if last.bits + here.bits <= bit_reader.bits_in_buffer() {
                                    break;
                                }
                                if let Err(return_code) = bit_reader.pull_byte() {
                                    restore!();
                                    return ControlFlow::Break(return_code);
                                };
                            }
                            bit_reader.drop_bits(last.bits);
                            self.back += last.bits as usize;
                        }
                        bit_reader.drop_bits(here.bits);
                        self.back += here.bits as usize;
                        self.length = here.val as usize;
                        if here.op == 0 {
                            break 'top Mode::Lit;
                        } else if here.op & 32 != 0 {
                            traceln!("inflate:         end of block");
                            self.back = usize::MAX;
                            mode = Mode::Type;
                            restore!();
                            return ControlFlow::Continue(());
                        } else if here.op & 64 != 0 {
                            mode = Mode::Bad;
                            {
                                restore!();
                                let this = &mut *self;
                                let msg: &'static str = "invalid literal/length code\0";
                                this.error_message = Some(msg);
                                return ControlFlow::Break(ReturnCode::DataError);
                            }
                        } else {
                            self.extra = (here.op & MAX_BITS) as usize;
                            break 'top Mode::LenExt;
                        }
                    }
                    Mode::Lit => {
                        if writer.is_full() {
                            restore!();
                            traceln!("Ok: writer is full ({} bytes)", self.writer.capacity());
                            return ControlFlow::Break(ReturnCode::Ok);
                        }
                        writer.push(self.length as u8);
                        break 'top Mode::Len;
                    }
                    Mode::LenExt => {
                        let extra = self.extra;
                        if extra != 0 {
                            match bit_reader.need_bits(extra) {
                                Err(return_code) => {
                                    restore!();
                                    return ControlFlow::Break(return_code);
                                }
                                Ok(v) => v,
                            };
                            self.length += bit_reader.bits(extra) as usize;
                            bit_reader.drop_bits(extra as u8);
                            self.back += extra;
                        }
                        traceln!("inflate: length {}", self.length);
                        self.was = self.length;
                        break 'top Mode::Dist;
                    }
                    Mode::Dist => {
                        let mut here;
                        loop {
                            let bits = bit_reader.bits(self.dist_table.bits) as usize;
                            here = dist_table[bits];
                            if here.bits <= bit_reader.bits_in_buffer() {
                                break;
                            }
                            if let Err(return_code) = bit_reader.pull_byte() {
                                restore!();
                                return ControlFlow::Break(return_code);
                            };
                        }
                        if here.op & 0xf0 == 0 {
                            let last = here;
                            loop {
                                let bits = bit_reader.bits((last.bits + last.op) as usize);
                                here =
                                    dist_table[last.val as usize + ((bits as usize) >> last.bits)];
                                if last.bits + here.bits <= bit_reader.bits_in_buffer() {
                                    break;
                                }
                                if let Err(return_code) = bit_reader.pull_byte() {
                                    restore!();
                                    return ControlFlow::Break(return_code);
                                };
                            }
                            bit_reader.drop_bits(last.bits);
                            self.back += last.bits as usize;
                        }
                        bit_reader.drop_bits(here.bits);
                        if here.op & 64 != 0 {
                            restore!();
                            self.mode = Mode::Bad;
                            return ControlFlow::Break(self.bad("invalid distance code\0"));
                        }
                        self.offset = here.val as usize;
                        self.extra = (here.op & MAX_BITS) as usize;
                        break 'top Mode::DistExt;
                    }
                    Mode::DistExt => {
                        let extra = self.extra;
                        if extra > 0 {
                            match bit_reader.need_bits(extra) {
                                Err(return_code) => {
                                    restore!();
                                    return ControlFlow::Break(return_code);
                                }
                                Ok(v) => v,
                            };
                            self.offset += bit_reader.bits(extra) as usize;
                            bit_reader.drop_bits(extra as u8);
                            self.back += extra;
                        }
                        if INFLATE_STRICT && self.offset > self.dmax {
                            restore!();
                            self.mode = Mode::Bad;
                            return ControlFlow::Break(
                                self.bad("invalid distance code too far back\0"),
                            );
                        }
                        traceln!("inflate: distance {}", self.offset);
                        break 'top Mode::Match;
                    }
                    Mode::Match => {
                        if writer.is_full() {
                            restore!();
                            traceln!(
                                "BufError: writer is full ({} bytes)",
                                self.writer.capacity()
                            );
                            return ControlFlow::Break(ReturnCode::Ok);
                        }
                        let left = writer.remaining();
                        let copy = writer.len();
                        let copy = if self.offset > copy {
                            let mut copy = self.offset - copy;
                            if copy > self.window.have() {
                                if self.flags.contains(Flags::SANE) {
                                    restore!();
                                    self.mode = Mode::Bad;
                                    return ControlFlow::Break(
                                        self.bad("invalid distance too far back\0"),
                                    );
                                }
                                panic!("INFLATE_ALLOW_INVALID_DISTANCE_TOOFAR_ARRR")
                            }
                            let wnext = self.window.next();
                            let wsize = self.window.size();
                            let from = if copy > wnext {
                                copy -= wnext;
                                wsize - copy
                            } else {
                                wnext - copy
                            };
                            copy = Ord::min(copy, self.length);
                            copy = Ord::min(copy, left);
                            writer.extend_from_window(&self.window, from..from + copy);
                            copy
                        } else {
                            let copy = Ord::min(self.length, left);
                            writer.copy_match(self.offset, copy);
                            copy
                        };
                        self.length -= copy;
                        if self.length == 0 {
                            break 'top Mode::Len;
                        } else {
                            break 'top Mode::Match;
                        }
                    }
                    _ => unsafe { core::hint::unreachable_unchecked() },
                }
            }
        }
    }
    fn dispatch(&mut self) -> ReturnCode {
        let mut mode = self.mode;
        macro_rules! pull_byte {
            ($ self : expr) => {
                match $self.bit_reader.pull_byte() {
                    Err(return_code) => {
                        self.mode = mode;
                        return $self.inflate_leave(return_code);
                    }
                    Ok(_) => (),
                }
            };
        }
        macro_rules! need_bits {
            ($ self : expr , $ n : expr) => {
                match $self.bit_reader.need_bits($n) {
                    Err(return_code) => {
                        self.mode = mode;
                        return $self.inflate_leave(return_code);
                    }
                    Ok(v) => v,
                }
            };
        }
        let ret = 'label: loop {
            mode = 'blk: {
                match mode {
                    Mode::Head => {
                        if self.wrap == 0 {
                            break 'blk Mode::TypeDo;
                        }
                        need_bits!(self, 16);
                        if (self.wrap & 2) != 0 && self.bit_reader.hold() == 0x8b1f {
                            if self.wbits == 0 {
                                self.wbits = 15;
                            }
                            let b0 = self.bit_reader.bits(8) as u8;
                            let b1 = (self.bit_reader.hold() >> 8) as u8;
                            self.checksum =
                                crc32(crate::compression::backend::CRC32_INITIAL_VALUE, &[b0, b1]);
                            self.bit_reader.init_bits();
                            break 'blk Mode::Flags;
                        }
                        if let Some(header) = &mut self.head {
                            header.done = -1;
                        }
                        if (self.wrap & 1) == 0
                            || ((self.bit_reader.bits(8) << 8) + (self.bit_reader.hold() >> 8)) % 31
                                != 0
                        {
                            mode = Mode::Bad;
                            break 'label self.bad("incorrect header check\0");
                        }
                        if self.bit_reader.bits(4) != Z_DEFLATED as u64 {
                            mode = Mode::Bad;
                            break 'label self.bad("unknown compression method\0");
                        }
                        self.bit_reader.drop_bits(4);
                        let len = self.bit_reader.bits(4) as u8 + 8;
                        if self.wbits == 0 {
                            self.wbits = len;
                        }
                        if len as i32 > MAX_WBITS || len > self.wbits {
                            mode = Mode::Bad;
                            break 'label self.bad("invalid window size\0");
                        }
                        self.dmax = 1 << len;
                        self.gzip_flags = 0;
                        self.checksum = crate::compression::backend::ADLER32_INITIAL_VALUE as _;
                        if self.bit_reader.hold() & 0x200 != 0 {
                            self.bit_reader.init_bits();
                            break 'blk Mode::DictId;
                        } else {
                            self.bit_reader.init_bits();
                            break 'blk Mode::Type;
                        }
                    }
                    Mode::Flags => {
                        need_bits!(self, 16);
                        self.gzip_flags = self.bit_reader.hold() as i32;
                        if self.gzip_flags & 0xff != Z_DEFLATED {
                            mode = Mode::Bad;
                            break 'label self.bad("unknown compression method\0");
                        }
                        if self.gzip_flags & 0xe000 != 0 {
                            mode = Mode::Bad;
                            break 'label self.bad("unknown header flags set\0");
                        }
                        if let Some(head) = self.head.as_mut() {
                            head.text = ((self.bit_reader.hold() >> 8) & 1) as i32;
                        }
                        if (self.gzip_flags & 0x0200) != 0 && (self.wrap & 4) != 0 {
                            let b0 = self.bit_reader.bits(8) as u8;
                            let b1 = (self.bit_reader.hold() >> 8) as u8;
                            self.checksum = crc32(self.checksum, &[b0, b1]);
                        }
                        self.bit_reader.init_bits();
                        break 'blk Mode::Time;
                    }
                    Mode::Time => {
                        need_bits!(self, 32);
                        if let Some(head) = self.head.as_mut() {
                            head.time = self.bit_reader.hold() as ZSize;
                        }
                        if (self.gzip_flags & 0x0200) != 0 && (self.wrap & 4) != 0 {
                            let bytes = (self.bit_reader.hold() as u32).to_le_bytes();
                            self.checksum = crc32(self.checksum, &bytes);
                        }
                        self.bit_reader.init_bits();
                        break 'blk Mode::Os;
                    }
                    Mode::Os => {
                        need_bits!(self, 16);
                        if let Some(head) = self.head.as_mut() {
                            head.xflags = (self.bit_reader.hold() & 0xff) as i32;
                            head.os = (self.bit_reader.hold() >> 8) as i32;
                        }
                        if (self.gzip_flags & 0x0200) != 0 && (self.wrap & 4) != 0 {
                            let bytes = (self.bit_reader.hold() as u16).to_le_bytes();
                            self.checksum = crc32(self.checksum, &bytes);
                        }
                        self.bit_reader.init_bits();
                        break 'blk Mode::ExLen;
                    }
                    Mode::ExLen => {
                        if (self.gzip_flags & 0x0400) != 0 {
                            need_bits!(self, 16);
                            self.length = self.bit_reader.hold() as usize;
                            if let Some(head) = self.head.as_mut() {
                                head.extra_len = self.length as u32;
                            }
                            if (self.gzip_flags & 0x0200) != 0 && (self.wrap & 4) != 0 {
                                let bytes = (self.bit_reader.hold() as u16).to_le_bytes();
                                self.checksum = crc32(self.checksum, &bytes);
                            }
                            self.bit_reader.init_bits();
                        } else if let Some(head) = self.head.as_mut() {
                            head.extra = core::ptr::null_mut();
                        }
                        break 'blk Mode::Extra;
                    }
                    Mode::Extra => {
                        if (self.gzip_flags & 0x0400) != 0 {
                            let extra_available =
                                Ord::min(self.length, self.bit_reader.bytes_remaining());
                            if extra_available > 0 {
                                if let Some(head) = self.head.as_mut() {
                                    if !head.extra.is_null() {
                                        let written_so_far = head.extra_len as usize - self.length;
                                        let count = Ord::min(
                                            (head.extra_max as usize)
                                                .saturating_sub(written_so_far),
                                            extra_available,
                                        );
                                        let next_write_offset =
                                            Ord::min(written_so_far, head.extra_max as usize);
                                        unsafe {
                                            core::ptr::copy_nonoverlapping(
                                                self.bit_reader.as_mut_ptr(),
                                                head.extra.add(next_write_offset),
                                                count,
                                            );
                                        }
                                    }
                                }
                                if (self.gzip_flags & 0x0200) != 0 && (self.wrap & 4) != 0 {
                                    let extra_slice =
                                        &self.bit_reader.as_slice()[..extra_available];
                                    self.checksum = crc32(self.checksum, extra_slice)
                                }
                                self.in_available -= extra_available;
                                self.bit_reader.advance(extra_available);
                                self.length -= extra_available;
                            }
                            if self.length != 0 {
                                break 'label self.inflate_leave(ReturnCode::Ok);
                            }
                        }
                        self.length = 0;
                        break 'blk Mode::Name;
                    }
                    Mode::Name => {
                        if (self.gzip_flags & 0x0800) != 0 {
                            if self.in_available == 0 {
                                break 'label self.inflate_leave(ReturnCode::Ok);
                            }
                            let slice = self.bit_reader.as_slice();
                            let null_terminator_index = slice.iter().position(|c| *c == 0);
                            let name_slice = match null_terminator_index {
                                Some(i) => &slice[..=i],
                                None => slice,
                            };
                            if let Some(head) = self.head.as_mut() {
                                if !head.name.is_null() {
                                    let remaining_name_bytes = (head.name_max as usize)
                                        .checked_sub(self.length)
                                        .expect("name out of bounds");
                                    let copy = Ord::min(name_slice.len(), remaining_name_bytes);
                                    unsafe {
                                        core::ptr::copy_nonoverlapping(
                                            name_slice.as_ptr(),
                                            head.name.add(self.length),
                                            copy,
                                        )
                                    };
                                    self.length += copy;
                                }
                            }
                            if (self.gzip_flags & 0x0200) != 0 && (self.wrap & 4) != 0 {
                                self.checksum = crc32(self.checksum, name_slice);
                            }
                            let reached_end = name_slice.last() == Some(&0);
                            self.bit_reader.advance(name_slice.len());
                            if !reached_end && self.bit_reader.bytes_remaining() == 0 {
                                break 'label self.inflate_leave(ReturnCode::Ok);
                            }
                        } else if let Some(head) = self.head.as_mut() {
                            head.name = core::ptr::null_mut();
                        }
                        self.length = 0;
                        break 'blk Mode::Comment;
                    }
                    Mode::Comment => {
                        if (self.gzip_flags & 0x01000) != 0 {
                            if self.in_available == 0 {
                                break 'label self.inflate_leave(ReturnCode::Ok);
                            }
                            let slice = self.bit_reader.as_slice();
                            let null_terminator_index = slice.iter().position(|c| *c == 0);
                            let comment_slice = match null_terminator_index {
                                Some(i) => &slice[..=i],
                                None => slice,
                            };
                            if let Some(head) = self.head.as_mut() {
                                if !head.comment.is_null() {
                                    let remaining_comm_bytes = (head.comm_max as usize)
                                        .checked_sub(self.length)
                                        .expect("comm out of bounds");
                                    let copy = Ord::min(comment_slice.len(), remaining_comm_bytes);
                                    unsafe {
                                        core::ptr::copy_nonoverlapping(
                                            comment_slice.as_ptr(),
                                            head.comment.add(self.length),
                                            copy,
                                        )
                                    };
                                    self.length += copy;
                                }
                            }
                            if (self.gzip_flags & 0x0200) != 0 && (self.wrap & 4) != 0 {
                                self.checksum = crc32(self.checksum, comment_slice);
                            }
                            let reached_end = comment_slice.last() == Some(&0);
                            self.bit_reader.advance(comment_slice.len());
                            if !reached_end && self.bit_reader.bytes_remaining() == 0 {
                                break 'label self.inflate_leave(ReturnCode::Ok);
                            }
                        } else if let Some(head) = self.head.as_mut() {
                            head.comment = core::ptr::null_mut();
                        }
                        break 'blk Mode::HCrc;
                    }
                    Mode::HCrc => {
                        if (self.gzip_flags & 0x0200) != 0 {
                            need_bits!(self, 16);
                            if (self.wrap & 4) != 0
                                && self.bit_reader.hold() as u32 != (self.checksum & 0xffff)
                            {
                                mode = Mode::Bad;
                                break 'label self.bad("header crc mismatch\0");
                            }
                            self.bit_reader.init_bits();
                        }
                        if let Some(head) = self.head.as_mut() {
                            head.hcrc = (self.gzip_flags >> 9) & 1;
                            head.done = 1;
                        }
                        if (self.wrap & 4 != 0) && self.gzip_flags != 0 {
                            self.crc_fold = Crc32Fold::new();
                            self.checksum = crate::compression::backend::CRC32_INITIAL_VALUE;
                        }
                        break 'blk Mode::Type;
                    }
                    Mode::Type => {
                        use InflateFlush::*;
                        match self.flush {
                            Block | Trees => break 'label ReturnCode::Ok,
                            NoFlush | SyncFlush | Finish => {
                                break 'blk Mode::TypeDo;
                            }
                        }
                    }
                    Mode::TypeDo => {
                        if self.flags.contains(Flags::IS_LAST_BLOCK) {
                            self.bit_reader.next_byte_boundary();
                            break 'blk Mode::Check;
                        }
                        need_bits!(self, 3);
                        let last = self.bit_reader.bits(1) != 0;
                        self.flags.update(Flags::IS_LAST_BLOCK, last);
                        self.bit_reader.drop_bits(1);
                        match self.bit_reader.bits(2) {
                            0b00 => {
                                traceln!("inflate:     stored block (last = {last})");
                                self.bit_reader.drop_bits(2);
                                break 'blk Mode::Stored;
                            }
                            0b01 => {
                                traceln!("inflate:     fixed codes block (last = {last})");
                                self.len_table = Table {
                                    codes: Codes::Fixed,
                                    bits: 9,
                                };
                                self.dist_table = Table {
                                    codes: Codes::Fixed,
                                    bits: 5,
                                };
                                mode = Mode::Len_;
                                self.bit_reader.drop_bits(2);
                                if let InflateFlush::Trees = self.flush {
                                    break 'label self.inflate_leave(ReturnCode::Ok);
                                } else {
                                    break 'blk Mode::Len_;
                                }
                            }
                            0b10 => {
                                traceln!("inflate:     dynamic codes block (last = {last})");
                                self.bit_reader.drop_bits(2);
                                break 'blk Mode::Table;
                            }
                            0b11 => {
                                traceln!("inflate:     invalid block type");
                                self.bit_reader.drop_bits(2);
                                mode = Mode::Bad;
                                break 'label self.bad("invalid block type\0");
                            }
                            _ => {
                                unreachable!(
                                    "BitReader::bits(2) only yields a value of two bits, so this match is already exhaustive"
                                )
                            }
                        }
                    }
                    Mode::Stored => {
                        self.bit_reader.next_byte_boundary();
                        need_bits!(self, 32);
                        let hold = self.bit_reader.bits(32) as u32;
                        traceln!("hold {hold:#x}");
                        if hold as u16 != !((hold >> 16) as u16) {
                            mode = Mode::Bad;
                            break 'label self.bad("invalid stored block lengths\0");
                        }
                        self.length = hold as usize & 0xFFFF;
                        traceln!("inflate:     stored length {}", self.length);
                        self.bit_reader.init_bits();
                        if let InflateFlush::Trees = self.flush {
                            break 'label self.inflate_leave(ReturnCode::Ok);
                        } else {
                            break 'blk Mode::CopyBlock;
                        }
                    }
                    Mode::CopyBlock => {
                        loop {
                            let mut copy = self.length;
                            if copy == 0 {
                                break;
                            }
                            copy = Ord::min(copy, self.writer.remaining());
                            copy = Ord::min(copy, self.bit_reader.bytes_remaining());
                            if copy == 0 {
                                break 'label self.inflate_leave(ReturnCode::Ok);
                            }
                            self.writer.extend(&self.bit_reader.as_slice()[..copy]);
                            self.bit_reader.advance(copy);
                            self.length -= copy;
                        }
                        break 'blk Mode::Type;
                    }
                    Mode::Check => {
                        if !cfg!(any()) && self.wrap != 0 {
                            need_bits!(self, 32);
                            self.total += self.writer.len();
                            if self.wrap & 4 != 0 {
                                if self.gzip_flags != 0 {
                                    self.crc_fold.fold(self.writer.filled(), self.checksum);
                                    self.checksum = self.crc_fold.finish();
                                } else {
                                    self.checksum = adler32(self.checksum, self.writer.filled());
                                }
                            }
                            let given_checksum = if self.gzip_flags != 0 {
                                self.bit_reader.hold() as u32
                            } else {
                                zswap32(self.bit_reader.hold() as u32)
                            };
                            self.out_available = self.writer.capacity() - self.writer.len();
                            if self.wrap & 4 != 0 && given_checksum != self.checksum {
                                mode = Mode::Bad;
                                break 'label self.bad("incorrect data check\0");
                            }
                            self.bit_reader.init_bits();
                        }
                        break 'blk Mode::Length;
                    }
                    Mode::Len_ => {
                        break 'blk Mode::Len;
                    }
                    Mode::Len => {
                        self.mode = mode;
                        let val = self.len_and_friends();
                        mode = self.mode;
                        match val {
                            ControlFlow::Break(return_code) => break 'label return_code,
                            ControlFlow::Continue(()) => continue 'label,
                        }
                    }
                    Mode::LenExt => {
                        let extra = self.extra;
                        if extra != 0 {
                            need_bits!(self, extra);
                            self.length += self.bit_reader.bits(extra) as usize;
                            self.bit_reader.drop_bits(extra as u8);
                            self.back += extra;
                        }
                        traceln!("inflate: length {}", self.length);
                        self.was = self.length;
                        break 'blk Mode::Dist;
                    }
                    Mode::Lit => {
                        if self.writer.is_full() {
                            traceln!("Ok: writer is full ({} bytes)", self.writer.capacity());
                            break 'label self.inflate_leave(ReturnCode::Ok);
                        }
                        self.writer.push(self.length as u8);
                        break 'blk Mode::Len;
                    }
                    Mode::Dist => {
                        let mut here;
                        loop {
                            let bits = self.bit_reader.bits(self.dist_table.bits) as usize;
                            here = self.dist_table_get(bits);
                            if here.bits <= self.bit_reader.bits_in_buffer() {
                                break;
                            }
                            pull_byte!(self);
                        }
                        if here.op & 0xf0 == 0 {
                            let last = here;
                            loop {
                                let bits = self.bit_reader.bits((last.bits + last.op) as usize);
                                here = self.dist_table_get(
                                    last.val as usize + ((bits as usize) >> last.bits),
                                );
                                if last.bits + here.bits <= self.bit_reader.bits_in_buffer() {
                                    break;
                                }
                                pull_byte!(self);
                            }
                            self.bit_reader.drop_bits(last.bits);
                            self.back += last.bits as usize;
                        }
                        self.bit_reader.drop_bits(here.bits);
                        if here.op & 64 != 0 {
                            mode = Mode::Bad;
                            break 'label self.bad("invalid distance code\0");
                        }
                        self.offset = here.val as usize;
                        self.extra = (here.op & MAX_BITS) as usize;
                        break 'blk Mode::DistExt;
                    }
                    Mode::DistExt => {
                        let extra = self.extra;
                        if extra > 0 {
                            need_bits!(self, extra);
                            self.offset += self.bit_reader.bits(extra) as usize;
                            self.bit_reader.drop_bits(extra as u8);
                            self.back += extra;
                        }
                        if INFLATE_STRICT && self.offset > self.dmax {
                            mode = Mode::Bad;
                            break 'label self.bad("invalid distance code too far back\0");
                        }
                        traceln!("inflate: distance {}", self.offset);
                        break 'blk Mode::Match;
                    }
                    Mode::Match => 'match_: loop {
                        if self.writer.is_full() {
                            traceln!(
                                "BufError: writer is full ({} bytes)",
                                self.writer.capacity()
                            );
                            break 'label self.inflate_leave(ReturnCode::Ok);
                        }
                        let left = self.writer.remaining();
                        let copy = self.writer.len();
                        let copy = if self.offset > copy {
                            let mut copy = self.offset - copy;
                            if copy > self.window.have() {
                                if self.flags.contains(Flags::SANE) {
                                    mode = Mode::Bad;
                                    break 'label self.bad("invalid distance too far back\0");
                                }
                                panic!("INFLATE_ALLOW_INVALID_DISTANCE_TOOFAR_ARRR")
                            }
                            let wnext = self.window.next();
                            let wsize = self.window.size();
                            let from = if copy > wnext {
                                copy -= wnext;
                                wsize - copy
                            } else {
                                wnext - copy
                            };
                            copy = Ord::min(copy, self.length);
                            copy = Ord::min(copy, left);
                            self.writer
                                .extend_from_window(&self.window, from..from + copy);
                            copy
                        } else {
                            let copy = Ord::min(self.length, left);
                            self.writer.copy_match(self.offset, copy);
                            copy
                        };
                        self.length -= copy;
                        if self.length == 0 {
                            break 'blk Mode::Len;
                        } else {
                            continue 'match_;
                        }
                    },
                    Mode::Table => {
                        need_bits!(self, 14);
                        self.nlen = self.bit_reader.bits(5) as usize + 257;
                        self.bit_reader.drop_bits(5);
                        self.ndist = self.bit_reader.bits(5) as usize + 1;
                        self.bit_reader.drop_bits(5);
                        self.ncode = self.bit_reader.bits(4) as usize + 4;
                        self.bit_reader.drop_bits(4);
                        if self.nlen > 286 || self.ndist > 30 {
                            mode = Mode::Bad;
                            break 'label self.bad("too many length or distance symbols\0");
                        }
                        self.have = 0;
                        break 'blk Mode::LenLens;
                    }
                    Mode::LenLens => {
                        const ORDER: [u8; 19] = [
                            16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                        ];
                        while self.have < self.ncode {
                            need_bits!(self, 3);
                            self.lens[usize::from(ORDER[self.have])] =
                                self.bit_reader.bits(3) as u16;
                            self.have += 1;
                            self.bit_reader.drop_bits(3);
                        }
                        while self.have < 19 {
                            self.lens[usize::from(ORDER[self.have])] = 0;
                            self.have += 1;
                        }
                        let InflateTable::Success { root, used } = inflate_table(
                            CodeType::Codes,
                            &self.lens[..19],
                            &mut self.codes_codes,
                            7,
                            &mut self.work,
                        ) else {
                            mode = Mode::Bad;
                            break 'label self.bad("invalid code lengths set\0");
                        };
                        self.next = used;
                        self.len_table.codes = Codes::Codes;
                        self.len_table.bits = root;
                        self.have = 0;
                        break 'blk Mode::CodeLens;
                    }
                    Mode::CodeLens => {
                        while self.have < self.nlen + self.ndist {
                            let here = loop {
                                let bits = self.bit_reader.bits(self.len_table.bits);
                                let here = self.len_table_get(bits as usize);
                                if here.bits <= self.bit_reader.bits_in_buffer() {
                                    break here;
                                }
                                pull_byte!(self);
                            };
                            let here_bits = here.bits;
                            match here.val {
                                0..=15 => {
                                    self.bit_reader.drop_bits(here_bits);
                                    self.lens[self.have] = here.val;
                                    self.have += 1;
                                }
                                16 => {
                                    need_bits!(self, usize::from(here_bits) + 2);
                                    self.bit_reader.drop_bits(here_bits);
                                    if self.have == 0 {
                                        mode = Mode::Bad;
                                        break 'label self.bad("invalid bit length repeat\0");
                                    }
                                    let len = self.lens[self.have - 1];
                                    let copy = 3 + self.bit_reader.bits(2) as usize;
                                    self.bit_reader.drop_bits(2);
                                    if self.have + copy > self.nlen + self.ndist {
                                        mode = Mode::Bad;
                                        break 'label self.bad("invalid bit length repeat\0");
                                    }
                                    self.lens[self.have..][..copy].fill(len);
                                    self.have += copy;
                                }
                                17 => {
                                    need_bits!(self, usize::from(here_bits) + 3);
                                    self.bit_reader.drop_bits(here_bits);
                                    let copy = 3 + self.bit_reader.bits(3) as usize;
                                    self.bit_reader.drop_bits(3);
                                    if self.have + copy > self.nlen + self.ndist {
                                        mode = Mode::Bad;
                                        break 'label self.bad("invalid bit length repeat\0");
                                    }
                                    self.lens[self.have..][..copy].fill(0);
                                    self.have += copy;
                                }
                                18.. => {
                                    need_bits!(self, usize::from(here_bits) + 7);
                                    self.bit_reader.drop_bits(here_bits);
                                    let copy = 11 + self.bit_reader.bits(7) as usize;
                                    self.bit_reader.drop_bits(7);
                                    if self.have + copy > self.nlen + self.ndist {
                                        mode = Mode::Bad;
                                        break 'label self.bad("invalid bit length repeat\0");
                                    }
                                    self.lens[self.have..][..copy].fill(0);
                                    self.have += copy;
                                }
                            }
                        }
                        if self.lens[256] == 0 {
                            mode = Mode::Bad;
                            break 'label self.bad("invalid code -- missing end-of-block\0");
                        }
                        let InflateTable::Success { root, used } = inflate_table(
                            CodeType::Lens,
                            &self.lens[..self.nlen],
                            &mut self.len_codes,
                            10,
                            &mut self.work,
                        ) else {
                            mode = Mode::Bad;
                            break 'label self.bad("invalid literal/lengths set\0");
                        };
                        self.len_table.codes = Codes::Len;
                        self.len_table.bits = root;
                        self.next = used;
                        let InflateTable::Success { root, used } = inflate_table(
                            CodeType::Dists,
                            &self.lens[self.nlen..][..self.ndist],
                            &mut self.dist_codes,
                            9,
                            &mut self.work,
                        ) else {
                            mode = Mode::Bad;
                            break 'label self.bad("invalid distances set\0");
                        };
                        self.dist_table.bits = root;
                        self.dist_table.codes = Codes::Dist;
                        self.next += used;
                        mode = Mode::Len_;
                        if matches!(self.flush, InflateFlush::Trees) {
                            break 'label self.inflate_leave(ReturnCode::Ok);
                        }
                        break 'blk Mode::Len_;
                    }
                    Mode::Dict => {
                        if !self.flags.contains(Flags::HAVE_DICT) {
                            break 'label self.inflate_leave(ReturnCode::NeedDict);
                        }
                        self.checksum = crate::compression::backend::ADLER32_INITIAL_VALUE as _;
                        break 'blk Mode::Type;
                    }
                    Mode::DictId => {
                        need_bits!(self, 32);
                        self.checksum = zswap32(self.bit_reader.hold() as u32);
                        self.bit_reader.init_bits();
                        break 'blk Mode::Dict;
                    }
                    Mode::Done => {
                        break 'label ReturnCode::StreamEnd;
                    }
                    Mode::Bad => {
                        let msg = "repeated call with bad state\0";
                        self.error_message = Some(msg);
                        break 'label ReturnCode::DataError;
                    }
                    Mode::Mem => {
                        break 'label ReturnCode::MemError;
                    }
                    Mode::Sync => {
                        break 'label ReturnCode::StreamError;
                    }
                    Mode::Length => {
                        if self.wrap != 0 && self.gzip_flags != 0 {
                            need_bits!(self, 32);
                            if (self.wrap & 0b100) != 0
                                && self.bit_reader.hold() as u32 != self.total as u32
                            {
                                mode = Mode::Bad;
                                break 'label self.bad("incorrect length check\0");
                            }
                            self.bit_reader.init_bits();
                        }
                        mode = Mode::Done;
                        break 'label ReturnCode::StreamEnd;
                    }
                };
            }
        };
        self.mode = mode;
        ret
    }
    fn bad(&mut self, msg: &'static str) -> ReturnCode {
        self.error_message = Some(msg);
        self.inflate_leave(ReturnCode::DataError)
    }
    fn inflate_leave(&mut self, return_code: ReturnCode) -> ReturnCode {
        return_code
    }
    fn decoding_state(&self) -> i32 {
        let bit_reader_bits = self.bit_reader.bits_in_buffer() as i32;
        debug_assert!(bit_reader_bits < 64);
        let last = if self.flags.contains(Flags::IS_LAST_BLOCK) {
            64
        } else {
            0
        };
        let mode = match self.mode {
            Mode::Type => 128,
            Mode::Len_ | Mode::CopyBlock => 256,
            _ => 0,
        };
        bit_reader_bits | last | mode
    }
}
unsafe fn inflate_fast_help(state: &mut State, start: usize) {
    {
        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        if crate::cpu_features::is_enabled_avx2_and_bmi2() {
            return unsafe { inflate_fast_help_avx2(state, start) };
        }
        unsafe { inflate_fast_help_vanilla(state, start) };
    }
}
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[target_feature(enable = "avx2")]
#[target_feature(enable = "bmi2")]
#[target_feature(enable = "bmi1")]
unsafe fn inflate_fast_help_avx2(state: &mut State, start: usize) {
    unsafe {
        unsafe { inflate_fast_help_impl::<{ CpuFeatures::AVX2 }>(state, start) };
    }
}
unsafe fn inflate_fast_help_vanilla(state: &mut State, start: usize) {
    {
        unsafe { inflate_fast_help_impl::<{ CpuFeatures::NONE }>(state, start) };
    }
}
#[inline(always)]
unsafe fn inflate_fast_help_impl<const FEATURES: usize>(state: &mut State, _start: usize) {
    {
        let mut bit_reader = BitReader::new(&[]);
        core::mem::swap(&mut bit_reader, &mut state.bit_reader);
        debug_assert!(bit_reader.bytes_remaining() >= 15);
        let mut writer = Writer::new(&mut []);
        core::mem::swap(&mut writer, &mut state.writer);
        let lcode = state.len_table_ref();
        let dcode = state.dist_table_ref();
        let lmask = (1u64 << state.len_table.bits) - 1;
        let dmask = (1u64 << state.dist_table.bits) - 1;
        let extra_safe = false;
        let window_size = state.window.size();
        let mut bad = None;
        if bit_reader.bits_in_buffer() < 10 {
            debug_assert!(bit_reader.bytes_remaining() >= 15);
            unsafe { bit_reader.refill() };
        }
        debug_assert!(
            bit_reader.bytes_remaining() >= 8
                && bit_reader.bytes_remaining_including_buffer() >= 15
        );
        'outer: loop {
            debug_assert!(
                bit_reader.bytes_remaining() >= 8
                    && bit_reader.bytes_remaining_including_buffer() >= 15
            );
            let mut here = {
                let bits = bit_reader.bits_in_buffer();
                let hold = bit_reader.hold();
                unsafe { bit_reader.refill() };
                debug_assert!(bit_reader.bytes_remaining() >= 8);
                if bits as usize >= state.len_table.bits {
                    lcode[(hold & lmask) as usize]
                } else {
                    lcode[(bit_reader.hold() & lmask) as usize]
                }
            };
            if here.op == 0 {
                writer.push(here.val as u8);
                bit_reader.drop_bits(here.bits);
                here = lcode[(bit_reader.hold() & lmask) as usize];
                if here.op == 0 {
                    writer.push(here.val as u8);
                    bit_reader.drop_bits(here.bits);
                    here = lcode[(bit_reader.hold() & lmask) as usize];
                }
            }
            'dolen: loop {
                bit_reader.drop_bits(here.bits);
                let op = here.op;
                if op == 0 {
                    writer.push(here.val as u8);
                } else if op & 16 != 0 {
                    let op = op & MAX_BITS;
                    let mut len = here.val + bit_reader.bits(op as usize) as u16;
                    bit_reader.drop_bits(op);
                    here = dcode[(bit_reader.hold() & dmask) as usize];
                    if bit_reader.bits_in_buffer() < MAX_BITS + MAX_DIST_EXTRA_BITS {
                        debug_assert!(bit_reader.bytes_remaining() >= 8);
                        unsafe { bit_reader.refill() };
                    }
                    'dodist: loop {
                        bit_reader.drop_bits(here.bits);
                        let op = here.op;
                        if op & 16 != 0 {
                            let op = op & MAX_BITS;
                            let dist = here.val + bit_reader.bits(op as usize) as u16;
                            if INFLATE_STRICT && dist as usize > state.dmax {
                                bad = Some("invalid distance too far back\0");
                                state.mode = Mode::Bad;
                                break 'outer;
                            }
                            bit_reader.drop_bits(op);
                            let written = writer.len();
                            if dist as usize > written {
                                if (dist as usize - written) > state.window.have() {
                                    if state.flags.contains(Flags::SANE) {
                                        bad = Some("invalid distance too far back\0");
                                        state.mode = Mode::Bad;
                                        break 'outer;
                                    }
                                    panic!("INFLATE_ALLOW_INVALID_DISTANCE_TOOFAR_ARRR")
                                }
                                let mut op = dist as usize - written;
                                let mut from;
                                let window_next = state.window.next();
                                if window_next == 0 {
                                    from = window_size - op;
                                } else if window_next >= op {
                                    from = window_next - op;
                                } else {
                                    op -= window_next;
                                    from = window_size - op;
                                    if op < len as usize {
                                        len -= op as u16;
                                        writer.extend_from_window_with_features::<FEATURES>(
                                            &state.window,
                                            from..from + op,
                                        );
                                        from = 0;
                                        op = window_next;
                                    }
                                }
                                let copy = Ord::min(op, len as usize);
                                writer.extend_from_window_with_features::<FEATURES>(
                                    &state.window,
                                    from..from + copy,
                                );
                                if op < len as usize {
                                    writer.copy_match_with_features::<FEATURES>(
                                        dist as usize,
                                        len as usize - op,
                                    );
                                }
                            } else if extra_safe {
                                todo!()
                            } else {
                                writer.copy_match_with_features::<FEATURES>(
                                    dist as usize,
                                    len as usize,
                                )
                            }
                        } else if (op & 64) == 0 {
                            here = dcode[(here.val + bit_reader.bits(op as usize) as u16) as usize];
                            continue 'dodist;
                        } else {
                            bad = Some("invalid distance code\0");
                            state.mode = Mode::Bad;
                            break 'outer;
                        }
                        break 'dodist;
                    }
                } else if (op & 64) == 0 {
                    here = lcode[(here.val + bit_reader.bits(op as usize) as u16) as usize];
                    continue 'dolen;
                } else if op & 32 != 0 {
                    state.mode = Mode::Type;
                    break 'outer;
                } else {
                    bad = Some("invalid literal/length code\0");
                    state.mode = Mode::Bad;
                    break 'outer;
                }
                break 'dolen;
            }
            let remaining = bit_reader.bytes_remaining_including_buffer();
            if remaining >= INFLATE_FAST_MIN_HAVE && writer.remaining() >= INFLATE_FAST_MIN_LEFT {
                continue;
            }
            break 'outer;
        }
        bit_reader.return_unused_bytes();
        state.bit_reader = bit_reader;
        state.writer = writer;
        if let Some(error_message) = bad {
            debug_assert!(matches!(state.mode, Mode::Bad));
            state.bad(error_message);
        }
    }
}

struct InflateAllocOffsets {
    total_size: usize,
    state_pos: usize,
    window_pos: usize,
}
impl InflateAllocOffsets {
    fn new() -> Self {
        use core::mem::size_of;
        const WINDOW_PAD_SIZE: usize = 64;
        const ALIGN_SIZE: usize = 64;
        let mut curr_size = 0usize;
        let state_size = size_of::<State>();
        let window_size = (1 << MAX_WBITS) + WINDOW_PAD_SIZE;
        let state_pos = curr_size.next_multiple_of(ALIGN_SIZE);
        curr_size = state_pos + state_size;
        let window_pos = curr_size.next_multiple_of(ALIGN_SIZE);
        curr_size = window_pos + window_size;
        let total_size = (curr_size + (ALIGN_SIZE - 1)).next_multiple_of(ALIGN_SIZE);
        Self {
            total_size,
            state_pos,
            window_pos,
        }
    }
}
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct InflateConfig {
    pub window_bits: i32,
}
impl Default for InflateConfig {
    fn default() -> Self {
        Self {
            window_bits: DEF_WBITS,
        }
    }
}
pub fn init(stream: &mut ZStream, config: InflateConfig) -> ReturnCode {
    stream.msg = core::ptr::null_mut();
    if stream.zalloc.is_none() || stream.zfree.is_none() {
        stream.configure_default_rust_allocator()
    }
    if stream.zalloc.is_none() || stream.zfree.is_none() {
        return ReturnCode::StreamError;
    }
    let mut state = State::new(&[], Writer::new(&mut []));
    state.chunksize = 32;
    let alloc = Allocator {
        zalloc: stream.zalloc.unwrap(),
        zfree: stream.zfree.unwrap(),
        opaque: stream.opaque,
        _marker: PhantomData,
    };
    let allocs = InflateAllocOffsets::new();
    let Some(allocation_start) = alloc.allocate_slice_raw::<u8>(allocs.total_size) else {
        return ReturnCode::MemError;
    };
    let address = allocation_start.as_ptr() as usize;
    let align_offset = address.next_multiple_of(64) - address;
    let buf = unsafe { allocation_start.as_ptr().add(align_offset) };
    let window_allocation = unsafe { buf.add(allocs.window_pos) };
    let window = unsafe { Window::from_raw_parts(window_allocation, (1 << MAX_WBITS) + 64) };
    state.window = window;
    let state_allocation = unsafe { buf.add(allocs.state_pos).cast::<State>() };
    unsafe { state_allocation.write(state) };
    stream.state = state_allocation.cast::<InternalState>();
    if let Some(stream) = unsafe { InflateStream::from_stream_mut(stream) } {
        stream.state.allocation_start = allocation_start.as_ptr();
        stream.state.total_allocation_size = allocs.total_size;
        let ret = reset_with_config(stream, config);
        if ret != ReturnCode::Ok {
            end(stream);
        }
        ret
    } else {
        ReturnCode::StreamError
    }
}
pub fn reset_with_config(stream: &mut InflateStream, config: InflateConfig) -> ReturnCode {
    let mut window_bits = config.window_bits;
    let wrap;
    if window_bits < 0 {
        wrap = 0;
        if window_bits < -MAX_WBITS {
            return ReturnCode::StreamError;
        }
        window_bits = -window_bits;
    } else {
        wrap = (window_bits >> 4) + 5;
        if window_bits < 48 {
            window_bits &= MAX_WBITS;
        }
    }
    if window_bits != 0 && !(MIN_WBITS..=MAX_WBITS).contains(&window_bits) {
        traceln!("invalid windowBits");
        return ReturnCode::StreamError;
    }
    stream.state.wrap = wrap as u8;
    stream.state.wbits = window_bits as _;
    reset(stream)
}
pub fn reset(stream: &mut InflateStream) -> ReturnCode {
    stream.state.window.clear();
    stream.state.error_message = None;
    reset_keep(stream)
}
pub fn reset_keep(stream: &mut InflateStream) -> ReturnCode {
    stream.total_in = 0;
    stream.total_out = 0;
    stream.state.total = 0;
    stream.msg = core::ptr::null_mut();
    let state = &mut stream.state;
    if state.wrap != 0 {
        stream.adler = (state.wrap & 1) as _;
    }
    state.mode = Mode::Head;
    state.checksum = crate::compression::backend::ADLER32_INITIAL_VALUE as u32;
    state.flags.update(Flags::IS_LAST_BLOCK, false);
    state.flags.update(Flags::HAVE_DICT, false);
    state.flags.update(Flags::SANE, true);
    state.gzip_flags = -1;
    state.dmax = 32768;
    state.head = None;
    state.bit_reader = BitReader::new(&[]);
    state.next = 0;
    state.len_table = Table::default();
    state.dist_table = Table::default();
    state.back = usize::MAX;
    ReturnCode::Ok
}

pub unsafe fn inflate(stream: &mut InflateStream, flush: InflateFlush) -> ReturnCode {
    {
        if stream.next_out.is_null() || (stream.next_in.is_null() && stream.avail_in != 0) {
            return ReturnCode::StreamError;
        }
        let state = &mut stream.state;
        if let Mode::Type = state.mode {
            state.mode = Mode::TypeDo;
        }
        state.flush = flush;
        unsafe {
            state
                .bit_reader
                .update_slice(stream.next_in, stream.avail_in as usize)
        };
        state.writer =
            unsafe { Writer::new_uninit(stream.next_out.cast(), stream.avail_out as usize) };
        state.in_available = stream.avail_in as _;
        state.out_available = stream.avail_out as _;
        let err = state.dispatch();
        let in_read = state.bit_reader.as_ptr() as usize - stream.next_in as usize;
        let out_written = state.out_available - (state.writer.capacity() - state.writer.len());
        stream.total_in += in_read as ZSize;
        state.total = state.total.wrapping_add(out_written);
        stream.total_out = state.total as _;
        stream.avail_in = state.bit_reader.bytes_remaining() as u32;
        stream.next_in = state.bit_reader.as_ptr() as *mut u8;
        stream.avail_out = (state.writer.capacity() - state.writer.len()) as u32;
        stream.next_out = state.writer.next_out() as *mut u8;
        stream.adler = state.checksum as ZChecksum;
        let valid_mode = |mode| !matches!(mode, Mode::Bad | Mode::Mem | Mode::Sync);
        let not_done = |mode| {
            !matches!(
                mode,
                Mode::Check | Mode::Length | Mode::Bad | Mode::Mem | Mode::Sync
            )
        };
        let must_update_window = state.window.size() != 0
            || (out_written != 0
                && valid_mode(state.mode)
                && (not_done(state.mode) || !matches!(state.flush, InflateFlush::Finish)));
        let update_checksum = state.wrap & 4 != 0;
        if must_update_window {
            state.window.extend(
                &state.writer.filled()[..out_written],
                state.gzip_flags,
                update_checksum,
                &mut state.checksum,
                &mut state.crc_fold,
            );
        }
        if let Some(msg) = state.error_message {
            assert!(msg.ends_with('\0'));
            stream.msg = msg.as_ptr() as *mut u8 as *mut core::ffi::c_char;
        }
        stream.data_type = state.decoding_state();
        if ((in_read == 0 && out_written == 0) || flush == InflateFlush::Finish)
            && err == ReturnCode::Ok
        {
            ReturnCode::BufError
        } else {
            err
        }
    }
}

pub fn end<'a>(stream: &'a mut InflateStream<'_>) -> &'a mut ZStream {
    let alloc = stream.alloc;
    let allocation_start = stream.state.allocation_start;
    let total_allocation_size = stream.state.total_allocation_size;
    let mut window = Window::empty();
    core::mem::swap(&mut window, &mut stream.state.window);
    let stream = stream.as_z_stream_mut();
    let _ = core::mem::replace(&mut stream.state, core::ptr::null_mut());
    unsafe { alloc.deallocate(allocation_start, total_allocation_size) };
    stream
}
