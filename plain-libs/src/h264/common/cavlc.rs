use crate::h264::common::BitReader;
use crate::h264::common::bit_reader::{Cursor, OutOfData};
use std::vec;
use std::vec::Vec;

#[inline]
pub fn un_scan_4x4_dcac(s: &[i32; 16]) -> [i32; 16] {
    let mut d = [0i32; 16];
    d[0] = s[0];
    d[1] = s[1];
    d[4] = s[2];
    d[8] = s[3];
    d[5] = s[4];
    d[2] = s[5];
    d[3] = s[6];
    d[6] = s[7];
    d[9] = s[8];
    d[12] = s[9];
    d[13] = s[10];
    d[10] = s[11];
    d[7] = s[12];
    d[11] = s[13];
    d[14] = s[14];
    d[15] = s[15];
    d
}
#[inline]
pub fn un_scan_4x4_ac_into(s: &[i32], d: &mut [i32; 16]) {
    d[1] = s[0];
    d[4] = s[1];
    d[8] = s[2];
    d[5] = s[3];
    d[2] = s[4];
    d[3] = s[5];
    d[6] = s[6];
    d[9] = s[7];
    d[12] = s[8];
    d[13] = s[9];
    d[10] = s[10];
    d[7] = s[11];
    d[11] = s[12];
    d[14] = s[13];
    d[15] = s[14];
}
# [rustfmt :: skip] const CBP_INTRA : [u8 ; 48] = [47 , 31 , 15 , 0 , 23 , 27 , 29 , 30 , 7 , 11 , 13 , 14 , 39 , 43 , 45 , 46 , 16 , 3 , 5 , 10 , 12 , 19 , 21 , 26 , 28 , 35 , 37 , 42 , 44 , 1 , 2 , 4 , 8 , 17 , 18 , 20 , 24 , 6 , 9 , 22 , 25 , 32 , 33 , 34 , 36 , 40 , 38 , 41 ,] ;
# [rustfmt :: skip] const CBP_INTER : [u8 ; 48] = [0 , 16 , 1 , 2 , 4 , 8 , 32 , 3 , 5 , 10 , 12 , 15 , 47 , 7 , 11 , 13 , 14 , 6 , 9 , 31 , 35 , 37 , 42 , 44 , 33 , 34 , 36 , 40 , 39 , 43 , 45 , 46 , 17 , 18 , 20 , 24 , 19 , 21 , 26 , 28 , 23 , 27 , 29 , 30 , 22 , 25 , 38 , 41 ,] ;
const fn invert_cbp(table: &[u8; 48]) -> [u8; 48] {
    let mut inv = [0u8; 48];
    let mut i = 0;
    while i < 48 {
        inv[table[i] as usize] = i as u8;
        i += 1;
    }
    inv
}
const INV_CBP_INTRA: [u8; 48] = invert_cbp(&CBP_INTRA);
const INV_CBP_INTER: [u8; 48] = invert_cbp(&CBP_INTER);
pub fn read_cbp_intra(r: &mut BitReader) -> Result<u32, OutOfData> {
    let code_num = r.read_ue()? as usize;
    CBP_INTRA.get(code_num).map(|&v| v as u32).ok_or(OutOfData)
}
pub fn read_cbp_inter(r: &mut BitReader) -> Result<u32, OutOfData> {
    let code_num = r.read_ue()? as usize;
    CBP_INTER.get(code_num).map(|&v| v as u32).ok_or(OutOfData)
}
# [rustfmt :: skip] const COEFF_TOKEN_LEN : [[u8 ; 68] ; 4] = [[1 , 0 , 0 , 0 , 6 , 2 , 0 , 0 , 8 , 6 , 3 , 0 , 9 , 8 , 7 , 5 , 10 , 9 , 8 , 6 , 11 , 10 , 9 , 7 , 13 , 11 , 10 , 8 , 13 , 13 , 11 , 9 , 13 , 13 , 13 , 10 , 14 , 14 , 13 , 11 , 14 , 14 , 14 , 13 , 15 , 15 , 14 , 14 , 15 , 15 , 15 , 14 , 16 , 15 , 15 , 15 , 16 , 16 , 16 , 15 , 16 , 16 , 16 , 16 , 16 , 16 , 16 , 16 ,] , [2 , 0 , 0 , 0 , 6 , 2 , 0 , 0 , 6 , 5 , 3 , 0 , 7 , 6 , 6 , 4 , 8 , 6 , 6 , 4 , 8 , 7 , 7 , 5 , 9 , 8 , 8 , 6 , 11 , 9 , 9 , 6 , 11 , 11 , 11 , 7 , 12 , 11 , 11 , 9 , 12 , 12 , 12 , 11 , 12 , 12 , 12 , 11 , 13 , 13 , 13 , 12 , 13 , 13 , 13 , 13 , 13 , 14 , 13 , 13 , 14 , 14 , 14 , 13 , 14 , 14 , 14 , 14 ,] , [4 , 0 , 0 , 0 , 6 , 4 , 0 , 0 , 6 , 5 , 4 , 0 , 6 , 5 , 5 , 4 , 7 , 5 , 5 , 4 , 7 , 5 , 5 , 4 , 7 , 6 , 6 , 4 , 7 , 6 , 6 , 4 , 8 , 7 , 7 , 5 , 8 , 8 , 7 , 6 , 9 , 8 , 8 , 7 , 9 , 9 , 8 , 8 , 9 , 9 , 9 , 8 , 10 , 9 , 9 , 9 , 10 , 10 , 10 , 10 , 10 , 10 , 10 , 10 , 10 , 10 , 10 , 10 ,] , [6 , 0 , 0 , 0 , 6 , 6 , 0 , 0 , 6 , 6 , 6 , 0 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 , 6 ,] ,] ;
# [rustfmt :: skip] const COEFF_TOKEN_BITS : [[u8 ; 68] ; 4] = [[1 , 0 , 0 , 0 , 5 , 1 , 0 , 0 , 7 , 4 , 1 , 0 , 7 , 6 , 5 , 3 , 7 , 6 , 5 , 3 , 7 , 6 , 5 , 4 , 15 , 6 , 5 , 4 , 11 , 14 , 5 , 4 , 8 , 10 , 13 , 4 , 15 , 14 , 9 , 4 , 11 , 10 , 13 , 12 , 15 , 14 , 9 , 12 , 11 , 10 , 13 , 8 , 15 , 1 , 9 , 12 , 11 , 14 , 13 , 8 , 7 , 10 , 9 , 12 , 4 , 6 , 5 , 8 ,] , [3 , 0 , 0 , 0 , 11 , 2 , 0 , 0 , 7 , 7 , 3 , 0 , 7 , 10 , 9 , 5 , 7 , 6 , 5 , 4 , 4 , 6 , 5 , 6 , 7 , 6 , 5 , 8 , 15 , 6 , 5 , 4 , 11 , 14 , 13 , 4 , 15 , 10 , 9 , 4 , 11 , 14 , 13 , 12 , 8 , 10 , 9 , 8 , 15 , 14 , 13 , 12 , 11 , 10 , 9 , 12 , 7 , 11 , 6 , 8 , 9 , 8 , 10 , 1 , 7 , 6 , 5 , 4 ,] , [15 , 0 , 0 , 0 , 15 , 14 , 0 , 0 , 11 , 15 , 13 , 0 , 8 , 12 , 14 , 12 , 15 , 10 , 11 , 11 , 11 , 8 , 9 , 10 , 9 , 14 , 13 , 9 , 8 , 10 , 9 , 8 , 15 , 14 , 13 , 13 , 11 , 14 , 10 , 12 , 15 , 10 , 13 , 12 , 11 , 14 , 9 , 12 , 8 , 10 , 13 , 8 , 13 , 7 , 9 , 12 , 9 , 12 , 11 , 10 , 5 , 8 , 7 , 6 , 1 , 4 , 3 , 2 ,] , [3 , 0 , 0 , 0 , 0 , 1 , 0 , 0 , 4 , 5 , 6 , 0 , 8 , 9 , 10 , 11 , 12 , 13 , 14 , 15 , 16 , 17 , 18 , 19 , 20 , 21 , 22 , 23 , 24 , 25 , 26 , 27 , 28 , 29 , 30 , 31 , 32 , 33 , 34 , 35 , 36 , 37 , 38 , 39 , 40 , 41 , 42 , 43 , 44 , 45 , 46 , 47 , 48 , 49 , 50 , 51 , 52 , 53 , 54 , 55 , 56 , 57 , 58 , 59 , 60 , 61 , 62 , 63 ,] ,] ;
# [rustfmt :: skip] const CHROMA_DC_COEFF_TOKEN_LEN : [u8 ; 20] = [2 , 0 , 0 , 0 , 6 , 1 , 0 , 0 , 6 , 6 , 3 , 0 , 6 , 7 , 7 , 6 , 6 , 8 , 8 , 7 ,] ;
# [rustfmt :: skip] const CHROMA_DC_COEFF_TOKEN_BITS : [u8 ; 20] = [1 , 0 , 0 , 0 , 7 , 1 , 0 , 0 , 4 , 6 , 1 , 0 , 3 , 3 , 2 , 5 , 2 , 3 , 2 , 0 ,] ;
# [rustfmt :: skip] const TOTAL_ZEROS_LEN : [[u8 ; 16] ; 15] = [[1 , 3 , 3 , 4 , 4 , 5 , 5 , 6 , 6 , 7 , 7 , 8 , 8 , 9 , 9 , 9] , [3 , 3 , 3 , 3 , 3 , 4 , 4 , 4 , 4 , 5 , 5 , 6 , 6 , 6 , 6 , 0] , [4 , 3 , 3 , 3 , 4 , 4 , 3 , 3 , 4 , 5 , 5 , 6 , 5 , 6 , 0 , 0] , [5 , 3 , 4 , 4 , 3 , 3 , 3 , 4 , 3 , 4 , 5 , 5 , 5 , 0 , 0 , 0] , [4 , 4 , 4 , 3 , 3 , 3 , 3 , 3 , 4 , 5 , 4 , 5 , 0 , 0 , 0 , 0] , [6 , 5 , 3 , 3 , 3 , 3 , 3 , 3 , 4 , 3 , 6 , 0 , 0 , 0 , 0 , 0] , [6 , 5 , 3 , 3 , 3 , 2 , 3 , 4 , 3 , 6 , 0 , 0 , 0 , 0 , 0 , 0] , [6 , 4 , 5 , 3 , 2 , 2 , 3 , 3 , 6 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [6 , 6 , 4 , 2 , 2 , 3 , 2 , 5 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [5 , 5 , 3 , 2 , 2 , 2 , 4 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [4 , 4 , 3 , 3 , 1 , 3 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [4 , 4 , 2 , 1 , 3 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [3 , 3 , 1 , 2 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [2 , 2 , 1 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [1 , 1 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] ,] ;
# [rustfmt :: skip] const TOTAL_ZEROS_BITS : [[u8 ; 16] ; 15] = [[1 , 3 , 2 , 3 , 2 , 3 , 2 , 3 , 2 , 3 , 2 , 3 , 2 , 3 , 2 , 1] , [7 , 6 , 5 , 4 , 3 , 5 , 4 , 3 , 2 , 3 , 2 , 3 , 2 , 1 , 0 , 0] , [5 , 7 , 6 , 5 , 4 , 3 , 4 , 3 , 2 , 3 , 2 , 1 , 1 , 0 , 0 , 0] , [3 , 7 , 5 , 4 , 6 , 5 , 4 , 3 , 3 , 2 , 2 , 1 , 0 , 0 , 0 , 0] , [5 , 4 , 3 , 7 , 6 , 5 , 4 , 3 , 2 , 1 , 1 , 0 , 0 , 0 , 0 , 0] , [1 , 1 , 7 , 6 , 5 , 4 , 3 , 2 , 1 , 1 , 0 , 0 , 0 , 0 , 0 , 0] , [1 , 1 , 5 , 4 , 3 , 3 , 2 , 1 , 1 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [1 , 1 , 1 , 3 , 3 , 2 , 2 , 1 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [1 , 0 , 1 , 3 , 2 , 1 , 1 , 1 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [1 , 0 , 1 , 3 , 2 , 1 , 1 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [0 , 1 , 1 , 2 , 1 , 3 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [0 , 1 , 1 , 1 , 1 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [0 , 1 , 1 , 1 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [0 , 1 , 1 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [0 , 1 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] ,] ;
# [rustfmt :: skip] const CHROMA_DC_TOTAL_ZEROS_LEN : [[u8 ; 4] ; 3] = [[1 , 2 , 3 , 3] , [1 , 2 , 2 , 0] , [1 , 1 , 0 , 0] ,] ;
# [rustfmt :: skip] const CHROMA_DC_TOTAL_ZEROS_BITS : [[u8 ; 4] ; 3] = [[1 , 1 , 1 , 0] , [1 , 1 , 0 , 0] , [1 , 0 , 0 , 0] ,] ;
# [rustfmt :: skip] const RUN_LEN : [[u8 ; 15] ; 7] = [[1 , 1 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [1 , 2 , 2 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [2 , 2 , 2 , 2 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [2 , 2 , 2 , 3 , 3 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [2 , 2 , 3 , 3 , 3 , 3 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [2 , 3 , 3 , 3 , 3 , 3 , 3 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [3 , 3 , 3 , 3 , 3 , 3 , 3 , 4 , 5 , 6 , 7 , 8 , 9 , 10 , 11] ,] ;
# [rustfmt :: skip] const RUN_BITS : [[u8 ; 15] ; 7] = [[1 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [1 , 1 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [3 , 2 , 1 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [3 , 2 , 1 , 1 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [3 , 2 , 3 , 2 , 1 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [3 , 0 , 1 , 3 , 2 , 5 , 4 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0] , [7 , 6 , 5 , 4 , 3 , 2 , 1 , 1 , 1 , 1 , 1 , 1 , 1 , 1 , 1] ,] ;
fn coeff_token_table(nc: i32) -> usize {
    if nc < 2 {
        0
    } else if nc < 4 {
        1
    } else if nc < 8 {
        2
    } else {
        3
    }
}
const NC_TABLE: [u8; 17] = [0, 0, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3];
pub struct Lut {
    width: u32,
    entry: &'static [u16],
}
const fn max_len(lens: &[u8]) -> u32 {
    let mut m = 0u8;
    let mut i = 0;
    while i < lens.len() {
        if lens[i] > m {
            m = lens[i];
        }
        i += 1;
    }
    m as u32
}
const fn build_lut<const N: usize>(lens: &[u8], bits: &[u8]) -> [u16; N] {
    let width = max_len(lens);
    assert!(
        1usize << width == N,
        "LUT size must be 1 << max code length"
    );
    let mut e = [0u16; N];
    let mut i = 0;
    while i < lens.len() {
        let l = lens[i] as u32;
        if l != 0 {
            let base = (bits[i] as usize) << (width - l);
            let span = 1usize << (width - l);
            let packed = ((i as u16) << 5) | l as u16;
            let mut k = 0;
            while k < span {
                e[base + k] = packed;
                k += 1;
            }
        }
        i += 1;
    }
    e
}
macro_rules ! lut_family { ($ arr : ident : $ n : literal , $ lens : ident , $ bits : ident , $ ($ i : literal => $ name : ident) ,* $ (,) ?) => { $ (static $ name : [u16 ; 1usize << max_len (&$ lens [$ i])] = build_lut (&$ lens [$ i] , &$ bits [$ i]) ;) * static $ arr : [Lut ; $ n] = [$ (Lut { width : max_len (&$ lens [$ i]) , entry : &$ name }) ,*] ; } ; }
lut_family ! (COEFF_TOKEN_LUT : 4 , COEFF_TOKEN_LEN , COEFF_TOKEN_BITS , 0 => CT0 , 1 => CT1 , 2 => CT2 , 3 => CT3);
lut_family ! (TOTAL_ZEROS_LUT : 15 , TOTAL_ZEROS_LEN , TOTAL_ZEROS_BITS , 0 => TZ0 , 1 => TZ1 , 2 => TZ2 , 3 => TZ3 , 4 => TZ4 , 5 => TZ5 , 6 => TZ6 , 7 => TZ7 , 8 => TZ8 , 9 => TZ9 , 10 => TZ10 , 11 => TZ11 , 12 => TZ12 , 13 => TZ13 , 14 => TZ14);
lut_family ! (CDC_TOTAL_ZEROS_LUT : 3 , CHROMA_DC_TOTAL_ZEROS_LEN , CHROMA_DC_TOTAL_ZEROS_BITS , 0 => CZ0 , 1 => CZ1 , 2 => CZ2);
lut_family ! (RUN_BEFORE_LUT : 7 , RUN_LEN , RUN_BITS , 0 => RB0 , 1 => RB1 , 2 => RB2 , 3 => RB3 , 4 => RB4 , 5 => RB5 , 6 => RB6);
static CDC_COEFF_TOKEN: [u16; 1usize << max_len(&CHROMA_DC_COEFF_TOKEN_LEN)] =
    build_lut(&CHROMA_DC_COEFF_TOKEN_LEN, &CHROMA_DC_COEFF_TOKEN_BITS);
