use std::vec;
use std::vec::Vec;
const NORM_ADJUST: [[i32; 3]; 6] = [
    [10, 16, 13],
    [11, 18, 14],
    [13, 20, 16],
    [14, 23, 18],
    [16, 25, 20],
    [18, 29, 23],
];

#[inline]
const fn pos_group(i: usize, j: usize) -> usize {
    match (i % 2, j % 2) {
        (0, 0) => 0,
        (1, 1) => 1,
        _ => 2,
    }
}
const POS_GROUP_FLAT: [usize; 16] = {
    let mut out = [0usize; 16];
    let mut i = 0;
    while i < 4 {
        let mut j = 0;
        while j < 4 {
            out[i * 4 + j] = pos_group(i, j);
            j += 1;
        }
        i += 1;
    }
    out
};

const fn flatten_level_scale() -> [[i32; 16]; 6] {
    let mut out = [[0i32; 16]; 6];
    let mut m = 0;
    while m < 6 {
        let mut idx = 0;
        while idx < 16 {
            out[m][idx] = 16 * NORM_ADJUST[m][POS_GROUP_FLAT[idx]];
            idx += 1;
        }
        m += 1;
    }
    out
}
const LEVEL_SCALE_FLAT: [[i32; 16]; 6] = flatten_level_scale();
#[inline]
fn inv_1d(d0: i32, d1: i32, d2: i32, d3: i32) -> (i32, i32, i32, i32) {
    let e0 = d0 + d2;
    let e1 = d0 - d2;
    let e2 = (d1 >> 1) - d3;
    let e3 = d1 + (d3 >> 1);
    (e0 + e3, e1 + e2, e1 - e2, e0 - e3)
}

pub fn dequantize(levels: &[i32; 16], qp: u8) -> [i32; 16] {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Dequant);
    let m = (qp % 6) as usize;
    let shift = (qp / 6) as i32;
    let ls = &LEVEL_SCALE_FLAT[m];
    let mut out = [0i32; 16];
    if qp >= 24 {
        let sh = shift - 4;
        for idx in 0..16 {
            out[idx] = (levels[idx] * ls[idx]) << sh;
        }
    } else {
        let add = 1 << (3 - shift);
        let sh = 4 - shift;
        for idx in 0..16 {
            out[idx] = (levels[idx] * ls[idx] + add) >> sh;
        }
    }
    out
}
pub const ZIG4: [usize; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];

