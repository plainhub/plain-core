use crate::h264::common::transform::inverse_core;
pub const LUMA_4X4_SCAN_XY: [(usize, usize); 16] = [
    (0, 0),
    (1, 0),
    (0, 1),
    (1, 1),
    (2, 0),
    (3, 0),
    (2, 1),
    (3, 1),
    (0, 2),
    (1, 2),
    (0, 3),
    (1, 3),
    (2, 2),
    (3, 2),
    (2, 3),
    (3, 3),
];
pub const CHROMA_4X4_SCAN_XY: [(usize, usize); 4] = [(0, 0), (1, 0), (0, 1), (1, 1)];
#[inline]
pub(crate) fn abl_intra() -> bool {
    {
        return false;
    }
}
#[inline]
pub(crate) fn abl_recon() -> bool {
    {
        return false;
    }
}
pub fn chroma_qp(qp: u8) -> u8 {
    const QPC: [u8; 22] = [
        29, 30, 31, 32, 32, 33, 34, 34, 35, 35, 36, 36, 37, 37, 37, 38, 38, 38, 39, 39, 39, 39,
    ];
    if qp < 30 {
        qp
    } else {
        QPC[((qp - 30) as usize).min(21)]
    }
}

fn sum(s: &[u8]) -> i32 {
    s.iter().map(|&x| x as i32).sum()
}
pub fn luma16x16_dc(avail_top: bool, avail_left: bool, top: &[u8; 16], left: &[u8; 16]) -> u8 {
    let v = if avail_top && avail_left {
        (sum(top) + sum(left) + 16) >> 5
    } else if avail_top {
        (sum(top) + 8) >> 4
    } else if avail_left {
        (sum(left) + 8) >> 4
    } else {
        128
    };
    v as u8
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I16Mode {
    Vertical = 0,
    Horizontal = 1,
    Dc = 2,
    Plane = 3,
}
impl I16Mode {
    pub fn from_id(v: u32) -> Self {
        match v & 3 {
            0 => I16Mode::Vertical,
            1 => I16Mode::Horizontal,
            2 => I16Mode::Dc,
            _ => I16Mode::Plane,
        }
    }
    pub fn available(self, avail_top: bool, avail_left: bool) -> bool {
        match self {
            I16Mode::Vertical => avail_top,
            I16Mode::Horizontal => avail_left,
            I16Mode::Dc => true,
            I16Mode::Plane => avail_top && avail_left,
        }
    }
}
pub fn luma16x16_pred(
    mode: I16Mode,
    avail_top: bool,
    avail_left: bool,
    top: &[u8; 16],
    left: &[u8; 16],
    corner: u8,
) -> [u8; 256] {
    if abl_intra() {
        return [128; 256];
    }
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::IntraPred);
    let mut out = [0u8; 256];
    match mode {
        I16Mode::Vertical => {
            for row in out.chunks_exact_mut(16) {
                row.copy_from_slice(top);
            }
        }
        I16Mode::Horizontal => {
            for (row, &l) in out.chunks_exact_mut(16).zip(left.iter()) {
                row.fill(l);
            }
        }
        I16Mode::Dc => {
            let v = luma16x16_dc(avail_top, avail_left, top, left);
            out.fill(v);
        }
        I16Mode::Plane => {
            let tp = |i: isize| -> i32 {
                if i < 0 {
                    corner as i32
                } else {
                    top[i as usize] as i32
                }
            };
            let lf = |i: isize| -> i32 {
                if i < 0 {
                    corner as i32
                } else {
                    left[i as usize] as i32
                }
            };
            let mut h = 0i32;
            let mut v = 0i32;
            for xp in 0..8i32 {
                h += (xp + 1) * (tp(8 + xp as isize) - tp(6 - xp as isize));
            }
            for yp in 0..8i32 {
                v += (yp + 1) * (lf(8 + yp as isize) - lf(6 - yp as isize));
            }
            let a = 16 * (left[15] as i32 + top[15] as i32);
            let b = (5 * h + 32) >> 6;
            let c = (5 * v + 32) >> 6;
            for y in 0..16i32 {
                for x in 0..16i32 {
                    let val = (a + b * (x - 7) + c * (y - 7) + 16) >> 5;
                    out[(y * 16 + x) as usize] = clip_u8(val);
                }
            }
        }
    }
    out
}
pub fn intra4x4_pred(
    mode: u8,
    avail_top: bool,
    avail_left: bool,
    top: &[u8; 8],
    left: &[u8; 4],
    corner: u8,
) -> [u8; 16] {
    if abl_intra() {
        return [128; 16];
    }
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::IntraPred);
    let mut e = [0i32; 13];
    for k in 0..4 {
        e[3 - k] = left[k] as i32;
    }
    e[4] = corner as i32;
    for k in 0..8 {
        e[5 + k] = top[k] as i32;
    }
    let mut f2 = [0i32; 16];
    let mut f3 = [0i32; 16];
    for i in 0..12 {
        f2[i] = (e[i] + e[i + 1] + 1) >> 1;
    }
    for i in 0..11 {
        f3[i] = (e[i] + 2 * e[i + 1] + e[i + 2] + 2) >> 2;
    }
    let mut out = [0u8; 16];
    match mode {
        0 => {
            for row in out.chunks_exact_mut(4) {
                row.copy_from_slice(&top[..4]);
            }
        }
        1 => {
            for (row, &l) in out.chunks_exact_mut(4).zip(left.iter()) {
                row.fill(l);
            }
        }
        2 => {
            let (ts, ls) = (e[5] + e[6] + e[7] + e[8], e[0] + e[1] + e[2] + e[3]);
            let v = if avail_top && avail_left {
                (ts + ls + 4) >> 3
            } else if avail_top {
                (ts + 2) >> 2
            } else if avail_left {
                (ls + 2) >> 2
            } else {
                128
            };
            out.fill(v as u8);
        }
        3 => {
            for y in 0..4 {
                for x in 0..4 {
                    out[y * 4 + x] = if x == 3 && y == 3 {
                        ((e[11] + 3 * e[12] + 2) >> 2) as u8
                    } else {
                        f3[(5 + x + y) & 15] as u8
                    };
                }
            }
        }
        4 => {
            for y in 0..4 {
                for x in 0..4 {
                    out[y * 4 + x] = f3[(3 + x + 16 - y) & 15] as u8;
                }
            }
        }
        5 => {
            for y in 0..4i32 {
                for x in 0..4i32 {
                    let zvr = 2 * x - y;
                    let k = x - (y >> 1);
                    let v = if zvr >= 0 && zvr % 2 == 0 {
                        f2[(4 + k) as usize & 15]
                    } else if zvr >= 0 {
                        f3[(3 + k) as usize & 15]
                    } else if zvr == -1 {
                        f3[3]
                    } else {
                        f3[(4 - y) as usize & 15]
                    };
                    out[(y * 4 + x) as usize] = v as u8;
                }
            }
        }
        6 => {
            for y in 0..4i32 {
                for x in 0..4i32 {
                    let zhd = 2 * y - x;
                    let k = y - (x >> 1);
                    let v = if zhd >= 0 && zhd % 2 == 0 {
                        f2[(3 - k) as usize & 15]
                    } else if zhd >= 0 {
                        f3[(3 - k) as usize & 15]
                    } else if zhd == -1 {
                        f3[3]
                    } else {
                        f3[(2 + x) as usize & 15]
                    };
                    out[(y * 4 + x) as usize] = v as u8;
                }
            }
        }
        7 => {
            for y in 0..4 {
                for x in 0..4 {
                    let k = x + (y >> 1);
                    out[y * 4 + x] = if y % 2 == 0 {
                        f2[(5 + k) & 15]
                    } else {
                        f3[(5 + k) & 15]
                    } as u8;
                }
            }
        }
        _ => {
            for y in 0..4 {
                for x in 0..4 {
                    let zhu = x + 2 * y;
                    let k = y + (x >> 1);
                    let v = if zhu <= 4 && zhu % 2 == 0 {
                        f2[(2 + 16 - k) & 15]
                    } else if zhu < 5 {
                        f3[(1 + 16 - k) & 15]
                    } else if zhu == 5 {
                        (e[1] + 3 * e[0] + 2) >> 2
                    } else {
                        e[0]
                    };
                    out[y * 4 + x] = v as u8;
                }
            }
        }
    }
    out
}
pub fn chroma8x8_dc(avail_top: bool, avail_left: bool, top: &[u8; 8], left: &[u8; 8]) -> [u8; 64] {
    let mut out = [128u8; 64];
    for &(bx, by) in &CHROMA_4X4_SCAN_XY {
        let (x0, y0) = (bx * 4, by * 4);
        let top4 = &top[x0..x0 + 4];
        let left4 = &left[y0..y0 + 4];
        let prefer_top = (x0, y0) == (4, 0);
        let prefer_left = (x0, y0) == (0, 4);
        let dc: i32 = if prefer_top {
            if avail_top {
                (sum(top4) + 2) >> 2
            } else if avail_left {
                (sum(left4) + 2) >> 2
            } else {
                128
            }
        } else if prefer_left {
            if avail_left {
                (sum(left4) + 2) >> 2
            } else if avail_top {
                (sum(top4) + 2) >> 2
            } else {
                128
            }
        } else {
            if avail_top && avail_left {
                (sum(top4) + sum(left4) + 4) >> 3
            } else if avail_top {
                (sum(top4) + 2) >> 2
            } else if avail_left {
                (sum(left4) + 2) >> 2
            } else {
                128
            }
        };
        for dy in 0..4 {
            for dx in 0..4 {
                out[(y0 + dy) * 8 + (x0 + dx)] = dc as u8;
            }
        }
    }
    out
}
pub fn chroma8x8_pred(
    mode: u8,
    avail_top: bool,
    avail_left: bool,
    top: &[u8; 8],
    left: &[u8; 8],
    corner: u8,
) -> [u8; 64] {
    if abl_intra() {
        return [128; 64];
    }
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::IntraPred);
    match mode {
        1 => {
            let mut out = [0u8; 64];
            for y in 0..8 {
                for x in 0..8 {
                    out[y * 8 + x] = left[y];
                }
            }
            out
        }
        2 => {
            let mut out = [0u8; 64];
            for y in 0..8 {
                for x in 0..8 {
                    out[y * 8 + x] = top[x];
                }
            }
            out
        }
        3 => {
            let tt = |k: i32| {
                if k < 0 {
                    corner as i32
                } else {
                    top[k as usize] as i32
                }
            };
            let ll = |k: i32| {
                if k < 0 {
                    corner as i32
                } else {
                    left[k as usize] as i32
                }
            };
            let mut h = 0i32;
            let mut v = 0i32;
            for xp in 0..4i32 {
                h += (xp + 1) * (top[(4 + xp) as usize] as i32 - tt(2 - xp));
            }
            for yp in 0..4i32 {
                v += (yp + 1) * (left[(4 + yp) as usize] as i32 - ll(2 - yp));
            }
            let a = 16 * (left[7] as i32 + top[7] as i32);
            let b = (17 * h + 16) >> 5;
            let c = (17 * v + 16) >> 5;
            let mut out = [0u8; 64];
            for y in 0..8i32 {
                for x in 0..8i32 {
                    out[(y * 8 + x) as usize] = clip_u8((a + b * (x - 3) + c * (y - 3) + 16) >> 5);
                }
            }
            out
        }
        _ => chroma8x8_dc(avail_top, avail_left, top, left),
    }
}

