use crate::image_codecs::jpeg::errors::DecodeErrors;
use std::string::ToString;
pub const HUFF_LOOKAHEAD: u8 = 9;
pub struct HuffmanTable {
    pub(crate) maxcode: [i32; 18],
    pub(crate) offset: [i32; 18],
    pub(crate) lookup: [i32; 1 << HUFF_LOOKAHEAD],
    pub(crate) ac_lookup: Option<[i16; 1 << HUFF_LOOKAHEAD]>,
    pub(crate) values: [u8; 256],
}
impl HuffmanTable {
    pub fn new(
        codes: &[u8; 17],
        values: [u8; 256],
        is_dc: bool,
        is_progressive: bool,
    ) -> Result<HuffmanTable, DecodeErrors> {
        let too_long_code = (i32::from(HUFF_LOOKAHEAD) + 1) << HUFF_LOOKAHEAD;
        let mut p = HuffmanTable {
            maxcode: [0; 18],
            offset: [0; 18],
            lookup: [too_long_code; 1 << HUFF_LOOKAHEAD],
            values,
            ac_lookup: None,
        };
        p.make_derived_table(is_dc, is_progressive, codes)?;
        Ok(p)
    }
    pub fn new_unfilled(
        codes: &[u8; 17],
        values: &[u8],
        is_dc: bool,
        is_progressive: bool,
    ) -> Result<HuffmanTable, DecodeErrors> {
        let mut buf = [0; 256];
        buf[..values.len()].copy_from_slice(values);
        HuffmanTable::new(codes, buf, is_dc, is_progressive)
    }
    fn make_derived_table(
        &mut self,
        is_dc: bool,
        _is_progressive: bool,
        bits: &[u8; 17],
    ) -> Result<(), DecodeErrors> {
        let mut huff_size = [0; 257];
        let mut huff_code: [u32; 257] = [0; 257];
        let mut p = 0;
        for l in 1..=16 {
            let mut i = i32::from(bits[l]);
            while i != 0 {
                huff_size[p] = l as u8;
                p += 1;
                i -= 1;
            }
        }
        huff_size[p] = 0;
        let num_symbols = p;
        let mut code = 0;
        let mut si = i32::from(huff_size[0]);
        p = 0;
        while huff_size[p] != 0 {
            while i32::from(huff_size[p]) == si {
                huff_code[p] = code;
                code += 1;
                p += 1;
            }
            self.maxcode[si as usize] = (code << (16 - si)) as i32;
            if (code as i32) >= (1 << si) {
                return Err(DecodeErrors::HuffmanDecode("Bad Huffman Table".to_string()));
            }
            code <<= 1;
            si += 1;
        }
        p = 0;
        for l in 0..=16 {
            if bits[l] == 0 {
                self.maxcode[l] = -1;
            } else {
                self.offset[l] = (p as i32) - (huff_code[p]) as i32;
                p += usize::from(bits[l]);
            }
        }
        self.offset[17] = 0;
        self.maxcode[17] = 0x000F_FFFF;
        p = 0;
        for l in 1..=HUFF_LOOKAHEAD {
            for _ in 1..=i32::from(bits[usize::from(l)]) {
                let mut look_bits = (huff_code[p] as usize) << (HUFF_LOOKAHEAD - l);
                for _ in 0..1 << (HUFF_LOOKAHEAD - l) {
                    self.lookup[look_bits] =
                        (i32::from(l) << HUFF_LOOKAHEAD) | i32::from(self.values[p]);
                    look_bits += 1;
                }
                p += 1;
            }
        }
        if !is_dc {
            let mut fast = [255; 1 << HUFF_LOOKAHEAD];
            for i in 0..num_symbols {
                let s = huff_size[i];
                if s <= HUFF_LOOKAHEAD {
                    let c = (huff_code[i] << (HUFF_LOOKAHEAD - s)) as usize;
                    let m = (1 << (HUFF_LOOKAHEAD - s)) as usize;
                    for j in 0..m {
                        fast[c + j] = i as i16;
                    }
                }
            }
            let mut fast_ac = [0; 1 << HUFF_LOOKAHEAD];
            for i in 0..(1 << HUFF_LOOKAHEAD) {
                let fast_v = fast[i];
                if fast_v < 255 {
                    let rs = self.values[fast_v as usize];
                    let run = i16::from((rs >> 4) & 15);
                    let mag_bits = i16::from(rs & 15);
                    let len = i16::from(huff_size[fast_v as usize]);
                    if mag_bits != 0 && (len + mag_bits) <= i16::from(HUFF_LOOKAHEAD) {
                        let mut k = (((i as i16) << len) & ((1 << HUFF_LOOKAHEAD) - 1))
                            >> (i16::from(HUFF_LOOKAHEAD) - mag_bits);
                        let m = 1 << (mag_bits - 1);
                        if k < m {
                            k += (!0_i16 << mag_bits) + 1;
                        };
                        if (-128..=127).contains(&k) {
                            fast_ac[i] = (k << 8) + (run << 4) + (len + mag_bits);
                        }
                    }
                }
            }
            self.ac_lookup = Some(fast_ac);
        }
        if is_dc {
            for i in 0..num_symbols {
                let sym = self.values[i];
                if sym > 15 {
                    return Err(DecodeErrors::HuffmanDecode("Bad Huffman Table".to_string()));
                }
            }
        }
        Ok(())
    }
}
