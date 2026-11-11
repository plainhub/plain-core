#![forbid(unsafe_code)]
use crate::compression::backend::{Code, ENOUGH_DISTS, ENOUGH_LENS};
pub(crate) enum CodeType {
    Codes,
    Lens,
    Dists,
}
const MAX_BITS: usize = 15;
const LBASE: [u16; 31] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258, 0, 0,
];
const LEXT: [u8; 31] = [
    16, 16, 16, 16, 16, 16, 16, 16, 17, 17, 17, 17, 18, 18, 18, 18, 19, 19, 19, 19, 20, 20, 20, 20,
    21, 21, 21, 21, 16, 77, 202,
];
const DBASE: [u16; 32] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577, 0, 0,
];
const DEXT: [u8; 32] = [
    16, 16, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22, 22, 23, 23, 24, 24, 25, 25, 26, 26,
    27, 27, 28, 28, 29, 29, 64, 64,
];
#[repr(i32)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum InflateTable {
    EnoughIsNotEnough = 1,
    Success { root: usize, used: usize } = 0,
    InvalidCode = -1,
}
pub(crate) fn inflate_table(
    codetype: CodeType,
    lens: &[u16],
    table: &mut [Code],
    bits: usize,
    work: &mut [u16],
) -> InflateTable {
    let mut count = [0u16; MAX_BITS + 1];
    let (mut min, mut max) = (MAX_BITS, 0);
    for &len in lens {
        if len > 0 {
            count[len as usize] += 1;
            max = Ord::max(max, usize::from(len));
            min = Ord::min(min, usize::from(len));
        }
    }
    if max == 0 {
        let code = Code {
            op: 64,
            bits: 1,
            val: 0,
        };
        table[0] = code;
        table[1] = code;
        return InflateTable::Success { root: 1, used: 2 };
    }
    let root = bits.clamp(min, max);
    let mut left = 1u32;
    for &sym in &count[1..] {
        left = match (left << 1).checked_sub(u32::from(sym)) {
            None => return InflateTable::InvalidCode,
            Some(v) => v,
        };
    }
    if left > 0 && (matches!(codetype, CodeType::Codes) || max != 1) {
        return InflateTable::InvalidCode;
    }
    let mut offs = [0u16; MAX_BITS + 1];
    for len in 1..MAX_BITS {
        offs[len + 1] = offs[len] + count[len];
    }
    for (sym, len) in lens.iter().copied().enumerate() {
        if len != 0 {
            let offset = offs[len as usize];
            offs[len as usize] += 1;
            work[offset as usize] = sym as u16;
        }
    }
    let (base, extra, match_) = match codetype {
        CodeType::Codes => (&[] as &[_], &[] as &[_], 20),
        CodeType::Lens => (&LBASE[..], &LEXT[..], 257),
        CodeType::Dists => (&DBASE[..], &DEXT[..], 0),
    };
    let mut used = 1 << root;
    if matches!(codetype, CodeType::Lens) && used > ENOUGH_LENS {
        return InflateTable::EnoughIsNotEnough;
    }
    if matches!(codetype, CodeType::Dists) && used > ENOUGH_DISTS {
        return InflateTable::EnoughIsNotEnough;
    }
    let mut huff = 0;
    let mut reversed_huff = 0u32;
    let mut sym = 0;
    let mut len = min;
    let mut next = 0usize;
    let mut curr = root;
    let mut drop_ = 0;
    let mut low = usize::MAX;
    let mask = used - 1;
    'outer: loop {
        let here = if work[sym] >= match_ {
            Code {
                bits: (len - drop_) as u8,
                op: extra[(work[sym] - match_) as usize],
                val: base[(work[sym] - match_) as usize],
            }
        } else if work[sym] + 1 < match_ {
            Code {
                bits: (len - drop_) as u8,
                op: 0,
                val: work[sym],
            }
        } else {
            Code {
                bits: (len - drop_) as u8,
                op: 0b01100000,
                val: 0,
            }
        };
        let incr = 1 << (len - drop_);
        let mut fill = 1 << curr;
        let min = fill;
        loop {
            fill -= incr;
            table[next + (huff >> drop_) + fill] = here;
            if fill == 0 {
                break;
            }
        }
        reversed_huff = reversed_huff.wrapping_add(0x80000000u32 >> (len - 1));
        huff = reversed_huff.reverse_bits() as usize;
        sym += 1;
        count[len] -= 1;
        if count[len] == 0 {
            if len == max {
                break 'outer;
            }
            len = lens[work[sym] as usize] as usize;
        }
        if len > root && (huff & mask) != low {
            if drop_ == 0 {
                drop_ = root;
            }
            next += min;
            curr = len - drop_;
            let mut left = 1 << curr;
            while curr + drop_ < max {
                left -= count[curr + drop_] as i32;
                if left <= 0 {
                    break;
                }
                curr += 1;
                left <<= 1;
            }
            used += 1usize << curr;
            if matches!(codetype, CodeType::Lens) && used > ENOUGH_LENS {
                return InflateTable::EnoughIsNotEnough;
            }
            if matches!(codetype, CodeType::Dists) && used > ENOUGH_DISTS {
                return InflateTable::EnoughIsNotEnough;
            }
            low = huff & mask;
            table[low] = Code {
                op: curr as u8,
                bits: root as u8,
                val: next as u16,
            };
        }
    }
    if huff != 0 {
        let here = Code {
            op: 64,
            bits: (len - drop_) as u8,
            val: 0,
        };
        table[next..][huff] = here;
    }
    InflateTable::Success { root, used }
}
