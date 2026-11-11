use crate::image_codecs::jpeg::errors::DecodeErrors;
use crate::image_codecs::jpeg::huffman::{HUFF_LOOKAHEAD, HuffmanTable};
use crate::image_codecs::jpeg::marker::Marker;
use crate::image_codecs::jpeg::mcu::DCT_BLOCK;
use crate::image_codecs::jpeg::misc::UN_ZIGZAG;
use crate::image_codecs::zune::bytestream::{ZByteReaderTrait, ZReader};
use crate::image_codecs::zune::log::warn;
use core::cmp::min;
use std::format;
use std::string::ToString;
macro_rules! decode_huff {
    ($ stream : tt ,$ symbol : tt ,$ table : tt) => {
        let mut code_length = $symbol >> HUFF_LOOKAHEAD;
        ($symbol) &= (1 << HUFF_LOOKAHEAD) - 1;
        if code_length > i32::from(HUFF_LOOKAHEAD) {
            $symbol = ($stream).peek_bits::<16>() as i32;
            while code_length < 17 {
                if $symbol < $table.maxcode[code_length as usize] {
                    break;
                }
                code_length += 1;
            }
            if code_length == 17 {
                return Err(DecodeErrors::Format(format!(
                    "Bad Huffman Code 0x{:X}, corrupt JPEG",
                    $symbol
                )));
            }
            $symbol >>= (16 - code_length);
            ($symbol) = i32::from(
                ($table).values
                    [(($symbol + ($table).offset[code_length as usize]) & 0xFF) as usize],
            );
        }
        if code_length > i32::from(($stream).bits_left) {
            return Err(DecodeErrors::Format(format!(
                "Code length {code_length}  more than bits left {}",
                ($stream).bits_left
            )));
        }
        ($stream).drop_bits(code_length as u8);
    };
}
# [rustfmt :: skip] pub (crate) struct BitStream { pub buffer : u64 , aligned_buffer : u64 , pub (crate) bits_left : u8 , pub marker : Option < Marker > , pub successive_low_mask : i16 , spec_start : u8 , spec_end : u8 , pub eob_run : i32 , pub overread_by : usize , pub seen_eoi : bool }
impl BitStream {
    # [rustfmt :: skip]    pub (crate) const fn new () -> BitStream { BitStream { buffer : 0 , aligned_buffer : 0 , bits_left : 0 , marker : None , successive_low_mask : 1 , spec_start : 0 , spec_end : 0 , eob_run : 0 , overread_by : 0 , seen_eoi : false } }    # [rustfmt :: skip]    pub (crate) fn new_progressive (al : u8 , spec_start : u8 , spec_end : u8) -> BitStream { BitStream { buffer : 0 , aligned_buffer : 0 , bits_left : 0 , marker : None , successive_low_mask : 1i16 << al , spec_start : spec_start , spec_end : spec_end , eob_run : 0 , overread_by : 0 , seen_eoi : false } }
    #[inline(always)]
    pub fn refill<T>(&mut self, reader: &mut ZReader<T>) -> Result<bool, DecodeErrors>
    where
        T: ZByteReaderTrait,
    {
        macro_rules! refill {
            ($ buffer : expr ,$ byte : expr ,$ bits_left : expr) => {
                $byte = u64::from(reader.read_u8());
                self.overread_by += usize::from(reader.eof()?);
                $buffer = ($buffer << 8) | $byte;
                $bits_left += 8;
                if $byte == 0xff {
                    let mut next_byte = u64::from(reader.read_u8());
                    if next_byte != 0x00 {
                        while next_byte == 0xFF {
                            next_byte = u64::from(reader.read_u8());
                        }
                        if next_byte != 0x00 {
                            $buffer >>= 8;
                            $bits_left -= 8;
                            if $bits_left != 0 {
                                self.aligned_buffer = $buffer << (64 - $bits_left);
                            }
                            let marker = Marker::from_u8(next_byte as u8);
                            self.marker = marker;
                            if let Some(Marker::UNKNOWN(_)) = marker {
                                return Err(DecodeErrors::Format(
                                    "Unknown marker in bit stream".to_string(),
                                ));
                            }
                            if next_byte == 0xD9 {
                                self.buffer <<= 8;
                                self.bits_left += 8;
                                self.aligned_buffer = self.buffer << (64 - self.bits_left);
                            }
                            return Ok(false);
                        }
                    }
                }
            };
        }
        if self.bits_left < 32 {
            if self.marker.is_some() || self.seen_eoi {
                self.buffer <<= 32;
                self.bits_left += 32;
                self.aligned_buffer = self.buffer << (64 - self.bits_left);
                return Ok(true);
            }
            if self.overread_by > 0 {
                if self.bits_left == 0 {
                    return Err(DecodeErrors::ExhaustedData);
                }
                return Ok(true);
            }
            if let Ok(bytes) = reader.read_fixed_bytes_or_error::<4>() {
                let msb_buf = u32::from_be_bytes(bytes);
                if !has_byte(msb_buf, 255) {
                    self.bits_left += 32;
                    self.buffer <<= 32;
                    self.buffer |= u64::from(msb_buf);
                    self.aligned_buffer = self.buffer << (64 - self.bits_left);
                    return Ok(true);
                }
                reader.rewind(4)?;
            }
            let mut byte;
            refill!(self.buffer, byte, self.bits_left);
            refill!(self.buffer, byte, self.bits_left);
            refill!(self.buffer, byte, self.bits_left);
            refill!(self.buffer, byte, self.bits_left);
            self.aligned_buffer = self.buffer << (64 - self.bits_left);
        }
        return Ok(true);
    }
    #[inline(always)]
    fn decode_dc<T>(
        &mut self,
        reader: &mut ZReader<T>,
        dc_table: &HuffmanTable,
        dc_prediction: &mut i32,
    ) -> Result<bool, DecodeErrors>
    where
        T: ZByteReaderTrait,
    {
        let (mut symbol, r);
        if self.bits_left < 32 {
            self.refill(reader)?;
        };
        symbol = self.peek_bits::<HUFF_LOOKAHEAD>();
        symbol = dc_table.lookup[symbol as usize];
        decode_huff!(self, symbol, dc_table);
        if symbol != 0 {
            r = self.get_bits(symbol as u8);
            symbol = huff_extend(r, symbol);
        }
        *dc_prediction = dc_prediction.wrapping_add(symbol);
        return Ok(true);
    }
    fn discard_dc<T>(
        &mut self,
        reader: &mut ZReader<T>,
        dc_table: &HuffmanTable,
    ) -> Result<bool, DecodeErrors>
    where
        T: ZByteReaderTrait,
    {
        let mut symbol;
        if self.bits_left < 32 {
            self.refill(reader)?;
        };
        symbol = self.peek_bits::<HUFF_LOOKAHEAD>();
        symbol = dc_table.lookup[symbol as usize];
        decode_huff!(self, symbol, dc_table);
        if symbol != 0 {
            let _ = self.get_bits(symbol as u8);
        }
        return Ok(true);
    }
    #[inline(never)]
    pub fn decode_mcu_block<T>(
        &mut self,
        reader: &mut ZReader<T>,
        dc_table: &HuffmanTable,
        ac_table: &HuffmanTable,
        qt_table: &[i32; DCT_BLOCK],
        block: &mut [i32; 64],
        dc_prediction: &mut i32,
    ) -> Result<u16, DecodeErrors>
    where
        T: ZByteReaderTrait,
    {
        let ac_lookup = ac_table.ac_lookup.as_ref().unwrap();
        let (mut symbol, mut r, mut fast_ac);
        let mut pos: usize = 1;
        if self.bits_left < 1 && self.marker.is_some() {
            return Err(DecodeErrors::Format(
                "No more bytes left in stream before marker".to_string(),
            ));
        }
        self.decode_dc(reader, dc_table, dc_prediction)?;
        block[0] = *dc_prediction * qt_table[0];
        while pos < 64 {
            self.refill(reader)?;
            symbol = self.peek_bits::<HUFF_LOOKAHEAD>();
            fast_ac = ac_lookup[symbol as usize];
            symbol = ac_table.lookup[symbol as usize];
            if fast_ac != 0 {
                pos += ((fast_ac >> 4) & 15) as usize;
                let t_pos = UN_ZIGZAG[min(pos, 63)] & 63;
                block[t_pos] = i32::from(fast_ac >> 8) * (qt_table[t_pos]);
                self.drop_bits((fast_ac & 15) as u8);
                pos += 1;
            } else {
                decode_huff!(self, symbol, ac_table);
                r = symbol >> 4;
                symbol &= 15;
                if symbol != 0 {
                    pos += r as usize;
                    r = self.get_bits(symbol as u8);
                    symbol = huff_extend(r, symbol);
                    let t_pos = UN_ZIGZAG[pos & 63] & 63;
                    block[t_pos] = symbol * qt_table[t_pos];
                    pos += 1;
                } else if r != 15 {
                    return Ok(pos as u16);
                } else {
                    pos += 16;
                }
            }
        }
        return Ok(64);
    }
    pub fn discard_mcu_block<T>(
        &mut self,
        reader: &mut ZReader<T>,
        dc_table: &HuffmanTable,
        ac_table: &HuffmanTable,
    ) -> Result<u16, DecodeErrors>
    where
        T: ZByteReaderTrait,
    {
        let ac_lookup = ac_table.ac_lookup.as_ref().unwrap();
        let (mut symbol, mut r, mut fast_ac);
        let mut pos: usize = 1;
        self.discard_dc(reader, dc_table)?;
        while pos < 64 {
            self.refill(reader)?;
            symbol = self.peek_bits::<HUFF_LOOKAHEAD>();
            fast_ac = ac_lookup[symbol as usize];
            symbol = ac_table.lookup[symbol as usize];
            if fast_ac != 0 {
                pos += ((fast_ac >> 4) & 15) as usize;
                self.drop_bits((fast_ac & 15) as u8);
                pos += 1;
            } else {
                decode_huff!(self, symbol, ac_table);
                r = symbol >> 4;
                symbol &= 15;
                if symbol != 0 {
                    pos += r as usize;
                    let _ = self.get_bits(symbol as u8);
                    pos += 1;
                } else if r != 15 {
                    return Ok(pos as u16);
                } else {
                    pos += 16;
                }
            }
        }
        return Ok(64);
    }
    #[inline(always)]
    const fn peek_bits<const LOOKAHEAD: u8>(&self) -> i32 {
        (self.aligned_buffer >> (64 - LOOKAHEAD)) as i32
    }
    #[inline]
    fn drop_bits(&mut self, n: u8) {
        self.bits_left = self.bits_left.saturating_sub(n);
        self.aligned_buffer <<= n;
    }
    #[inline(always)]
    fn get_bits(&mut self, n_bits: u8) -> i32 {
        let mask = (1_u64 << n_bits) - 1;
        self.aligned_buffer = self.aligned_buffer.rotate_left(u32::from(n_bits));
        let bits = (self.aligned_buffer & mask) as i32;
        self.bits_left = self.bits_left.wrapping_sub(n_bits);
        bits
    }
    #[inline]
    pub(crate) fn decode_prog_dc_first<T>(
        &mut self,
        reader: &mut ZReader<T>,
        dc_table: &HuffmanTable,
        block: &mut i16,
        dc_prediction: &mut i32,
    ) -> Result<(), DecodeErrors>
    where
        T: ZByteReaderTrait,
    {
        self.decode_dc(reader, dc_table, dc_prediction)?;
        *block = (*dc_prediction as i16).wrapping_mul(self.successive_low_mask);
        return Ok(());
    }
    #[inline]
    pub(crate) fn decode_prog_dc_refine<T>(
        &mut self,
        reader: &mut ZReader<T>,
        block: &mut i16,
    ) -> Result<(), DecodeErrors>
    where
        T: ZByteReaderTrait,
    {
        if self.bits_left < 1 {
            self.refill(reader)?;
            if self.bits_left < 1 {
                return Err(DecodeErrors::Format(
                    "Marker found where not expected in refine bit".to_string(),
                ));
            }
        }
        if self.get_bit() == 1 {
            *block = block.wrapping_add(self.successive_low_mask);
        }
        Ok(())
    }
    fn get_bit(&mut self) -> u8 {
        let k = (self.aligned_buffer >> 63) as u8;
        self.drop_bits(1);
        return k;
    }
    pub(crate) fn decode_mcu_ac_first<T>(
        &mut self,
        reader: &mut ZReader<T>,
        ac_table: &HuffmanTable,
        block: &mut [i16; 64],
    ) -> Result<bool, DecodeErrors>
    where
        T: ZByteReaderTrait,
    {
        let fast_ac = ac_table.ac_lookup.as_ref().unwrap();
        let bit = self.successive_low_mask;
        let mut k = self.spec_start as usize;
        let (mut symbol, mut r, mut fac);
        'block: loop {
            self.refill(reader)?;
            symbol = self.peek_bits::<HUFF_LOOKAHEAD>();
            fac = fast_ac[symbol as usize];
            symbol = ac_table.lookup[symbol as usize];
            if fac != 0 {
                k += ((fac >> 4) & 15) as usize;
                block[UN_ZIGZAG[min(k, 63)] & 63] = (fac >> 8).wrapping_mul(bit);
                self.drop_bits((fac & 15) as u8);
                k += 1;
            } else {
                decode_huff!(self, symbol, ac_table);
                r = symbol >> 4;
                symbol &= 15;
                if symbol != 0 {
                    k += r as usize;
                    r = self.get_bits(symbol as u8);
                    symbol = huff_extend(r, symbol);
                    block[UN_ZIGZAG[k & 63] & 63] = (symbol as i16).wrapping_mul(bit);
                    k += 1;
                } else {
                    if r != 15 {
                        self.eob_run = 1 << r;
                        self.eob_run += self.get_bits(r as u8);
                        self.eob_run -= 1;
                        break;
                    }
                    k += 16;
                }
            }
            if k > self.spec_end as usize {
                break 'block;
            }
        }
        return Ok(true);
    }
    pub(crate) fn decode_mcu_ac_refine<T>(
        &mut self,
        reader: &mut ZReader<T>,
        table: &HuffmanTable,
        block: &mut [i16; 64],
    ) -> Result<bool, DecodeErrors>
    where
        T: ZByteReaderTrait,
    {
        let bit = self.successive_low_mask;
        let mut k = self.spec_start;
        let (mut symbol, mut r);
        if self.eob_run == 0 {
            'no_eob: loop {
                self.refill(reader)?;
                symbol = self.peek_bits::<HUFF_LOOKAHEAD>();
                symbol = table.lookup[symbol as usize];
                decode_huff!(self, symbol, table);
                r = symbol >> 4;
                symbol &= 15;
                if symbol == 0 {
                    if r != 15 {
                        self.eob_run = 1 << r;
                        self.eob_run += self.get_bits(r as u8);
                        break 'no_eob;
                    }
                } else {
                    if symbol != 1 {
                        warn!("Bad Huffman code, corrupt JPEG?");
                    }
                    if self.get_bit() == 1 {
                        symbol = i32::from(bit);
                    } else {
                        symbol = i32::from(-bit);
                    }
                }
                if k <= self.spec_end {
                    'advance_nonzero: loop {
                        let coefficient = &mut block[UN_ZIGZAG[k as usize & 63] & 63];
                        if *coefficient != 0 {
                            if self.bits_left < 1 {
                                self.refill(reader)?;
                                if self.bits_left < 1 && self.marker.is_some() {
                                    return Err(DecodeErrors::Format(
                                        "Marker found where not expected in refine bit".to_string(),
                                    ));
                                }
                            }
                            if self.get_bit() == 1 && (*coefficient & bit) == 0 {
                                if *coefficient > 0 {
                                    *coefficient = coefficient.wrapping_add(bit);
                                } else {
                                    *coefficient = coefficient.wrapping_sub(bit);
                                }
                            }
                        } else {
                            r -= 1;
                            if r < 0 {
                                break 'advance_nonzero;
                            }
                        };
                        if k == self.spec_end {
                            break 'advance_nonzero;
                        }
                        k += 1;
                    }
                }
                if symbol != 0 {
                    let pos = UN_ZIGZAG[k as usize & 63];
                    block[pos & 63] = symbol as i16;
                }
                k += 1;
                if k > self.spec_end {
                    break 'no_eob;
                }
            }
        }
        if self.eob_run > 0 {
            if &block[1..] != &[0; 63] {
                self.refill(reader)?;
                while k <= self.spec_end {
                    let coefficient = &mut block[UN_ZIGZAG[k as usize & 63] & 63];
                    if *coefficient != 0 && self.get_bit() == 1 {
                        if (*coefficient & bit) == 0 {
                            if *coefficient >= 0 {
                                *coefficient = coefficient.wrapping_add(bit);
                            } else {
                                *coefficient = coefficient.wrapping_sub(bit);
                            }
                        }
                    }
                    if self.bits_left < 1 {
                        self.refill(reader)?;
                    }
                    k += 1;
                }
            }
            self.eob_run -= 1;
        }
        return Ok(true);
    }
    pub fn update_progressive_params(&mut self, _ah: u8, al: u8, spec_start: u8, spec_end: u8) {
        self.successive_low_mask = 1i16 << al;
        self.spec_start = spec_start;
        self.spec_end = spec_end;
    }
    #[cold]
    pub fn reset(&mut self) {
        self.bits_left = 0;
        self.marker = None;
        self.buffer = 0;
        self.aligned_buffer = 0;
        self.eob_run = 0;
    }
}
#[inline(always)]
fn huff_extend(x: i32, s: i32) -> i32 {
    (x) + ((((x) - (1 << ((s) - 1))) >> 31) & (((-1) << (s)) + 1))
}
const fn has_zero(v: u32) -> bool {
    return !((((v & 0x7F7F_7F7F) + 0x7F7F_7F7F) | v) | 0x7F7F_7F7F) != 0;
}
const fn has_byte(b: u32, val: u8) -> bool {
    has_zero(b ^ ((!0_u32 / 255) * (val as u32)))
}