#[inline]
pub fn clip_u8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

#[inline]
pub fn reconstruct_4x4_into(
    deq: &[i32; 16],
    pred: &[u8],
    p_off: usize,
    p_stride: usize,
    rec: &mut [u8],
    r_off: usize,
    r_stride: usize,
) {
    if abl_recon() {
        for r in 0..4 {
            let src = &pred[p_off + r * p_stride..][..4];
            rec[r_off + r * r_stride..][..4].copy_from_slice(src);
        }
        return;
    }
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Reconstruct);
    let res = crate::h264::common::transform::inverse_core(deq);
    for r in 0..4 {
        let src = &pred[p_off + r * p_stride..][..4];
        let dst = &mut rec[r_off + r * r_stride..][..4];
        let row = &res[r * 4..][..4];
        for c in 0..4 {
            dst[c] = clip_u8(src[c] as i32 + row[c]);
        }
    }
}
#[inline]
pub fn reconstruct_4x4_scan_into<const AC: bool>(
    scan: &[i32; 16],
    q: &crate::h264::common::transform::DequantQp,
    dc: i32,
    pred: &[u8],
    p_off: usize,
    p_stride: usize,
    rec: &mut [u8],
    r_off: usize,
    r_stride: usize,
) {
    if abl_recon() {
        for r in 0..4 {
            let src = &pred[p_off + r * p_stride..][..4];
            rec[r_off + r * r_stride..][..4].copy_from_slice(src);
        }
        return;
    }
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Reconstruct);
    let raster = if AC {
        let mut d = [0i32; 16];
        crate::h264::common::cavlc::un_scan_4x4_ac_into(&scan[..15], &mut d);
        d
    } else {
        crate::h264::common::cavlc::un_scan_4x4_dcac(scan)
    };
    let mut deq = q.apply(&raster);
    if AC {
        deq[0] = dc;
    }
    reconstruct_4x4_into(&deq, pred, p_off, p_stride, rec, r_off, r_stride);
}
#[inline]
pub fn reconstruct_4x4_dc_into(
    rval: i32,
    pred: &[u8],
    p_off: usize,
    p_stride: usize,
    rec: &mut [u8],
    r_off: usize,
    r_stride: usize,
) {
    if abl_recon() {
        for r in 0..4 {
            let src = &pred[p_off + r * p_stride..][..4];
            rec[r_off + r * r_stride..][..4].copy_from_slice(src);
        }
        return;
    }
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Reconstruct);
    for r in 0..4 {
        let src = &pred[p_off + r * p_stride..][..4];
        let dst = &mut rec[r_off + r * r_stride..][..4];
        for c in 0..4 {
            dst[c] = clip_u8(src[c] as i32 + rval);
        }
    }
}