#[inline]
pub fn dequantize_dc4(level: i32, qp: u8, weight0: Option<i32>) -> i32 {
    let m = (qp % 6) as usize;
    let shift = (qp / 6) as i32;
    let ls0 = match weight0 {
        Some(w) => w * NORM_ADJUST[m][POS_GROUP_FLAT[0]],
        None => LEVEL_SCALE_FLAT[m][0],
    };
    if qp >= 24 {
        (level * ls0) << (shift - 4)
    } else {
        (level * ls0 + (1 << (3 - shift))) >> (4 - shift)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DequantQp {
    pub ls: [i32; 16],
    pub add: i32,
    pub sr: i32,
}
impl DequantQp {
    pub const fn flat(qp: u8) -> Self {
        let m = (qp % 6) as usize;
        let shift = (qp / 6) as i32;
        let mut ls = [0i32; 16];
        let mut i = 0;
        while i < 16 {
            ls[i] = if qp >= 24 {
                LEVEL_SCALE_FLAT[m][i] << (shift - 4)
            } else {
                LEVEL_SCALE_FLAT[m][i]
            };
            i += 1;
        }
        if qp >= 24 {
            Self { ls, add: 0, sr: 0 }
        } else {
            Self {
                ls,
                add: 1 << (3 - shift),
                sr: 4 - shift,
            }
        }
    }
    pub fn weighted(qp: u8, weight: &[i32; 16]) -> Self {
        let m = (qp % 6) as usize;
        let shift = (qp / 6) as i32;
        let ls: [i32; 16] = core::array::from_fn(|idx| {
            let w = weight[idx] * NORM_ADJUST[m][POS_GROUP_FLAT[idx]];
            if qp >= 24 { w << (shift - 4) } else { w }
        });
        if qp >= 24 {
            Self { ls, add: 0, sr: 0 }
        } else {
            Self {
                ls,
                add: 1 << (3 - shift),
                sr: 4 - shift,
            }
        }
    }
    pub fn apply(&self, raster: &[i32; 16]) -> [i32; 16] {
        core::array::from_fn(|i| {
            (raster[i].wrapping_mul(self.ls[i]).wrapping_add(self.add)) >> self.sr
        })
    }
}
pub const DQ_FLAT: [DequantQp; 52] = {
    let mut t = [DequantQp {
        ls: [0; 16],
        add: 0,
        sr: 0,
    }; 52];
    let mut q = 0;
    while q < 52 {
        t[q] = DequantQp::flat(q as u8);
        q += 1;
    }
    t
};
pub fn inverse_core(coeffs: &[i32; 16]) -> [i32; 16] {
    let mut m = *coeffs;
    for r in 0..4 {
        let (a, b, c, d) = inv_1d(m[r * 4], m[r * 4 + 1], m[r * 4 + 2], m[r * 4 + 3]);
        m[r * 4] = a;
        m[r * 4 + 1] = b;
        m[r * 4 + 2] = c;
        m[r * 4 + 3] = d;
    }
    for c in 0..4 {
        let (a, b, cc, d) = inv_1d(m[c], m[4 + c], m[8 + c], m[12 + c]);
        m[c] = a;
        m[4 + c] = b;
        m[8 + c] = cc;
        m[12 + c] = d;
    }
    for v in m.iter_mut() {
        *v = (*v + 32) >> 6;
    }
    m
}

const NORM_ADJUST_8X8: [[i32; 6]; 6] = [
    [20, 18, 32, 19, 25, 24],
    [22, 19, 35, 21, 28, 26],
    [26, 23, 42, 24, 33, 31],
    [28, 25, 45, 26, 35, 33],
    [32, 28, 51, 30, 40, 38],
    [36, 32, 58, 34, 46, 43],
];
#[inline]
const fn pos_group_8x8(i: usize, j: usize) -> usize {
    let (i4, j4) = (i % 4, j % 4);
    if i4 == 0 && j4 == 0 {
        0
    } else if i % 2 == 1 && j % 2 == 1 {
        1
    } else if i4 == 2 && j4 == 2 {
        2
    } else if (i4 == 0 && j % 2 == 1) || (i % 2 == 1 && j4 == 0) {
        3
    } else if (i4 == 0 && j4 == 2) || (i4 == 2 && j4 == 0) {
        4
    } else {
        5
    }
}
const POS_GROUP_8X8_FLAT: [usize; 64] = {
    let mut out = [0usize; 64];
    let mut i = 0;
    while i < 8 {
        let mut j = 0;
        while j < 8 {
            out[i * 8 + j] = pos_group_8x8(i, j);
            j += 1;
        }
        i += 1;
    }
    out
};
#[inline]
fn inv_1d_8x8(d: &[i32; 8]) -> [i32; 8] {
    let a0 = d[0] + d[4];
    let a4 = d[0] - d[4];
    let a2 = (d[2] >> 1) - d[6];
    let a6 = d[2] + (d[6] >> 1);
    let b0 = a0 + a6;
    let b2 = a4 + a2;
    let b4 = a4 - a2;
    let b6 = a0 - a6;
    let a1 = -d[3] + d[5] - d[7] - (d[7] >> 1);
    let a3 = d[1] + d[7] - d[3] - (d[3] >> 1);
    let a5 = -d[1] + d[7] + d[5] + (d[5] >> 1);
    let a7 = d[3] + d[5] + d[1] + (d[1] >> 1);
    let b1 = a1 + (a7 >> 2);
    let b7 = a7 - (a1 >> 2);
    let b3 = a3 + (a5 >> 2);
    let b5 = (a3 >> 2) - a5;
    [
        b0 + b7,
        b2 + b5,
        b4 + b3,
        b6 + b1,
        b6 - b1,
        b4 - b3,
        b2 - b5,
        b0 - b7,
    ]
}
pub fn inverse_core_8x8(coeffs: &[i32; 64]) -> [i32; 64] {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Reconstruct);
    let mut m = *coeffs;
    for r in 0..8 {
        let row: [i32; 8] = core::array::from_fn(|k| m[r * 8 + k]);
        let o = inv_1d_8x8(&row);
        for k in 0..8 {
            m[r * 8 + k] = o[k];
        }
    }
    for c in 0..8 {
        let col: [i32; 8] = core::array::from_fn(|k| m[k * 8 + c]);
        let o = inv_1d_8x8(&col);
        for k in 0..8 {
            m[k * 8 + c] = o[k];
        }
    }
    for v in m.iter_mut() {
        *v = (*v + 32) >> 6;
    }
    m
}
pub fn dequantize_8x8(levels: &[i32; 64], qp: u8, weight: &[i32; 64]) -> [i32; 64] {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Dequant);
    let m = (qp % 6) as usize;
    let shift = (qp / 6) as i32;
    let mut out = [0i32; 64];
    if qp >= 36 {
        let sh = shift - 6;
        for idx in 0..64 {
            let ls = weight[idx] * NORM_ADJUST_8X8[m][POS_GROUP_8X8_FLAT[idx]];
            out[idx] = (levels[idx] * ls) << sh;
        }
    } else {
        let add = 1 << (5 - shift);
        let sh = 6 - shift;
        for idx in 0..64 {
            let ls = weight[idx] * NORM_ADJUST_8X8[m][POS_GROUP_8X8_FLAT[idx]];
            out[idx] = (levels[idx] * ls + add) >> sh;
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dequant8Qp {
    pub ls: [i32; 64],
    pub add: i32,
    pub sr: i32,
}
impl Dequant8Qp {
    pub const fn build(qp: u8, weight: &[i32; 64]) -> Self {
        let m = (qp % 6) as usize;
        let shift = (qp / 6) as i32;
        let mut ls = [0i32; 64];
        let mut i = 0;
        while i < 64 {
            let w = weight[i] * NORM_ADJUST_8X8[m][POS_GROUP_8X8_FLAT[i]];
            ls[i] = if qp >= 36 { w << (shift - 6) } else { w };
            i += 1;
        }
        if qp >= 36 {
            Self { ls, add: 0, sr: 0 }
        } else {
            Self {
                ls,
                add: 1 << (5 - shift),
                sr: 6 - shift,
            }
        }
    }
}
pub const DQ8_FLAT: [Dequant8Qp; 52] = {
    let mut t = [Dequant8Qp {
        ls: [0; 64],
        add: 0,
        sr: 0,
    }; 52];
    let mut q = 0;
    while q < 52 {
        t[q] = Dequant8Qp::build(q as u8, &[16i32; 64]);
        q += 1;
    }
    t
};
#[inline]
pub fn dequantize_8x8_dq(levels: &[i32; 64], q: &Dequant8Qp) -> [i32; 64] {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Dequant);
    core::array::from_fn(|i| (levels[i].wrapping_mul(q.ls[i]).wrapping_add(q.add)) >> q.sr)
}
#[inline]
pub fn inverse_quant_8x8_dq(levels: &[i32; 64], q: &Dequant8Qp) -> [i32; 64] {
    inverse_core_8x8(&dequantize_8x8_dq(levels, q))
}

#[inline]
fn hadamard_1d(a: i32, b: i32, c: i32, d: i32) -> (i32, i32, i32, i32) {
    (a + b + c + d, a + b - c - d, a - b - c + d, a - b + c - d)
}
pub fn hadamard_4x4(block: &[i32; 16]) -> [i32; 16] {
    let mut m = *block;
    for r in 0..4 {
        let (a, b, c, d) = hadamard_1d(m[r * 4], m[r * 4 + 1], m[r * 4 + 2], m[r * 4 + 3]);
        m[r * 4] = a;
        m[r * 4 + 1] = b;
        m[r * 4 + 2] = c;
        m[r * 4 + 3] = d;
    }
    for c in 0..4 {
        let (a, b, cc, d) = hadamard_1d(m[c], m[4 + c], m[8 + c], m[12 + c]);
        m[c] = a;
        m[4 + c] = b;
        m[8 + c] = cc;
        m[12 + c] = d;
    }
    m
}
pub fn inverse_quant_luma_dc(levels: &[i32; 16], qp: u8) -> [i32; 16] {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Dequant);
    let g = hadamard_4x4(levels);
    let m = (qp % 6) as usize;
    let shift = (qp / 6) as i32;
    let level_scale = 16 * NORM_ADJUST[m][0];
    let mut out = [0i32; 16];
    for (o, &gv) in out.iter_mut().zip(g.iter()) {
        *o = if qp >= 36 {
            (gv * level_scale) << (shift - 6)
        } else {
            (gv * level_scale + (1 << (5 - shift))) >> (6 - shift)
        };
    }
    out
}
pub fn inverse_quant_luma_dc_weighted(levels: &[i32; 16], qp: u8, w00: i32) -> [i32; 16] {
    let g = hadamard_4x4(levels);
    let m = (qp % 6) as usize;
    let shift = (qp / 6) as i32;
    let level_scale = w00 * NORM_ADJUST[m][0];
    let mut out = [0i32; 16];
    for (o, &gv) in out.iter_mut().zip(g.iter()) {
        *o = if qp >= 36 {
            (gv * level_scale) << (shift - 6)
        } else {
            (gv * level_scale + (1 << (5 - shift))) >> (6 - shift)
        };
    }
    out
}
pub fn inverse_quant_luma_dc_scan(scan: &[i32; 16], qp: u8, w00: Option<i32>) -> [i32; 16] {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Dequant);
    let m = (qp % 6) as usize;
    let shift = (qp / 6) as i32;
    let _ = (m, shift);
    {
        let raster = crate::h264::common::cavlc::un_scan_4x4_dcac(scan);
        match w00 {
            Some(w) => inverse_quant_luma_dc_weighted(&raster, qp, w),
            None => inverse_quant_luma_dc(&raster, qp),
        }
    }
}
pub fn inverse_quant_chroma_dc_weighted(levels: &[i32; 4], qp: u8, w00: i32) -> [i32; 4] {
    let g = hadamard_2x2(levels);
    let m = (qp % 6) as usize;
    let shift = (qp / 6) as i32;
    let level_scale = w00 * NORM_ADJUST[m][0];
    let mut out = [0i32; 4];
    for (o, &gv) in out.iter_mut().zip(g.iter()) {
        *o = ((gv * level_scale) << shift) >> 5;
    }
    out
}
pub fn hadamard_2x2(dc: &[i32; 4]) -> [i32; 4] {
    let (a, b, c, d) = (dc[0], dc[1], dc[2], dc[3]);
    [a + b + c + d, a - b + c - d, a + b - c - d, a - b - c + d]
}
pub fn inverse_quant_chroma_dc(levels: &[i32; 4], qp: u8) -> [i32; 4] {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Dequant);
    let g = hadamard_2x2(levels);
    let m = (qp % 6) as usize;
    let shift = (qp / 6) as i32;
    let level_scale = 16 * NORM_ADJUST[m][0];
    let mut out = [0i32; 4];
    for (o, &gv) in out.iter_mut().zip(g.iter()) {
        *o = ((gv * level_scale) << shift) >> 5;
    }
    out
}
