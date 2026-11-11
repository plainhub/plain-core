use std::vec::Vec;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutOfData;
impl core::fmt::Display for OutOfData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("bit reader ran out of data")
    }
}
impl core::error::Error for OutOfData {}
#[derive(Debug, Clone)]
pub struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    stop_pos: usize,
}
impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        let stop_pos = data
            .iter()
            .enumerate()
            .rev()
            .find(|&(_, &b)| b != 0)
            .map_or(0, |(bi, &b)| bi * 8 + (7 - b.trailing_zeros() as usize));
        Self {
            data,
            pos: 0,
            stop_pos,
        }
    }
    pub fn data(&self) -> &'a [u8] {
        self.data
    }
    pub fn bit_pos(&self) -> usize {
        self.pos
    }
    pub fn bit_len(&self) -> usize {
        self.data.len() * 8
    }
    #[inline]
    pub fn more_rbsp_data(&self) -> bool {
        self.pos < self.stop_pos
    }
    pub fn is_byte_aligned(&self) -> bool {
        self.pos % 8 == 0
    }
    pub fn align_to_byte(&mut self) -> Result<(), OutOfData> {
        while self.pos % 8 != 0 {
            self.read_bit()?;
        }
        Ok(())
    }
    pub fn read_bit(&mut self) -> Result<bool, OutOfData> {
        let Some(&byte) = self.data.get(self.pos / 8) else {
            return Err(OutOfData);
        };
        let bit = (byte >> (7 - (self.pos % 8))) & 1;
        self.pos += 1;
        Ok(bit == 1)
    }
    pub fn read_bits(&mut self, n: u32) -> Result<u32, OutOfData> {
        if n > 32 {
            return Err(OutOfData);
        }
        if n == 0 {
            return Ok(0);
        }
        if n <= 24 {
            let v = self.peek_bits(n);
            self.skip_bits(n)?;
            return Ok(v);
        }
        let hi = self.read_bits(n - 16)?;
        let lo = self.read_bits(16)?;
        Ok((hi << 16) | lo)
    }
    #[inline]
    pub fn peek_bits(&self, n: u32) -> u32 {
        debug_assert!(n <= 24);
        let byte = self.pos / 8;
        let off = (self.pos % 8) as u32;
        let acc = match self.data.get(byte..byte + 4) {
            Some(c) => u32::from_be_bytes([c[0], c[1], c[2], c[3]]),
            None => {
                ((*self.data.get(byte).unwrap_or(&0) as u32) << 24)
                    | ((*self.data.get(byte + 1).unwrap_or(&0) as u32) << 16)
                    | ((*self.data.get(byte + 2).unwrap_or(&0) as u32) << 8)
                    | (*self.data.get(byte + 3).unwrap_or(&0) as u32)
            }
        };
        (acc >> (32 - off - n)) & ((1u32 << n) - 1)
    }
    #[inline]
    pub fn skip_bits(&mut self, n: u32) -> Result<(), OutOfData> {
        if self.pos + n as usize > self.bit_len() {
            return Err(OutOfData);
        }
        self.pos += n as usize;
        Ok(())
    }
    pub fn read_ue(&mut self) -> Result<u32, OutOfData> {
        let window = self.peek_bits(24);
        let lz = window.leading_zeros() - 8;
        if lz <= 11 {
            let total = 2 * lz + 1;
            self.skip_bits(total)?;
            if lz == 0 {
                return Ok(0);
            }
            let info = (window >> (24 - total)) & ((1u32 << lz) - 1);
            return Ok((1u32 << lz) - 1 + info);
        }
        let mut leading_zeros = 0u32;
        while !self.read_bit()? {
            leading_zeros += 1;
            if leading_zeros >= 32 {
                return Err(OutOfData);
            }
        }
        if leading_zeros == 0 {
            return Ok(0);
        }
        let info = self.read_bits(leading_zeros)?;
        Ok((1u32 << leading_zeros) - 1 + info)
    }
    pub fn read_se(&mut self) -> Result<i32, OutOfData> {
        let code_num = self.read_ue()?;
        let magnitude = code_num.div_ceil(2) as i32;
        Ok(if code_num % 2 == 1 {
            magnitude
        } else {
            -magnitude
        })
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}
impl<'a> BitReader<'a> {
    #[inline(always)]
    pub fn cursor(&self) -> Cursor<'a> {
        Cursor {
            data: self.data,
            pos: self.pos,
        }
    }
    #[inline(always)]
    pub fn commit(&mut self, c: Cursor<'a>) {
        debug_assert!(core::ptr::eq(self.data, c.data));
        self.pos = c.pos;
    }
}
#[cold]
#[inline(never)]
fn window_tail(data: &[u8], byte: usize) -> u32 {
    ((*data.get(byte).unwrap_or(&0) as u32) << 24)
        | ((*data.get(byte + 1).unwrap_or(&0) as u32) << 16)
        | ((*data.get(byte + 2).unwrap_or(&0) as u32) << 8)
        | (*data.get(byte + 3).unwrap_or(&0) as u32)
}
impl<'a> Cursor<'a> {
    #[inline(always)]
    pub fn from_parts(data: &'a [u8], pos: usize) -> Self {
        Cursor { data, pos }
    }
    #[inline(always)]
    pub fn into_parts(self) -> (&'a [u8], usize) {
        (self.data, self.pos)
    }
    #[inline(always)]
    pub fn bit_pos(&self) -> usize {
        self.pos
    }
    #[inline(always)]
    pub fn peek24(&self) -> u32 {
        let byte = self.pos >> 3;
        let off = (self.pos & 7) as u32;
        let acc = match self.data.get(byte..byte + 4) {
            Some(c) => u32::from_be_bytes([c[0], c[1], c[2], c[3]]),
            None => window_tail(self.data, byte),
        };
        (acc >> (8 - off)) & 0x00FF_FFFF
    }
    #[inline(always)]
    pub fn peek(&self, n: u32) -> u32 {
        debug_assert!(n <= 24);
        self.peek24() >> (24 - n)
    }
    #[inline(always)]
    pub fn skip(&mut self, n: u32) -> Result<(), OutOfData> {
        let np = self.pos + n as usize;
        if np > self.data.len() * 8 {
            return Err(OutOfData);
        }
        self.pos = np;
        Ok(())
    }
    #[inline(always)]
    pub fn read_bit(&mut self) -> Result<bool, OutOfData> {
        let Some(&byte) = self.data.get(self.pos >> 3) else {
            return Err(OutOfData);
        };
        let bit = (byte >> (7 - (self.pos & 7))) & 1;
        self.pos += 1;
        Ok(bit == 1)
    }
    #[inline(always)]
    pub fn read_bits(&mut self, n: u32) -> Result<u32, OutOfData> {
        if n <= 24 {
            let v = self.peek(n);
            self.skip(n)?;
            return Ok(v);
        }
        let (v, c) = self.read_bits_long(n)?;
        *self = c;
        Ok(v)
    }
    #[cold]
    #[inline(never)]
    fn read_bits_long(mut self, n: u32) -> Result<(u32, Self), OutOfData> {
        if n > 32 {
            return Err(OutOfData);
        }
        let hi = self.read_bits(n - 16)?;
        let lo = self.read_bits(16)?;
        Ok(((hi << 16) | lo, self))
    }
}