#[inline]
pub fn add_residual_4x4(res: &[i32; 16], pred: &[i32; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = clip_u8(pred[i] + res[i]);
    }
    out
}
pub fn intra8x8_pred(
    mode: u8,
    avail_top: bool,
    avail_left: bool,
    avail_corner: bool,
    top: &[u8; 16],
    left: &[u8; 8],
    corner: u8,
) -> [u8; 64] {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::IntraPred);
    let t = |k: usize| top[k] as i32;
    let l = |k: usize| left[k] as i32;
    let cc = corner as i32;
    let mut ft = [0i32; 16];
    ft[0] = if avail_corner {
        (cc + 2 * t(0) + t(1) + 2) >> 2
    } else {
        (3 * t(0) + t(1) + 2) >> 2
    };
    for k in 1..15 {
        ft[k] = (t(k - 1) + 2 * t(k) + t(k + 1) + 2) >> 2;
    }
    ft[15] = (t(14) + 3 * t(15) + 2) >> 2;
    let mut fl = [0i32; 8];
    fl[0] = if avail_corner {
        (cc + 2 * l(0) + l(1) + 2) >> 2
    } else {
        (3 * l(0) + l(1) + 2) >> 2
    };
    for k in 1..7 {
        fl[k] = (l(k - 1) + 2 * l(k) + l(k + 1) + 2) >> 2;
    }
    fl[7] = (l(6) + 3 * l(7) + 2) >> 2;
    let fc = if avail_corner {
        if avail_top && avail_left {
            (t(0) + 2 * cc + l(0) + 2) >> 2
        } else if avail_top {
            (3 * cc + t(0) + 2) >> 2
        } else if avail_left {
            (3 * cc + l(0) + 2) >> 2
        } else {
            cc
        }
    } else {
        cc
    };
    let ttf = |k: i32| -> i32 { if k < 0 { fc } else { ft[k as usize] } };
    let llf = |k: i32| -> i32 { if k < 0 { fc } else { fl[k as usize] } };
    let mut p = [0i32; 64];
    match mode {
        0 => {
            for y in 0..8 {
                for x in 0..8 {
                    p[y * 8 + x] = ft[x];
                }
            }
        }
        1 => {
            for y in 0..8 {
                for x in 0..8 {
                    p[y * 8 + x] = fl[y];
                }
            }
        }
        2 => {
            let st: i32 = ft[0..8].iter().sum();
            let sl: i32 = fl[0..8].iter().sum();
            let v = if avail_top && avail_left {
                (st + sl + 8) >> 4
            } else if avail_top {
                (st + 4) >> 3
            } else if avail_left {
                (sl + 4) >> 3
            } else {
                128
            };
            p.fill(v);
        }
        3 => {
            for y in 0..8 {
                for x in 0..8 {
                    p[y * 8 + x] = if x == 7 && y == 7 {
                        (ft[14] + 3 * ft[15] + 2) >> 2
                    } else {
                        (ft[x + y] + 2 * ft[x + y + 1] + ft[x + y + 2] + 2) >> 2
                    };
                }
            }
        }
        4 => {
            for y in 0..8i32 {
                for x in 0..8i32 {
                    p[(y * 8 + x) as usize] = if x > y {
                        (ttf(x - y - 2) + 2 * ttf(x - y - 1) + ttf(x - y) + 2) >> 2
                    } else if x < y {
                        (llf(y - x - 2) + 2 * llf(y - x - 1) + llf(y - x) + 2) >> 2
                    } else {
                        (ft[0] + 2 * fc + fl[0] + 2) >> 2
                    };
                }
            }
        }
        5 => {
            for y in 0..8i32 {
                for x in 0..8i32 {
                    let zvr = 2 * x - y;
                    let k = x - (y >> 1);
                    p[(y * 8 + x) as usize] = if zvr >= 0 && zvr % 2 == 0 {
                        (ttf(k - 1) + ttf(k) + 1) >> 1
                    } else if zvr >= 0 {
                        (ttf(k - 2) + 2 * ttf(k - 1) + ttf(k) + 2) >> 2
                    } else if zvr == -1 {
                        (fl[0] + 2 * fc + ft[0] + 2) >> 2
                    } else {
                        let j = y - 2 * x;
                        (llf(j - 1) + 2 * llf(j - 2) + llf(j - 3) + 2) >> 2
                    };
                }
            }
        }
        6 => {
            for y in 0..8i32 {
                for x in 0..8i32 {
                    let zhd = 2 * y - x;
                    let k = y - (x >> 1);
                    p[(y * 8 + x) as usize] = if zhd >= 0 && zhd % 2 == 0 {
                        (llf(k - 1) + llf(k) + 1) >> 1
                    } else if zhd >= 0 {
                        (llf(k - 2) + 2 * llf(k - 1) + llf(k) + 2) >> 2
                    } else if zhd == -1 {
                        (fl[0] + 2 * fc + ft[0] + 2) >> 2
                    } else {
                        let j = x - 2 * y;
                        (ttf(j - 1) + 2 * ttf(j - 2) + ttf(j - 3) + 2) >> 2
                    };
                }
            }
        }
        7 => {
            for y in 0..8 {
                for x in 0..8 {
                    let k = x + (y >> 1);
                    p[y * 8 + x] = if y % 2 == 0 {
                        (ft[k] + ft[k + 1] + 1) >> 1
                    } else {
                        (ft[k] + 2 * ft[k + 1] + ft[k + 2] + 2) >> 2
                    };
                }
            }
        }
        _ => {
            for y in 0..8 {
                for x in 0..8 {
                    let zhu = x + 2 * y;
                    let k = y + (x >> 1);
                    p[y * 8 + x] = if zhu < 13 && zhu % 2 == 0 {
                        (fl[k] + fl[k + 1] + 1) >> 1
                    } else if zhu < 13 {
                        (fl[k] + 2 * fl[k + 1] + fl[k + 2] + 2) >> 2
                    } else if zhu == 13 {
                        (fl[6] + 3 * fl[7] + 2) >> 2
                    } else {
                        fl[7]
                    };
                }
            }
        }
    }
    let mut out = [0u8; 64];
    for i in 0..64 {
        out[i] = clip_u8(p[i]);
    }
    out
}
pub fn add_residual_8x8(res: &[i32; 64], pred: &[i32; 64]) -> [u8; 64] {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Reconstruct);
    let mut out = [0u8; 64];
    for i in 0..64 {
        out[i] = clip_u8(pred[i] + res[i]);
    }
    out
}
#[inline]
pub fn nnz_raster_from_z(n: &[u8; 24]) -> [u8; 24] {
    [
        n[0], n[1], n[4], n[5], n[2], n[3], n[6], n[7], n[8], n[9], n[12], n[13], n[10], n[11],
        n[14], n[15], n[16], n[17], n[20], n[21], n[18], n[19], n[22], n[23],
    ]
}
