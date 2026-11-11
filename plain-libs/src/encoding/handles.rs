use super::DecoderResult;
use crate::encoding::ascii::*;
use crate::encoding::utf_8::utf8_valid_up_to;
pub trait Endian {
    const OPPOSITE_ENDIAN: bool;
}
impl UnalignedU16Slice {
    #[inline(always)]
    pub unsafe fn new(ptr: *const u8, len: usize) -> UnalignedU16Slice {
        { UnalignedU16Slice { ptr, len } }
    }
    #[inline(always)]
    pub fn trim_last(&mut self) {
        assert!(self.len > 0);
        self.len -= 1;
    }
    #[inline(always)]
    pub fn at(&self, i: usize) -> u16 {
        use core::mem::MaybeUninit;
        assert!(i < self.len);
        unsafe {
            let mut u: MaybeUninit<u16> = MaybeUninit::uninit();
            ::core::ptr::copy_nonoverlapping(self.ptr.add(i * 2), u.as_mut_ptr() as *mut u8, 2);
            u.assume_init()
        }
    }
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }
    #[inline(always)]
    pub fn tail(&self, from: usize) -> UnalignedU16Slice {
        assert!(from <= self.len);
        unsafe { UnalignedU16Slice::new(self.ptr.add(from * 2), self.len - from) }
    }
}
#[inline(always)]
fn copy_unaligned_basic_latin_to_ascii_alu<E: Endian>(
    src: UnalignedU16Slice,
    dst: &mut [u8],
    offset: usize,
) -> CopyAsciiResult<usize, (u16, usize)> {
    let len = ::core::cmp::min(src.len(), dst.len());
    let mut i = 0usize;
    loop {
        if i == len {
            return CopyAsciiResult::Stop(i + offset);
        }
        let unit = swap_if_opposite_endian::<E>(src.at(i));
        if unit > 0x7F {
            return CopyAsciiResult::GoOn((unit, i + offset));
        }
        dst[i] = unit as u8;
        i += 1;
    }
}
#[inline(always)]
fn swap_if_opposite_endian<E: Endian>(unit: u16) -> u16 {
    if E::OPPOSITE_ENDIAN {
        unit.swap_bytes()
    } else {
        unit
    }
}
#[inline(always)]
fn copy_unaligned_basic_latin_to_ascii<E: Endian>(
    src: UnalignedU16Slice,
    dst: &mut [u8],
) -> CopyAsciiResult<usize, (u16, usize)> {
    copy_unaligned_basic_latin_to_ascii_alu::<E>(src, dst, 0)
}
#[inline(always)]
fn convert_unaligned_utf16_to_utf8<E: Endian>(
    src: UnalignedU16Slice,
    dst: &mut [u8],
) -> (usize, usize, bool) {
    if dst.len() < 4 {
        return (0, 0, false);
    }
    let mut src_pos = 0usize;
    let mut dst_pos = 0usize;
    let src_len = src.len();
    let dst_len_minus_three = dst.len() - 3;
    'outer: loop {
        let mut non_ascii = match copy_unaligned_basic_latin_to_ascii::<E>(
            src.tail(src_pos),
            &mut dst[dst_pos..],
        ) {
            CopyAsciiResult::GoOn((unit, read_written)) => {
                src_pos += read_written;
                dst_pos += read_written;
                unit
            }
            CopyAsciiResult::Stop(read_written) => {
                return (src_pos + read_written, dst_pos + read_written, false);
            }
        };
        if dst_pos >= dst_len_minus_three {
            break 'outer;
        }
        src_pos += 1;
        'inner: loop {
            let non_ascii_minus_surrogate_start = non_ascii.wrapping_sub(0xD800);
            if non_ascii_minus_surrogate_start > (0xDFFF - 0xD800) {
                if non_ascii < 0x800 {
                    dst[dst_pos] = ((non_ascii >> 6) | 0xC0) as u8;
                    dst_pos += 1;
                    dst[dst_pos] = ((non_ascii & 0x3F) | 0x80) as u8;
                    dst_pos += 1;
                } else {
                    dst[dst_pos] = ((non_ascii >> 12) | 0xE0) as u8;
                    dst_pos += 1;
                    dst[dst_pos] = (((non_ascii & 0xFC0) >> 6) | 0x80) as u8;
                    dst_pos += 1;
                    dst[dst_pos] = ((non_ascii & 0x3F) | 0x80) as u8;
                    dst_pos += 1;
                }
            } else if non_ascii_minus_surrogate_start <= (0xDBFF - 0xD800) {
                if src_pos < src_len {
                    let second = swap_if_opposite_endian::<E>(src.at(src_pos));
                    let second_minus_low_surrogate_start = second.wrapping_sub(0xDC00);
                    if second_minus_low_surrogate_start <= (0xDFFF - 0xDC00) {
                        src_pos += 1;
                        let point = (u32::from(non_ascii) << 10) + u32::from(second)
                            - (((0xD800u32 << 10) - 0x10000u32) + 0xDC00u32);
                        dst[dst_pos] = ((point >> 18) | 0xF0u32) as u8;
                        dst_pos += 1;
                        dst[dst_pos] = (((point & 0x3F000u32) >> 12) | 0x80u32) as u8;
                        dst_pos += 1;
                        dst[dst_pos] = (((point & 0xFC0u32) >> 6) | 0x80u32) as u8;
                        dst_pos += 1;
                        dst[dst_pos] = ((point & 0x3Fu32) | 0x80u32) as u8;
                        dst_pos += 1;
                    } else {
                        return (src_pos, dst_pos, true);
                    }
                } else {
                    return (src_pos, dst_pos, true);
                }
            } else {
                return (src_pos, dst_pos, true);
            }
            if dst_pos >= dst_len_minus_three || src_pos == src_len {
                break 'outer;
            }
            let unit = swap_if_opposite_endian::<E>(src.at(src_pos));
            src_pos += 1;
            if unit > 0x7F {
                non_ascii = unit;
                continue 'inner;
            }
            dst[dst_pos] = unit as u8;
            dst_pos += 1;
            continue 'outer;
        }
    }
    (src_pos, dst_pos, false)
}
impl<'a> ByteSource<'a> {
    #[inline(always)]
    pub fn new(src: &'a [u8]) -> ByteSource<'a> {
        ByteSource { slice: src, pos: 0 }
    }
    #[inline(always)]
    pub fn check_available<'b>(&'b mut self) -> Space<ByteReadHandle<'b, 'a>> {
        if self.pos < self.slice.len() {
            Space::Available(ByteReadHandle::new(self))
        } else {
            Space::Full(self.consumed())
        }
    }
    #[inline(always)]
    fn read(&mut self) -> u8 {
        let ret = self.slice[self.pos];
        self.pos += 1;
        ret
    }
    #[inline(always)]
    fn unread(&mut self) -> usize {
        self.pos -= 1;
        self.pos
    }
    #[inline(always)]
    pub fn consumed(&self) -> usize {
        self.pos
    }
}
impl<'a, 'b> ByteReadHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(src: &'a mut ByteSource<'b>) -> ByteReadHandle<'a, 'b> {
        ByteReadHandle { source: src }
    }
    #[inline(always)]
    pub fn read(self) -> (u8, ByteUnreadHandle<'a, 'b>) {
        let byte = self.source.read();
        let handle = ByteUnreadHandle::new(self.source);
        (byte, handle)
    }
    #[inline(always)]
    pub fn consumed(&self) -> usize {
        self.source.consumed()
    }
}
impl<'a, 'b> ByteUnreadHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(src: &'a mut ByteSource<'b>) -> ByteUnreadHandle<'a, 'b> {
        ByteUnreadHandle { source: src }
    }
    #[inline(always)]
    pub fn unread(self) -> usize {
        self.source.unread()
    }
    #[inline(always)]
    pub fn consumed(&self) -> usize {
        self.source.consumed()
    }
    #[inline(always)]
    pub fn commit(self) -> &'a mut ByteSource<'b> {
        self.source
    }
}
impl<'a, 'b> Utf8BmpHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(dst: &'a mut Utf8Destination<'b>) -> Utf8BmpHandle<'a, 'b> {
        Utf8BmpHandle { dest: dst }
    }
    #[inline(always)]
    pub fn written(&self) -> usize {
        self.dest.written()
    }
    #[inline(always)]
    pub fn write_ascii(self, ascii: u8) -> &'a mut Utf8Destination<'b> {
        self.dest.write_ascii(ascii);
        self.dest
    }
    #[inline(always)]
    pub fn write_bmp(self, bmp: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_bmp(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_bmp_excl_ascii(self, bmp: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_bmp_excl_ascii(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_mid_bmp(self, bmp: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_mid_bmp(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_upper_bmp(self, bmp: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_upper_bmp(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn commit(self) -> &'a mut Utf8Destination<'b> {
        self.dest
    }
}
impl<'a, 'b> Utf8AstralHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(dst: &'a mut Utf8Destination<'b>) -> Utf8AstralHandle<'a, 'b> {
        Utf8AstralHandle { dest: dst }
    }
    #[inline(always)]
    pub fn written(&self) -> usize {
        self.dest.written()
    }
    #[inline(always)]
    pub fn write_ascii(self, ascii: u8) -> &'a mut Utf8Destination<'b> {
        self.dest.write_ascii(ascii);
        self.dest
    }
    #[inline(always)]
    pub fn write_bmp(self, bmp: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_bmp(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_bmp_excl_ascii(self, bmp: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_bmp_excl_ascii(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_upper_bmp(self, bmp: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_upper_bmp(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_astral(self, astral: u32) -> &'a mut Utf8Destination<'b> {
        self.dest.write_astral(astral);
        self.dest
    }
    #[inline(always)]
    pub fn write_surrogate_pair(self, high: u16, low: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_surrogate_pair(high, low);
        self.dest
    }
    #[inline(always)]
    pub fn write_big5_combination(
        self,
        combined: u16,
        combining: u16,
    ) -> &'a mut Utf8Destination<'b> {
        self.dest.write_big5_combination(combined, combining);
        self.dest
    }
    #[inline(always)]
    pub fn commit(self) -> &'a mut Utf8Destination<'b> {
        self.dest
    }
}
impl<'a> Utf8Destination<'a> {
    #[inline(always)]
    pub fn new(dst: &mut [u8]) -> Utf8Destination<'_> {
        Utf8Destination { slice: dst, pos: 0 }
    }
    #[inline(always)]
    pub fn check_space_bmp<'b>(&'b mut self) -> Space<Utf8BmpHandle<'b, 'a>> {
        if self.pos + 2 < self.slice.len() {
            Space::Available(Utf8BmpHandle::new(self))
        } else {
            Space::Full(self.written())
        }
    }
    #[inline(always)]
    pub fn check_space_astral<'b>(&'b mut self) -> Space<Utf8AstralHandle<'b, 'a>> {
        if self.pos + 3 < self.slice.len() {
            Space::Available(Utf8AstralHandle::new(self))
        } else {
            Space::Full(self.written())
        }
    }
    #[inline(always)]
    pub fn written(&self) -> usize {
        self.pos
    }
    #[inline(always)]
    fn write_code_unit(&mut self, u: u8) {
        unsafe {
            *(self.slice.get_unchecked_mut(self.pos)) = u;
        }
        self.pos += 1;
    }
    #[inline(always)]
    fn write_ascii(&mut self, ascii: u8) {
        debug_assert!(ascii < 0x80);
        self.write_code_unit(ascii);
    }
    #[inline(always)]
    fn write_bmp(&mut self, bmp: u16) {
        if bmp < 0x80u16 {
            self.write_ascii(bmp as u8);
        } else if bmp < 0x800u16 {
            self.write_mid_bmp(bmp);
        } else {
            self.write_upper_bmp(bmp);
        }
    }
    #[inline(always)]
    fn write_mid_bmp(&mut self, mid_bmp: u16) {
        debug_assert!(mid_bmp >= 0x80);
        debug_assert!(mid_bmp < 0x800);
        self.write_code_unit(((mid_bmp >> 6) | 0xC0) as u8);
        self.write_code_unit(((mid_bmp & 0x3F) | 0x80) as u8);
    }
    #[inline(always)]
    fn write_upper_bmp(&mut self, upper_bmp: u16) {
        debug_assert!(upper_bmp >= 0x800);
        self.write_code_unit(((upper_bmp >> 12) | 0xE0) as u8);
        self.write_code_unit((((upper_bmp & 0xFC0) >> 6) | 0x80) as u8);
        self.write_code_unit(((upper_bmp & 0x3F) | 0x80) as u8);
    }
    #[inline(always)]
    fn write_bmp_excl_ascii(&mut self, bmp: u16) {
        if bmp < 0x800u16 {
            self.write_mid_bmp(bmp);
        } else {
            self.write_upper_bmp(bmp);
        }
    }
    #[inline(always)]
    fn write_astral(&mut self, astral: u32) {
        debug_assert!(astral > 0xFFFF);
        debug_assert!(astral <= 0x10_FFFF);
        self.write_code_unit(((astral >> 18) | 0xF0) as u8);
        self.write_code_unit((((astral & 0x3F000) >> 12) | 0x80) as u8);
        self.write_code_unit((((astral & 0xFC0) >> 6) | 0x80) as u8);
        self.write_code_unit(((astral & 0x3F) | 0x80) as u8);
    }
    #[inline(always)]
    pub fn write_surrogate_pair(&mut self, high: u16, low: u16) {
        self.write_astral(
            (u32::from(high) << 10) + u32::from(low)
                - (((0xD800u32 << 10) - 0x10000u32) + 0xDC00u32),
        );
    }
    #[inline(always)]
    fn write_big5_combination(&mut self, combined: u16, combining: u16) {
        self.write_mid_bmp(combined);
        self.write_mid_bmp(combining);
    }
    #[inline(always)]
    pub fn copy_ascii_from_check_space_bmp<'b>(
        &'b mut self,
        source: &mut ByteSource,
    ) -> CopyAsciiResult<(DecoderResult, usize, usize), (u8, Utf8BmpHandle<'b, 'a>)> {
        let non_ascii_ret = {
            let dst_len = self.slice.len();
            let src_remaining = &source.slice[source.pos..];
            let dst_remaining = &mut self.slice[self.pos..];
            let (pending, length) = if dst_remaining.len() < src_remaining.len() {
                (DecoderResult::OutputFull, dst_remaining.len())
            } else {
                (DecoderResult::InputEmpty, src_remaining.len())
            };
            match ascii_to_ascii(src_remaining, dst_remaining) {
                None => {
                    source.pos += length;
                    self.pos += length;
                    return CopyAsciiResult::Stop((pending, source.pos, self.pos));
                }
                Some((non_ascii, consumed)) => {
                    source.pos += consumed;
                    self.pos += consumed;
                    if self.pos + 2 < dst_len {
                        source.pos += 1;
                        non_ascii
                    } else {
                        return CopyAsciiResult::Stop((
                            DecoderResult::OutputFull,
                            source.pos,
                            self.pos,
                        ));
                    }
                }
            }
        };
        CopyAsciiResult::GoOn((non_ascii_ret, Utf8BmpHandle::new(self)))
    }
    #[inline(always)]
    pub fn copy_ascii_from_check_space_astral<'b>(
        &'b mut self,
        source: &mut ByteSource,
    ) -> CopyAsciiResult<(DecoderResult, usize, usize), (u8, Utf8AstralHandle<'b, 'a>)> {
        let non_ascii_ret = {
            let dst_len = self.slice.len();
            let src_remaining = &source.slice[source.pos..];
            let dst_remaining = &mut self.slice[self.pos..];
            let (pending, length) = if dst_remaining.len() < src_remaining.len() {
                (DecoderResult::OutputFull, dst_remaining.len())
            } else {
                (DecoderResult::InputEmpty, src_remaining.len())
            };
            match ascii_to_ascii(src_remaining, dst_remaining) {
                None => {
                    source.pos += length;
                    self.pos += length;
                    return CopyAsciiResult::Stop((pending, source.pos, self.pos));
                }
                Some((non_ascii, consumed)) => {
                    source.pos += consumed;
                    self.pos += consumed;
                    if self.pos + 3 < dst_len {
                        source.pos += 1;
                        non_ascii
                    } else {
                        return CopyAsciiResult::Stop((
                            DecoderResult::OutputFull,
                            source.pos,
                            self.pos,
                        ));
                    }
                }
            }
        };
        CopyAsciiResult::GoOn((non_ascii_ret, Utf8AstralHandle::new(self)))
    }
    #[inline(always)]
    pub fn copy_utf8_up_to_invalid_from(&mut self, source: &mut ByteSource) {
        let src_remaining = &source.slice[source.pos..];
        let dst_remaining = &mut self.slice[self.pos..];
        let min_len = ::core::cmp::min(src_remaining.len(), dst_remaining.len());
        let valid_len = utf8_valid_up_to(&src_remaining[..min_len]);
        dst_remaining[..valid_len].copy_from_slice(&src_remaining[..valid_len]);
        source.pos += valid_len;
        self.pos += valid_len;
    }
    #[inline(always)]
    pub fn copy_utf16_from<E: Endian>(
        &mut self,
        source: &mut ByteSource,
    ) -> Option<(usize, usize)> {
        let src_remaining = &source.slice[source.pos..];
        let dst_remaining = &mut self.slice[self.pos..];
        let mut src_unaligned =
            unsafe { UnalignedU16Slice::new(src_remaining.as_ptr(), src_remaining.len() / 2) };
        if src_unaligned.len() == 0 {
            return None;
        }
        let mut last_unit = src_unaligned.at(src_unaligned.len() - 1);
        if E::OPPOSITE_ENDIAN {
            last_unit = last_unit.swap_bytes();
        }
        if super::in_range16(last_unit, 0xD800, 0xDC00) {
            src_unaligned.trim_last();
        }
        let (read, written, had_error) =
            convert_unaligned_utf16_to_utf8::<E>(src_unaligned, dst_remaining);
        source.pos += read * 2;
        self.pos += written;
        if had_error {
            Some((source.pos, self.pos))
        } else {
            None
        }
    }
}

pub enum Space<T> {
    Available(T),
    Full(usize),
}
pub enum CopyAsciiResult<T, U> {
    Stop(T),
    GoOn(U),
}
pub struct BigEndian;
pub struct LittleEndian;
#[derive(Debug, Copy, Clone)]
struct UnalignedU16Slice {
    ptr: *const u8,
    len: usize,
}
pub struct ByteSource<'a> {
    slice: &'a [u8],
    pos: usize,
}
pub struct ByteReadHandle<'a, 'b>
where
    'b: 'a,
{
    source: &'a mut ByteSource<'b>,
}
pub struct ByteUnreadHandle<'a, 'b>
where
    'b: 'a,
{
    source: &'a mut ByteSource<'b>,
}
pub struct Utf8BmpHandle<'a, 'b>
where
    'b: 'a,
{
    dest: &'a mut Utf8Destination<'b>,
}
pub struct Utf8AstralHandle<'a, 'b>
where
    'b: 'a,
{
    dest: &'a mut Utf8Destination<'b>,
}
pub struct Utf8Destination<'a> {
    slice: &'a mut [u8],
    pos: usize,
}

impl Endian for BigEndian {
    const OPPOSITE_ENDIAN: bool = cfg!(target_endian = "little");
}
impl Endian for LittleEndian {
    const OPPOSITE_ENDIAN: bool = cfg!(target_endian = "big");
}