static CDC_COEFF_TOKEN_LUT: Lut = Lut {
    width: max_len(&CHROMA_DC_COEFF_TOKEN_LEN),
    entry: &CDC_COEFF_TOKEN,
};
#[inline(always)]
fn lut_read(l: &Lut, c: &mut Cursor) -> Result<usize, OutOfData> {
    let packed = l.entry.get(c.peek(l.width) as usize).copied().unwrap_or(0);
    let len = (packed & 0x1F) as u32;
    if len == 0 {
        return Err(OutOfData);
    }
    c.skip(len)?;
    Ok((packed >> 5) as usize)
}
fn level_to_code(level: i32) -> i32 {
    if level > 0 {
        (level << 1) - 2
    } else {
        (-level << 1) - 1
    }
}
#[inline(always)]
fn read_level_prefix(c: &mut Cursor) -> Result<u32, OutOfData> {
    let window = c.peek24();
    let lz = window.leading_zeros() - 8;
    if lz < 24 {
        c.skip(lz + 1)?;
        return Ok(lz);
    }
    let (n, c2) = read_level_prefix_long(*c)?;
    *c = c2;
    Ok(n)
}
#[cold]
#[inline(never)]
fn read_level_prefix_long(mut c: Cursor) -> Result<(u32, Cursor), OutOfData> {
    let mut n = 0;
    while !c.read_bit()? {
        n += 1;
        if n > 32 {
            return Err(OutOfData);
        }
    }
    Ok((n, c))
}
#[inline(always)]
pub fn decode_residual_block_into<const MAX: usize, const N: usize>(
    r: &mut BitReader,
    nc: i32,
    out: &mut [i32; N],
) -> Result<u8, OutOfData> {
    const { assert!(MAX == 16 || MAX == 15 || MAX == 4) };
    const { assert!((N == 16 || N == 4) && MAX <= N) };
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Entropy);
    let mut c = r.cursor();
    let win = c.peek24();
    let lut: &Lut = if MAX == 4 {
        &CDC_COEFF_TOKEN_LUT
    } else {
        &COEFF_TOKEN_LUT[NC_TABLE[(nc as usize).min(16)] as usize]
    };
    let packed = lut
        .entry
        .get((win >> (24 - lut.width)) as usize)
        .copied()
        .unwrap_or(0);
    let len = (packed & 0x1F) as u32;
    if len == 0 {
        return Err(OutOfData);
    }
    let idx = (packed >> 5) as usize;
    let (total_coeff, trailing_ones) = (idx >> 2, idx & 3);
    if total_coeff == 0 {
        c.skip(len)?;
        r.commit(c);
        return Ok(0);
    }
    let signs = (win >> (24 - len - trailing_ones as u32)) & ((1u32 << trailing_ones) - 1);
    c.skip(len + trailing_ones as u32)?;
    let (data, pos) = c.into_parts();
    let (tc, pos) = decode_coded_body::<MAX, N>(data, pos, total_coeff, trailing_ones, signs, out)?;
    r.commit(Cursor::from_parts(data, pos));
    Ok(tc)
}
#[inline(never)]
fn decode_coded_body<const MAX: usize, const N: usize>(
    data: &[u8],
    pos: usize,
    total_coeff: usize,
    trailing_ones: usize,
    signs: u32,
    out: &mut [i32; N],
) -> Result<(u8, usize), OutOfData> {
    let mut c = Cursor::from_parts(data, pos);
    if total_coeff > MAX {
        return Err(OutOfData);
    }
    let _lg = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::CavLvl);
    let mut levels = [0i32; N];
    let s3 = signs << (3 - trailing_ones);
    levels[0] = 1 - 2 * ((s3 >> 2) & 1) as i32;
    levels[1] = 1 - 2 * ((s3 >> 1) & 1) as i32;
    levels[2] = 1 - 2 * (s3 & 1) as i32;
    let mut suffix_length = if total_coeff > 10 && trailing_ones < 3 {
        1
    } else {
        0
    };
    for k in trailing_ones..total_coeff {
        let level_prefix = read_level_prefix(&mut c)?;
        let level_suffix_size = if level_prefix == 14 && suffix_length == 0 {
            4
        } else if level_prefix >= 15 {
            level_prefix - 3
        } else {
            suffix_length
        };
        let level_suffix = if level_suffix_size > 0 {
            c.read_bits(level_suffix_size)?
        } else {
            0
        };
        let mut level_code = (level_prefix.min(15) << suffix_length) as i32 + level_suffix as i32;
        if level_prefix >= 15 && suffix_length == 0 {
            level_code += 15;
        }
        if level_prefix >= 16 {
            level_code += (1 << (level_prefix - 3)) - 4096;
        }
        if k == trailing_ones && trailing_ones < 3 {
            level_code += 2;
        }
        let level = if level_code % 2 == 0 {
            (level_code + 2) >> 1
        } else {
            (-level_code - 1) >> 1
        };
        if level_prefix >= 16 && !(-32768..=32767).contains(&level) {
            return Err(OutOfData);
        }
        levels[k & (N - 1)] = level;
        if suffix_length == 0 {
            suffix_length = 1;
        }
        if level.abs() > (3 << (suffix_length - 1)) && suffix_length < 6 {
            suffix_length += 1;
        }
    }
    drop(_lg);
    let _rg = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::CavRun);
    let total_zeros = if total_coeff < MAX {
        let l = if MAX == 4 {
            &CDC_TOTAL_ZEROS_LUT[(total_coeff - 1).min(2)]
        } else {
            &TOTAL_ZEROS_LUT[(total_coeff - 1).min(14)]
        };
        lut_read(l, &mut c)?
    } else {
        0
    };
    if total_coeff + total_zeros > MAX {
        return Err(OutOfData);
    }
    let mut zeros_left = total_zeros;
    out[(total_coeff - 1 + total_zeros) & (N - 1)] = levels[0];
    for k in 1..total_coeff {
        if zeros_left > 0 {
            let run = lut_read(&RUN_BEFORE_LUT[zeros_left.min(7) - 1], &mut c)?;
            zeros_left = zeros_left.checked_sub(run).ok_or(OutOfData)?;
        }
        out[((total_coeff - 1 - k) + zeros_left) & (N - 1)] = levels[k & (N - 1)];
    }
    Ok((total_coeff as u8, c.bit_pos()))
}
