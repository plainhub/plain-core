use std::vec;
use std::vec::Vec;
# [rustfmt :: skip] const ALPHA : [i32 ; 52] = [0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 4 , 4 , 5 , 6 , 7 , 8 , 9 , 10 , 12 , 13 , 15 , 17 , 20 , 22 , 25 , 28 , 32 , 36 , 40 , 45 , 50 , 56 , 63 , 71 , 80 , 90 , 101 , 113 , 127 , 144 , 162 , 182 , 203 , 226 , 255 , 255 ,] ;
# [rustfmt :: skip] const BETA : [i32 ; 52] = [0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 0 , 2 , 2 , 2 , 3 , 3 , 3 , 3 , 4 , 4 , 4 , 6 , 6 , 7 , 7 , 8 , 8 , 9 , 9 , 10 , 10 , 11 , 11 , 12 , 12 , 13 , 13 , 14 , 14 , 15 , 15 , 16 , 16 , 17 , 17 , 18 , 18 ,] ;
# [rustfmt :: skip] const TC0 : [[i32 ; 3] ; 52] = [[0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 0] , [0 , 0 , 1] , [0 , 0 , 1] , [0 , 0 , 1] , [0 , 0 , 1] , [0 , 1 , 1] , [0 , 1 , 1] , [1 , 1 , 1] , [1 , 1 , 1] , [1 , 1 , 1] , [1 , 1 , 1] , [1 , 1 , 2] , [1 , 1 , 2] , [1 , 1 , 2] , [1 , 1 , 2] , [1 , 2 , 3] , [1 , 2 , 3] , [2 , 2 , 3] , [2 , 2 , 4] , [2 , 3 , 4] , [2 , 3 , 4] , [3 , 3 , 5] , [3 , 4 , 6] , [3 , 4 , 6] , [4 , 5 , 7] , [4 , 5 , 8] , [4 , 6 , 9] , [5 , 7 , 10] , [6 , 8 , 11] , [6 , 8 , 13] , [7 , 10 , 14] , [8 , 11 , 16] , [9 , 12 , 18] , [10 , 13 , 20] , [11 , 15 , 23] , [13 , 17 , 25] ,] ;
#[inline]
fn clip1(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}
#[inline]
fn clip3(lo: i32, hi: i32, v: i32) -> i32 {
    v.clamp(lo, hi)
}
struct Line {
    base: usize,
    step: isize,
}
fn filter_luma_line(plane: &mut [u8], line: &Line, bs: i32, alpha: i32, beta: i32, tc0: i32) {
    let at = |i: isize| -> i32 { plane[(line.base as isize + i * line.step) as usize] as i32 };
    let (p0, p1, p2, p3) = (at(-1), at(-2), at(-3), at(-4));
    let (q0, q1, q2, q3) = (at(0), at(1), at(2), at(3));
    if (p0 - q0).abs() >= alpha || (p1 - p0).abs() >= beta || (q1 - q0).abs() >= beta {
        return;
    }
    let set = |plane: &mut [u8], i: isize, v: u8| {
        plane[(line.base as isize + i * line.step) as usize] = v;
    };
    let ap = (p2 - p0).abs();
    let aq = (q2 - q0).abs();
    if bs < 4 {
        let tc = tc0 + (ap < beta) as i32 + (aq < beta) as i32;
        let delta = clip3(-tc, tc, (((q0 - p0) << 2) + (p1 - q1) + 4) >> 3);
        set(plane, -1, clip1(p0 + delta));
        set(plane, 0, clip1(q0 - delta));
        if ap < beta {
            let d = clip3(-tc0, tc0, (p2 + ((p0 + q0 + 1) >> 1) - (p1 << 1)) >> 1);
            set(plane, -2, clip1(p1 + d));
        }
        if aq < beta {
            let d = clip3(-tc0, tc0, (q2 + ((p0 + q0 + 1) >> 1) - (q1 << 1)) >> 1);
            set(plane, 1, clip1(q1 + d));
        }
    } else {
        let strong = (p0 - q0).abs() < (alpha >> 2) + 2;
        if strong && ap < beta {
            set(
                plane,
                -1,
                clip1((p2 + 2 * p1 + 2 * p0 + 2 * q0 + q1 + 4) >> 3),
            );
            set(plane, -2, clip1((p2 + p1 + p0 + q0 + 2) >> 2));
            set(plane, -3, clip1((2 * p3 + 3 * p2 + p1 + p0 + q0 + 4) >> 3));
        } else {
            set(plane, -1, clip1((2 * p1 + p0 + q1 + 2) >> 2));
        }
        if strong && aq < beta {
            set(
                plane,
                0,
                clip1((q2 + 2 * q1 + 2 * q0 + 2 * p0 + p1 + 4) >> 3),
            );
            set(plane, 1, clip1((q2 + q1 + q0 + p0 + 2) >> 2));
            set(plane, 2, clip1((2 * q3 + 3 * q2 + q1 + q0 + p0 + 4) >> 3));
        } else {
            set(plane, 0, clip1((2 * q1 + q0 + p1 + 2) >> 2));
        }
    }
}
fn filter_luma_line_contig(
    plane: &mut [u8],
    base: usize,
    bs: i32,
    alpha: i32,
    beta: i32,
    tc0: i32,
) {
    let w = &mut plane[base - 4..][..8];
    let (p0, p1, p2, p3) = (w[3] as i32, w[2] as i32, w[1] as i32, w[0] as i32);
    let (q0, q1, q2, q3) = (w[4] as i32, w[5] as i32, w[6] as i32, w[7] as i32);
    if (p0 - q0).abs() >= alpha || (p1 - p0).abs() >= beta || (q1 - q0).abs() >= beta {
        return;
    }
    let ap = (p2 - p0).abs();
    let aq = (q2 - q0).abs();
    if bs < 4 {
        let tc = tc0 + (ap < beta) as i32 + (aq < beta) as i32;
        let delta = clip3(-tc, tc, (((q0 - p0) << 2) + (p1 - q1) + 4) >> 3);
        w[3] = clip1(p0 + delta);
        w[4] = clip1(q0 - delta);
        if ap < beta {
            let d = clip3(-tc0, tc0, (p2 + ((p0 + q0 + 1) >> 1) - (p1 << 1)) >> 1);
            w[2] = clip1(p1 + d);
        }
        if aq < beta {
            let d = clip3(-tc0, tc0, (q2 + ((p0 + q0 + 1) >> 1) - (q1 << 1)) >> 1);
            w[5] = clip1(q1 + d);
        }
    } else {
        let strong = (p0 - q0).abs() < (alpha >> 2) + 2;
        if strong && ap < beta {
            w[3] = clip1((p2 + 2 * p1 + 2 * p0 + 2 * q0 + q1 + 4) >> 3);
            w[2] = clip1((p2 + p1 + p0 + q0 + 2) >> 2);
            w[1] = clip1((2 * p3 + 3 * p2 + p1 + p0 + q0 + 4) >> 3);
        } else {
            w[3] = clip1((2 * p1 + p0 + q1 + 2) >> 2);
        }
        if strong && aq < beta {
            w[4] = clip1((q2 + 2 * q1 + 2 * q0 + 2 * p0 + p1 + 4) >> 3);
            w[5] = clip1((q2 + q1 + q0 + p0 + 2) >> 2);
            w[6] = clip1((2 * q3 + 3 * q2 + q1 + q0 + p0 + 4) >> 3);
        } else {
            w[4] = clip1((2 * q1 + q0 + p1 + 2) >> 2);
        }
    }
}
fn filter_chroma_line_contig(
    plane: &mut [u8],
    base: usize,
    bs: i32,
    alpha: i32,
    beta: i32,
    tc0: i32,
) {
    let w = &mut plane[base - 2..][..4];
    let (p0, p1) = (w[1] as i32, w[0] as i32);
    let (q0, q1) = (w[2] as i32, w[3] as i32);
    if (p0 - q0).abs() >= alpha || (p1 - p0).abs() >= beta || (q1 - q0).abs() >= beta {
        return;
    }
    if bs < 4 {
        let tc = tc0 + 1;
        let delta = clip3(-tc, tc, (((q0 - p0) << 2) + (p1 - q1) + 4) >> 3);
        w[1] = clip1(p0 + delta);
        w[2] = clip1(q0 - delta);
    } else {
        w[1] = clip1((2 * p1 + p0 + q1 + 2) >> 2);
        w[2] = clip1((2 * q1 + q0 + p1 + 2) >> 2);
    }
}
fn filter_chroma_line(plane: &mut [u8], line: &Line, bs: i32, alpha: i32, beta: i32, tc0: i32) {
    let at = |i: isize| -> i32 { plane[(line.base as isize + i * line.step) as usize] as i32 };
    let (p0, p1) = (at(-1), at(-2));
    let (q0, q1) = (at(0), at(1));
    if (p0 - q0).abs() >= alpha || (p1 - p0).abs() >= beta || (q1 - q0).abs() >= beta {
        return;
    }
    let set = |plane: &mut [u8], i: isize, v: u8| {
        plane[(line.base as isize + i * line.step) as usize] = v;
    };
    if bs < 4 {
        let tc = tc0 + 1;
        let delta = clip3(-tc, tc, (((q0 - p0) << 2) + (p1 - q1) + 4) >> 3);
        set(plane, -1, clip1(p0 + delta));
        set(plane, 0, clip1(q0 - delta));
    } else {
        set(plane, -1, clip1((2 * p1 + p0 + q1 + 2) >> 2));
        set(plane, 0, clip1((2 * q1 + q0 + p1 + 2) >> 2));
    }
}
pub struct BlockInfo<'a> {
    pub inter: &'a [bool],
    pub nnz: &'a [u8],
    pub mv: &'a [(i32, i32)],
    pub ref_id: &'a [i32],
    pub mv1: &'a [(i32, i32)],
    pub ref_id1: &'a [i32],
    pub w4: usize,
    pub t8x8: &'a [bool],
    pub bs: &'a [MbBs],
    pub poc0: &'a [i32],
    pub poc1: &'a [i32],
    pub kind: &'a [u8],
}
pub const MB_KIND_UNSET: u8 = 255;
pub const MB_KIND_INTRA: u8 = 0;
pub const MB_KIND_SKIP: u8 = 1;
pub const MB_KIND_INTER_UNIFORM: u8 = 2;
pub const MB_KIND_INTER: u8 = 3;
impl MbKind {
    #[inline]
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            MB_KIND_INTRA => Some(MbKind::Intra),
            MB_KIND_SKIP => Some(MbKind::Skip),
            MB_KIND_INTER_UNIFORM => Some(MbKind::InterUniform),
            MB_KIND_INTER => Some(MbKind::Inter),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct MbBs {
    pub v: [[u8; 4]; 4],
    pub h: [[u8; 4]; 4],
}
impl MbBs {
    pub const UNSET: MbBs = MbBs {
        v: [[0xFF; 4]; 4],
        h: [[0xFF; 4]; 4],
    };
}
const NO_REF: i32 = i32::MIN;
#[derive(Clone, Copy, Default)]
struct Blk {
    inter: bool,
    nz: bool,
    ref_id: i32,
    mvx: i32,
    mvy: i32,
    ref1: i32,
    mv1x: i32,
    mv1y: i32,
}
impl Blk {
    #[inline]
    fn load(info: &BlockInfo, i: usize) -> Self {
        let (mvx, mvy) = info.mv.get(i).copied().unwrap_or((0, 0));
        let (ref1, (mv1x, mv1y)) = if info.ref_id1.is_empty() {
            (NO_REF, (0, 0))
        } else {
            (info.rid1_at(i), info.mv1.get(i).copied().unwrap_or((0, 0)))
        };
        Blk {
            inter: info.inter.get(i).copied().unwrap_or(false),
            nz: info.nnz.get(i).copied().unwrap_or(0) != 0,
            ref_id: info.rid_at(i),
            mvx,
            mvy,
            ref1,
            mv1x,
            mv1y,
        }
    }
    #[inline]
    fn same_motion(&self, o: &Blk) -> bool {
        self.ref_id == o.ref_id
            && self.mvx == o.mvx
            && self.mvy == o.mvy
            && self.ref1 == o.ref1
            && self.mv1x == o.mv1x
            && self.mv1y == o.mv1y
    }
}
#[inline]
fn bs1_tile(p: &Blk, q: &Blk) -> bool {
    if (p.ref1 == NO_REF) & (q.ref1 == NO_REF) {
        let far4 = |d: i32| (d.wrapping_add(3) as u32) >= 7;
        let far = far4(p.mvx - q.mvx) | far4(p.mvy - q.mvy);
        return (p.ref_id != q.ref_id) | ((p.ref_id != NO_REF) & far);
    }
    let used = |b: &Blk| {
        let mut v = [(0i32, (0i32, 0i32)); 2];
        let mut n = 0usize;
        if b.ref_id != NO_REF {
            v[n] = (b.ref_id, (b.mvx, b.mvy));
            n += 1;
        }
        if b.ref1 != NO_REF {
            v[n] = (b.ref1, (b.mv1x, b.mv1y));
            n += 1;
        }
        (v, n)
    };
    let (pv, pn) = used(p);
    let (qv, qn) = used(q);
    if pn != qn {
        return true;
    }
    let far = |a: (i32, i32), b: (i32, i32)| {
        let f = |d: i32| (d.wrapping_add(3) as u32) >= 7;
        f(a.0 - b.0) | f(a.1 - b.1)
    };
    match pn {
        0 => false,
        1 => pv[0].0 != qv[0].0 || far(pv[0].1, qv[0].1),
        _ => {
            let direct = !far(pv[0].1, qv[0].1) && !far(pv[1].1, qv[1].1);
            let swap = !far(pv[0].1, qv[1].1) && !far(pv[1].1, qv[0].1);
            if pv[0].0 == pv[1].0 {
                qv[0].0 != pv[0].0 || qv[1].0 != pv[0].0 || !(direct || swap)
            } else if pv[0].0 == qv[0].0 && pv[1].0 == qv[1].0 {
                !direct
            } else if pv[0].0 == qv[1].0 && pv[1].0 == qv[0].0 {
                !swap
            } else {
                true
            }
        }
    }
}
#[inline]
fn bs_inter(p: &Blk, q: &Blk) -> i32 {
    let nz = p.nz | q.nz;
    let moved = bs1_tile(p, q);
    if nz { 2 } else { moved as i32 }
}
#[inline(always)]
pub fn census_note_kind(kind: MbKind) {
    let _ = kind;
}
#[inline(always)]
pub fn census_note_packed() {}
#[inline]
fn derive_mb_bs(
    tile: &Tile,
    mb_x: usize,
    mb_y: usize,
    flat_inter: bool,
    uniform_motion: bool,
    mb_t8: bool,
    bs_v: &mut [[i32; 4]; 4],
    bs_h: &mut [[i32; 4]; 4],
) {
    let cur_intra = !tile[1][1].inter;
    if mb_x > 0 {
        bs_v[0] = if cur_intra || !tile[1][0].inter {
            [4; 4]
        } else {
            core::array::from_fn(|seg| bs_inter(&tile[seg + 1][0], &tile[seg + 1][1]))
        };
    }
    if mb_y > 0 {
        bs_h[0] = if cur_intra || !tile[0][1].inter {
            [4; 4]
        } else {
            core::array::from_fn(|seg| bs_inter(&tile[0][seg + 1], &tile[1][seg + 1]))
        };
    }
    if flat_inter {
        return;
    }
    for be in 1..4usize {
        if mb_t8 && (be == 1 || be == 3) {
            continue;
        }
        if cur_intra {
            bs_v[be] = [3; 4];
            bs_h[be] = [3; 4];
        } else if uniform_motion {
            bs_v[be] = core::array::from_fn(|seg| {
                2 * (tile[seg + 1][be].nz | tile[seg + 1][be + 1].nz) as i32
            });
            bs_h[be] = core::array::from_fn(|seg| {
                2 * (tile[be][seg + 1].nz | tile[be + 1][seg + 1].nz) as i32
            });
        } else {
            bs_v[be] =
                core::array::from_fn(|seg| bs_inter(&tile[seg + 1][be], &tile[seg + 1][be + 1]));
            bs_h[be] =
                core::array::from_fn(|seg| bs_inter(&tile[be][seg + 1], &tile[be + 1][seg + 1]));
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MbKind {
    Intra,
    Skip,
    InterUniform,
    Inter,
}
#[inline]
pub fn intra_mb_bs(mb_x: usize, mb_y: usize) -> MbBs {
    const fn intra_bs(left: bool, top: bool) -> MbBs {
        MbBs {
            v: [if left { [4; 4] } else { [0; 4] }, [3; 4], [3; 4], [3; 4]],
            h: [if top { [4; 4] } else { [0; 4] }, [3; 4], [3; 4], [3; 4]],
        }
    }
    const INTRA_BS: [[MbBs; 2]; 2] = [
        [intra_bs(false, false), intra_bs(false, true)],
        [intra_bs(true, false), intra_bs(true, true)],
    ];
    INTRA_BS[(mb_x > 0) as usize][(mb_y > 0) as usize]
}
pub fn derive_mb_kind_into(
    info: &BlockInfo,
    mb_x: usize,
    mb_y: usize,
    kind: MbKind,
    bs_v: &mut [[i32; 4]; 4],
    bs_h: &mut [[i32; 4]; 4],
) {
    *bs_v = [[0; 4]; 4];
    *bs_h = [[0; 4]; 4];
    let (bx0, by0) = (mb_x * 4, mb_y * 4);
    let w4 = info.w4;
    match kind {
        MbKind::Intra => {
            if mb_x > 0 {
                bs_v[0] = [4; 4];
            }
            if mb_y > 0 {
                bs_h[0] = [4; 4];
            }
            for e in 1..4 {
                bs_v[e] = [3; 4];
                bs_h[e] = [3; 4];
            }
        }
        MbKind::Skip => {
            let me = Blk::load(info, by0 * w4 + bx0);
            if mb_x > 0 {
                bs_v[0] = core::array::from_fn(|seg| {
                    let p = Blk::load(info, (by0 + seg) * w4 + bx0 - 1);
                    if p.inter { bs_inter(&p, &me) } else { 4 }
                });
            }
            if mb_y > 0 {
                bs_h[0] = core::array::from_fn(|seg| {
                    let p = Blk::load(info, (by0 - 1) * w4 + bx0 + seg);
                    if p.inter { bs_inter(&p, &me) } else { 4 }
                });
            }
        }
        MbKind::InterUniform => {
            let nz: [&[u8]; 4] = core::array::from_fn(|r| &info.nnz[(by0 + r) * w4 + bx0..][..4]);
            for e in 1..4usize {
                bs_v[e] = core::array::from_fn(|seg| {
                    2 * ((nz[seg][e] != 0) | (nz[seg][e - 1] != 0)) as i32
                });
                bs_h[e] = core::array::from_fn(|seg| {
                    2 * ((nz[e][seg] != 0) | (nz[e - 1][seg] != 0)) as i32
                });
            }
            if mb_x > 0 {
                bs_v[0] = core::array::from_fn(|seg| {
                    let qi = (by0 + seg) * w4 + bx0;
                    let p = Blk::load(info, qi - 1);
                    if p.inter {
                        bs_inter(&p, &Blk::load(info, qi))
                    } else {
                        4
                    }
                });
            }
            if mb_y > 0 {
                bs_h[0] = core::array::from_fn(|seg| {
                    let qi = by0 * w4 + bx0 + seg;
                    let p = Blk::load(info, qi - w4);
                    if p.inter {
                        bs_inter(&p, &Blk::load(info, qi))
                    } else {
                        4
                    }
                });
            }
        }
        MbKind::Inter => {
            let m = derive_mb(info, mb_x, mb_y, false);
            for e in 0..4 {
                for sg in 0..4 {
                    bs_v[e][sg] = m.v[e][sg] as i32;
                    bs_h[e][sg] = m.h[e][sg] as i32;
                }
            }
        }
    }
}
pub fn derive_mb_kind(info: &BlockInfo, mb_x: usize, mb_y: usize, kind: MbKind) -> MbBs {
    census_note_kind(kind);
    let (bx0, by0) = (mb_x * 4, mb_y * 4);
    let w4 = info.w4;
    match kind {
        MbKind::Intra => intra_mb_bs(mb_x, mb_y),
        MbKind::Skip => {
            let mut m = MbBs::default();
            let me = Blk::load(info, by0 * w4 + bx0);
            if mb_x > 0 {
                m.v[0] = core::array::from_fn(|seg| {
                    let p = Blk::load(info, (by0 + seg) * w4 + bx0 - 1);
                    if p.inter { bs_inter(&p, &me) as u8 } else { 4 }
                });
            }
            if mb_y > 0 {
                m.h[0] = core::array::from_fn(|seg| {
                    let p = Blk::load(info, (by0 - 1) * w4 + bx0 + seg);
                    if p.inter { bs_inter(&p, &me) as u8 } else { 4 }
                });
            }
            m
        }
        MbKind::InterUniform => {
            let mut m = MbBs::default();
            let nz: [&[u8]; 4] = core::array::from_fn(|r| &info.nnz[(by0 + r) * w4 + bx0..][..4]);
            for e in 1..4usize {
                m.v[e] = core::array::from_fn(|seg| {
                    2 * ((nz[seg][e] != 0) | (nz[seg][e - 1] != 0)) as u8
                });
                m.h[e] = core::array::from_fn(|seg| {
                    2 * ((nz[e][seg] != 0) | (nz[e - 1][seg] != 0)) as u8
                });
            }
            if mb_x > 0 {
                m.v[0] = core::array::from_fn(|seg| {
                    let qi = (by0 + seg) * w4 + bx0;
                    let p = Blk::load(info, qi - 1);
                    if p.inter {
                        bs_inter(&p, &Blk::load(info, qi)) as u8
                    } else {
                        4
                    }
                });
            }
            if mb_y > 0 {
                m.h[0] = core::array::from_fn(|seg| {
                    let qi = by0 * w4 + bx0 + seg;
                    let p = Blk::load(info, qi - w4);
                    if p.inter {
                        bs_inter(&p, &Blk::load(info, qi)) as u8
                    } else {
                        4
                    }
                });
            }
            m
        }
        MbKind::Inter => derive_mb(info, mb_x, mb_y, false),
    }
}
#[derive(Clone, Copy)]
#[repr(C, align(32))]
pub struct MbPack {
    pub nnz_mask: u16,
    pub inter: bool,
    _pad: u8,
    pub mvx: [i16; 16],
    pub mvy: [i16; 16],
    pub l1_used: u16,
    pub mvx1: [i16; 16],
    pub mvy1: [i16; 16],
    pub ref1: [i32; 16],
    pub ref_id: [i32; 16],
}
impl BlockInfo<'_> {
    #[inline]
    fn inter_at(&self, i: usize) -> bool {
        self.inter.get(i).copied().unwrap_or(false)
    }
    #[inline]
    fn nnz_at(&self, i: usize) -> u8 {
        self.nnz.get(i).copied().unwrap_or(0)
    }
    #[inline]
    fn ref_at(&self, i: usize) -> i32 {
        self.ref_id.get(i).copied().unwrap_or(NO_REF)
    }
    #[inline]
    fn ref1_at(&self, i: usize) -> i32 {
        self.ref_id1.get(i).copied().unwrap_or(NO_REF)
    }
    #[inline]
    fn mv_at(&self, i: usize) -> (i32, i32) {
        self.mv.get(i).copied().unwrap_or((0, 0))
    }
    #[inline]
    fn mv1_at(&self, i: usize) -> (i32, i32) {
        self.mv1.get(i).copied().unwrap_or((0, 0))
    }
    #[inline]
    fn rid_at(&self, i: usize) -> i32 {
        let r = self.ref_id.get(i).copied().unwrap_or(NO_REF);
        if self.poc0.is_empty() {
            r
        } else if r >= 0 {
            self.poc0.get(r as usize).copied().unwrap_or(NO_REF)
        } else {
            NO_REF
        }
    }
    #[inline]
    fn rid1_at(&self, i: usize) -> i32 {
        let r = self.ref_id1.get(i).copied().unwrap_or(NO_REF);
        if self.poc1.is_empty() {
            r
        } else if r >= 0 {
            self.poc1.get(r as usize).copied().unwrap_or(NO_REF)
        } else {
            NO_REF
        }
    }
}
impl Default for MbPack {
    fn default() -> Self {
        Self {
            nnz_mask: 0,
            inter: false,
            _pad: 0,
            mvx: [0; 16],
            mvy: [0; 16],
            l1_used: 0,
            mvx1: [0; 16],
            mvy1: [0; 16],
            ref1: [NO_REF; 16],
            ref_id: [NO_REF; 16],
        }
    }
}

pub fn pack_frame_into(info: &BlockInfo, mb_w: usize, mb_h: usize, out: &mut Vec<MbPack>) {
    let has1 = !info.ref_id1.is_empty();
    out.clear();
    out.reserve(mb_w * mb_h);
    for mb_y in 0..mb_h {
        for mb_x in 0..mb_w {
            out.push(pack_mb(info, has1, mb_x, mb_y));
        }
    }
}
#[inline]
fn map_ref(mapped: bool, poc: &[i32], r: i32) -> i32 {
    if !mapped {
        r
    } else if r >= 0 {
        poc.get(r as usize).copied().unwrap_or(NO_REF)
    } else {
        NO_REF
    }
}
#[inline]
pub fn pack_mb(info: &BlockInfo, has1: bool, mb_x: usize, mb_y: usize) -> MbPack {
    let mut rec = MbPack::default();
    let (bx0, by0) = (mb_x * 4, mb_y * 4);
    let w4 = info.w4;
    let base = by0 * w4 + bx0;
    rec.inter = info.inter.get(base).copied().unwrap_or(false);
    let map0 = !info.poc0.is_empty();
    let map1 = !info.poc1.is_empty();
    for r in 0..4 {
        let row = base + r * w4;
        let nnz = &info.nnz[row..row + 4];
        let mvr = &info.mv[row..row + 4];
        let rid = &info.ref_id[row..row + 4];
        for c in 0..4 {
            let k = r * 4 + c;
            rec.nnz_mask |= ((nnz[c] != 0) as u16) << k;
            rec.mvx[k] = mvr[c].0 as i16;
            rec.mvy[k] = mvr[c].1 as i16;
        }
        if map0 {
            for c in 0..4 {
                let r0 = rid[c];
                rec.ref_id[r * 4 + c] = if r0 >= 0 {
                    info.poc0.get(r0 as usize).copied().unwrap_or(NO_REF)
                } else {
                    NO_REF
                };
            }
        } else {
            for c in 0..4 {
                rec.ref_id[r * 4 + c] = rid[c];
            }
        }
        if has1 {
            let mv1r = &info.mv1[row..row + 4];
            let rid1 = &info.ref_id1[row..row + 4];
            for c in 0..4 {
                let k = r * 4 + c;
                let r1 = map_ref(map1, info.poc1, rid1[c]);
                rec.ref1[k] = r1;
                rec.mvx1[k] = mv1r[c].0 as i16;
                rec.mvy1[k] = mv1r[c].1 as i16;
                rec.l1_used |= ((r1 != NO_REF) as u16) << k;
            }
        }
    }
    rec
}
pub fn precompute_bs_frame(info: &BlockInfo, mb_w: usize, mb_h: usize, out: &mut Vec<MbBs>) {
    let has1 = !info.ref_id1.is_empty();
    out.clear();
    out.reserve(mb_w * mb_h);
    let mut prev_row: Vec<MbPack> = Vec::with_capacity(mb_w);
    let mut cur_row: Vec<MbPack> = Vec::with_capacity(mb_w);
    for mb_y in 0..mb_h {
        cur_row.clear();
        for mb_x in 0..mb_w {
            cur_row.push(pack_mb(info, has1, mb_x, mb_y));
            let Some((cur, before)) = cur_row.split_last() else {
                continue;
            };
            let left = if mb_x > 0 { before.last() } else { None };
            let top = if mb_y > 0 { prev_row.get(mb_x) } else { None };
            let mb_t8 = info.t8x8.get(mb_y * mb_w + mb_x).copied().unwrap_or(false);
            let (mut bv, mut bh) = ([[0i32; 4]; 4], [[0i32; 4]; 4]);
            derive_mb_records(cur, left, top, mb_t8, &mut bv, &mut bh);
            let mut m = MbBs {
                v: [[0; 4]; 4],
                h: [[0; 4]; 4],
            };
            for e in 0..4 {
                for sg in 0..4 {
                    m.v[e][sg] = bv[e][sg] as u8;
                    m.h[e][sg] = bh[e][sg] as u8;
                }
            }
            out.push(m);
        }
        core::mem::swap(&mut prev_row, &mut cur_row);
    }
}
thread_local! { static PACK_SCRATCH : core :: cell :: Cell < Vec < MbPack >> = const { core :: cell :: Cell :: new (Vec :: new ()) } ; }
#[inline]
pub fn mb_uniform_scalar(p: &MbPack) -> bool {
    p.inter
        && (1..16).all(|k| {
            p.ref_id[k] == p.ref_id[0]
                && p.mvx[k] == p.mvx[0]
                && p.mvy[k] == p.mvy[0]
                && p.ref1[k] == p.ref1[0]
                && p.mvx1[k] == p.mvx1[0]
                && p.mvy1[k] == p.mvy1[0]
        })
}
#[inline]
pub fn mb_uniform(p: &MbPack) -> bool {
    mb_uniform_scalar(p)
}
#[inline]
pub fn bs_motion_masks_scalar(p: &MbPack) -> (u16, u16) {
    let differs = |a: usize, b: usize| -> bool { pk_differs(p, a, p, b) };
    let (mut left, mut up) = (0u16, 0u16);
    for k in 0..16usize {
        if k % 4 != 0 && differs(k, k - 1) {
            left |= 1 << k;
        }
        if k >= 4 && differs(k, k - 4) {
            up |= 1 << k;
        }
    }
    (left & 0xEEEE, up & 0xFFF0)
}
#[inline]
pub fn bs_motion_masks(p: &MbPack) -> (u16, u16) {
    bs_motion_masks_scalar(p)
}
#[inline(always)]
fn pk_differs(p: &MbPack, pk: usize, q: &MbPack, qk: usize) -> bool {
    let p_has1 = p.ref1[pk] != NO_REF;
    let q_has1 = q.ref1[qk] != NO_REF;
    if !p_has1 && !q_has1 {
        let far4 = |d: i16| (d.wrapping_add(3) as u16) >= 7;
        let far = far4(p.mvx[pk] - q.mvx[qk]) | far4(p.mvy[pk] - q.mvy[qk]);
        return (p.ref_id[pk] != q.ref_id[qk]) | ((p.ref_id[pk] != NO_REF) & far);
    }
    pk_differs_two_list(p, pk, q, qk)
}
#[inline(never)]
fn pk_differs_two_list(p: &MbPack, pk: usize, q: &MbPack, qk: usize) -> bool {
    let used = |b: &MbPack, k: usize| {
        let k = k & 15;
        let mut v = [(0i32, (0i32, 0i32)); 2];
        let mut n = 0usize;
        if b.ref_id[k] != NO_REF {
            v[n] = (b.ref_id[k], (b.mvx[k] as i32, b.mvy[k] as i32));
            n += 1;
        }
        if b.ref1[k] != NO_REF {
            v[n] = (b.ref1[k], (b.mvx1[k] as i32, b.mvy1[k] as i32));
            n += 1;
        }
        (v, n)
    };
    let (pv, pn) = used(p, pk);
    let (qv, qn) = used(q, qk);
    if pn != qn {
        return true;
    }
    let far = |a: (i32, i32), b: (i32, i32)| {
        let f = |d: i32| (d.wrapping_add(3) as u32) >= 7;
        f(a.0 - b.0) | f(a.1 - b.1)
    };
    match pn {
        0 => false,
        1 => pv[0].0 != qv[0].0 || far(pv[0].1, qv[0].1),
        _ => {
            let direct = !far(pv[0].1, qv[0].1) && !far(pv[1].1, qv[1].1);
            let swap = !far(pv[0].1, qv[1].1) && !far(pv[1].1, qv[0].1);
            if pv[0].0 == pv[1].0 {
                qv[0].0 != pv[0].0 || qv[1].0 != pv[0].0 || !(direct || swap)
            } else if pv[0].0 == qv[0].0 && pv[1].0 == qv[1].0 {
                !direct
            } else if pv[0].0 == qv[1].0 && pv[1].0 == qv[0].0 {
                !swap
            } else {
                true
            }
        }
    }
}
#[inline]
fn pk_nz(p: &MbPack, k: usize) -> bool {
    (p.nnz_mask >> k) & 1 != 0
}
#[inline]
fn pk_bs_inter(p: &MbPack, pk: usize, q: &MbPack, qk: usize) -> i32 {
    if pk_nz(p, pk) | pk_nz(q, qk) {
        return 2;
    }
    pk_differs(p, pk, q, qk) as i32
}
pub fn derive_mb_packed(
    packs: &[MbPack],
    mb_w: usize,
    mb_x: usize,
    mb_y: usize,
    mb_t8: bool,
    bs_v: &mut [[i32; 4]; 4],
    bs_h: &mut [[i32; 4]; 4],
) -> bool {
    let row = &packs[mb_y * mb_w..][..mb_w];
    let Some(cur) = row.get(mb_x) else {
        return false;
    };
    let left = if mb_x > 0 { row.get(mb_x - 1) } else { None };
    let top = if mb_y > 0 {
        packs[(mb_y - 1) * mb_w..][..mb_w].get(mb_x)
    } else {
        None
    };
    derive_mb_records(cur, left, top, mb_t8, bs_v, bs_h)
}
pub fn derive_mb_records(
    cur: &MbPack,
    left: Option<&MbPack>,
    top: Option<&MbPack>,
    mb_t8: bool,
    bs_v: &mut [[i32; 4]; 4],
    bs_h: &mut [[i32; 4]; 4],
) -> bool {
    let cur_intra = !cur.inter;
    let uniform = mb_uniform(cur);
    let flat_inter = uniform && cur.nnz_mask == 0;
    if let Some(l) = left {
        bs_v[0] = if cur_intra || !l.inter {
            [4; 4]
        } else {
            core::array::from_fn(|seg| pk_bs_inter(l, seg * 4 + 3, cur, seg * 4))
        };
    }
    if let Some(t) = top {
        bs_h[0] = if cur_intra || !t.inter {
            [4; 4]
        } else {
            core::array::from_fn(|seg| pk_bs_inter(t, 12 + seg, cur, seg))
        };
    }
    if flat_inter {
        return true;
    }
    let masks = if cur_intra || uniform {
        (0, 0)
    } else {
        bs_motion_masks(cur)
    };
    for be in 1..4usize {
        if mb_t8 && (be == 1 || be == 3) {
            continue;
        }
        if cur_intra {
            bs_v[be] = [3; 4];
            bs_h[be] = [3; 4];
        } else if uniform {
            bs_v[be] = core::array::from_fn(|seg| {
                2 * (pk_nz(cur, seg * 4 + be) | pk_nz(cur, seg * 4 + be - 1)) as i32
            });
            bs_h[be] = core::array::from_fn(|seg| {
                2 * (pk_nz(cur, be * 4 + seg) | pk_nz(cur, (be - 1) * 4 + seg)) as i32
            });
        } else {
            let (left, up) = masks;
            bs_v[be] = core::array::from_fn(|seg| {
                let k = seg * 4 + be;
                if pk_nz(cur, k) | pk_nz(cur, k - 1) {
                    2
                } else {
                    ((left >> k) & 1) as i32
                }
            });
            bs_h[be] = core::array::from_fn(|seg| {
                let k = be * 4 + seg;
                if pk_nz(cur, k) | pk_nz(cur, k - 4) {
                    2
                } else {
                    ((up >> k) & 1) as i32
                }
            });
        }
    }
    false
}
#[inline]
fn scan_uniform_flat(tile: &Tile) -> (bool, bool) {
    let b0 = &tile[1][1];
    if !b0.inter {
        return (false, false);
    }
    let mut any_nz = false;
    for r in 1..5 {
        for c in 1..5 {
            let b = &tile[r][c];
            if !b.inter || !b.same_motion(b0) {
                return (false, false);
            }
            any_nz |= b.nz;
        }
    }
    (true, !any_nz)
}
#[inline]
fn scan_two_pass(tile: &Tile) -> (bool, bool) {
    let b0 = &tile[1][1];
    let flat = b0.inter
        && (1..5).all(|r| {
            (1..5).all(|c| {
                let b = &tile[r][c];
                b.inter && !b.nz && b.same_motion(b0)
            })
        });
    let uniform =
        b0.inter && (1..5).all(|r| (1..5).all(|c| tile[r][c].inter && tile[r][c].same_motion(b0)));
    (uniform, flat)
}
fn bs_twopass() -> bool {
    {
        return false;
    }
}
fn verify_kind() -> bool {
    {
        return false;
    }
}
fn verify_kind_matches_blind(
    info: &BlockInfo,
    mb_x: usize,
    mb_y: usize,
    mb_t8: bool,
    kind: MbKind,
    flat_inter: bool,
    bs_v: &[[i32; 4]; 4],
    bs_h: &[[i32; 4]; 4],
) {
    let tile = gather_tile(info, mb_x, mb_y);
    let (u, f) = scan_uniform_flat(&tile);
    let (mut vv, mut vh) = ([[0i32; 4]; 4], [[0i32; 4]; 4]);
    derive_mb_bs(&tile, mb_x, mb_y, f, u, mb_t8, &mut vv, &mut vh);
    assert_eq!(
        flat_inter, f,
        "MB ({mb_x},{mb_y}) kind {kind:?}: flat_inter {flat_inter} but blind derived {f}"
    );
    for be in 0..4usize {
        if be == 0 {
            if mb_x > 0 {
                assert_eq!(
                    bs_v[0], vv[0],
                    "MB ({mb_x},{mb_y}) kind {kind:?}: left MB edge"
                );
            }
            if mb_y > 0 {
                assert_eq!(
                    bs_h[0], vh[0],
                    "MB ({mb_x},{mb_y}) kind {kind:?}: top MB edge"
                );
            }
            continue;
        }
        if flat_inter || (mb_t8 && (be == 1 || be == 3)) {
            continue;
        }
        assert_eq!(
            bs_v[be], vv[be],
            "MB ({mb_x},{mb_y}) kind {kind:?}: v internal edge {be}"
        );
        assert_eq!(
            bs_h[be], vh[be],
            "MB ({mb_x},{mb_y}) kind {kind:?}: h internal edge {be}"
        );
    }
}
fn verify_packed() -> bool {
    {
        return false;
    }
}
fn bs_packed_on() -> bool {
    {
        return true;
    }
}
fn kind_gate_off() -> bool {
    {
        return false;
    }
}
#[inline]
fn scan_predicates(tile: &Tile, two_pass: bool) -> (bool, bool) {
    if two_pass {
        scan_two_pass(tile)
    } else {
        scan_uniform_flat(tile)
    }
}
pub fn derive_mb(info: &BlockInfo, mb_x: usize, mb_y: usize, mb_t8: bool) -> MbBs {
    let tile = gather_tile(info, mb_x, mb_y);
    let (uniform_motion, flat_inter) = scan_predicates(&tile, bs_twopass());
    let (mut bs_v, mut bs_h) = ([[0i32; 4]; 4], [[0i32; 4]; 4]);
    derive_mb_bs(
        &tile,
        mb_x,
        mb_y,
        flat_inter,
        uniform_motion,
        mb_t8,
        &mut bs_v,
        &mut bs_h,
    );
    let pack = |a: [[i32; 4]; 4]| a.map(|e| e.map(|x| x as u8));
    MbBs {
        v: pack(bs_v),
        h: pack(bs_h),
    }
}
type Tile = [[Blk; 5]; 5];
fn gather_tile(info: &BlockInfo, mb_x: usize, mb_y: usize) -> Tile {
    let mut t: Tile = Default::default();
    let (bx0, by0) = (mb_x * 4, mb_y * 4);
    for r in 0..4 {
        let row = (by0 + r) * info.w4 + bx0;
        for c in 0..4 {
            t[r + 1][c + 1] = Blk::load(info, row + c);
        }
    }
    if mb_x > 0 {
        for r in 0..4 {
            t[r + 1][0] = Blk::load(info, (by0 + r) * info.w4 + bx0 - 1);
        }
    }
    if mb_y > 0 {
        let row = (by0 - 1) * info.w4 + bx0;
        for c in 0..4 {
            t[0][c + 1] = Blk::load(info, row + c);
        }
    }
    debug_assert!(
        (1..5).all(|r| (1..5).all(|c| t[r][c].inter == t[1][1].inter)),
        "macroblock ({mb_x},{mb_y}) mixes intra and inter 4x4 blocks"
    );
    t
}
static BS_MODE: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);
fn branchless_bs() -> bool {
    {
        return true;
    }
}
fn deblock_tile() -> bool {
    branchless_bs()
}
static BS_PRECOMP: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);

impl BlockInfo<'_> {
    #[inline]
    fn at(&self, bx: usize, by: usize) -> usize {
        by * self.w4 + bx
    }
    #[inline]
    fn bs(&self, p: usize, q: usize, mb_edge: bool) -> i32 {
        if branchless_bs() {
            self.bs_branchless(p, q, mb_edge)
        } else {
            self.bs_branchy(p, q, mb_edge)
        }
    }
    fn bs_branchy(&self, p: usize, q: usize, mb_edge: bool) -> i32 {
        if !self.inter_at(p) || !self.inter_at(q) {
            if mb_edge { 4 } else { 3 }
        } else if self.nnz_at(p) > 0 || self.nnz_at(q) > 0 {
            2
        } else if self.inter_bs1(p, q) {
            1
        } else {
            0
        }
    }
    #[inline]
    fn bs_branchless(&self, p: usize, q: usize, mb_edge: bool) -> i32 {
        let intra = !(self.inter_at(p) & self.inter_at(q));
        let nz = (self.nnz_at(p) | self.nnz_at(q)) != 0;
        let moved = self.inter_bs1(p, q);
        let intra_bs = if mb_edge { 4 } else { 3 };
        let motion_bs = moved as i32;
        let non_intra = if nz { 2 } else { motion_bs };
        if intra { intra_bs } else { non_intra }
    }
    fn inter_bs1(&self, p: usize, q: usize) -> bool {
        if self.ref_id1.is_empty() {
            let (rp, rq) = (self.ref_at(p), self.ref_at(q));
            let (a, b) = (self.mv_at(p), self.mv_at(q));
            let f4 = |d: i32| (d.wrapping_add(3) as u32) >= 7;
            let far = f4(a.0 - b.0) | f4(a.1 - b.1);
            return (rp != rq) | ((rp != NO_REF) & far);
        }
        let used = |i: usize| {
            let mut v = [(0i32, (0i32, 0i32)); 2];
            let mut n = 0;
            if self.ref_at(i) != NO_REF {
                v[n] = (self.ref_at(i), self.mv_at(i));
                n += 1;
            }
            if !self.ref_id1.is_empty() && self.ref1_at(i) != NO_REF {
                v[n] = (self.ref1_at(i), self.mv1_at(i));
                n += 1;
            }
            (v, n)
        };
        let (pv, pn) = used(p);
        let (qv, qn) = used(q);
        if pn != qn {
            return true;
        }
        let far = |a: (i32, i32), b: (i32, i32)| {
            let f = |d: i32| (d.wrapping_add(3) as u32) >= 7;
            f(a.0 - b.0) | f(a.1 - b.1)
        };
        match pn {
            0 => false,
            1 => pv[0].0 != qv[0].0 || far(pv[0].1, qv[0].1),
            _ => {
                let direct = !far(pv[0].1, qv[0].1) && !far(pv[1].1, qv[1].1);
                let swap = !far(pv[0].1, qv[1].1) && !far(pv[1].1, qv[0].1);
                if pv[0].0 == pv[1].0 {
                    qv[0].0 != pv[0].0 || qv[1].0 != pv[0].0 || !(direct || swap)
                } else if pv[0].0 == qv[0].0 && pv[1].0 == qv[1].0 {
                    !direct
                } else if pv[0].0 == qv[1].0 && pv[1].0 == qv[0].0 {
                    !swap
                } else {
                    true
                }
            }
        }
    }
}
#[inline]
fn thresholds(qpav: i32, offset_a: i32, offset_b: i32) -> (i32, i32, [i32; 3]) {
    let ia = (qpav + offset_a).clamp(0, 51) as usize;
    let ib = (qpav + offset_b).clamp(0, 51) as usize;
    (ALPHA[ia], BETA[ib], TC0[ia])
}

#[inline(never)]
fn derive_mb_general(
    info: &BlockInfo,
    mb_x: usize,
    mb_y: usize,
    mb_w: usize,
    mb_t8: bool,
    use_tile: bool,
    kind_off: bool,
    two_pass: bool,
    verify_kinds: bool,
    packs: Option<&Vec<MbPack>>,
    bs_v: &mut [[i32; 4]; 4],
    bs_h: &mut [[i32; 4]; 4],
) -> bool {
    let fast_kind = if use_tile && !kind_off {
        match info
            .kind
            .get(mb_y * mb_w + mb_x)
            .copied()
            .and_then(MbKind::from_u8)
        {
            Some(k) if k != MbKind::Inter => Some(k),
            _ => None,
        }
    } else {
        None
    };
    let blind_tile = use_tile && fast_kind.is_none() && packs.is_none();
    let packed_mb = packs.filter(|_| use_tile && fast_kind.is_none());
    let tile = if blind_tile {
        Some(gather_tile(info, mb_x, mb_y))
    } else {
        None
    };
    let mut uniform_motion = false;
    let flat_inter = if let Some(k) = fast_kind {
        match k {
            MbKind::Skip => true,
            MbKind::InterUniform => {
                let (bx0, by0) = (mb_x * 4, mb_y * 4);
                (0..4).all(|r| {
                    info.nnz[(by0 + r) * info.w4 + bx0..][..4]
                        .iter()
                        .all(|&n| n == 0)
                })
            }
            _ => false,
        }
    } else if let Some(pk) = packed_mb {
        let pflat = derive_mb_packed(pk, mb_w, mb_x, mb_y, mb_t8, bs_v, bs_h);
        if verify_packed() {
            let t = gather_tile(info, mb_x, mb_y);
            let (u, f) = scan_uniform_flat(&t);
            let (mut tv, mut th) = ([[0i32; 4]; 4], [[0i32; 4]; 4]);
            derive_mb_bs(&t, mb_x, mb_y, f, u, mb_t8, &mut tv, &mut th);
            assert_eq!(pflat, f, "MB ({mb_x},{mb_y}) t8={mb_t8}: flat_inter");
            assert_eq!(*bs_v, tv, "MB ({mb_x},{mb_y}) t8={mb_t8} flat={f}: bs_v");
            assert_eq!(*bs_h, th, "MB ({mb_x},{mb_y}) t8={mb_t8} flat={f}: bs_h");
        }
        pflat
    } else if blind_tile {
        let (u, f) = scan_predicates(tile.as_ref().unwrap(), two_pass);
        uniform_motion = u;
        f
    } else {
        let has1 = !info.ref_id1.is_empty();
        let mut ok = true;
        let mut anchor: Option<(i32, (i32, i32), i32, (i32, i32))> = None;
        'scan: for by in 0..4 {
            let row = (mb_y * 4 + by) * info.w4 + mb_x * 4;
            let inter = &info.inter[row..][..4];
            let nnz = &info.nnz[row..][..4];
            let rid = &info.ref_id[row..][..4];
            let mvr = &info.mv[row..][..4];
            let (rid1, mv1r): (&[i32], &[(i32, i32)]) = if has1 {
                (&info.ref_id1[row..][..4], &info.mv1[row..][..4])
            } else {
                (&[], &[])
            };
            let (r0, m0, r10, m10) = *anchor.get_or_insert_with(|| {
                (
                    rid[0],
                    mvr[0],
                    if has1 { rid1[0] } else { NO_REF },
                    if has1 { mv1r[0] } else { (0, 0) },
                )
            });
            for bx in 0..4 {
                if !inter[bx] || nnz[bx] != 0 || rid[bx] != r0 || mvr[bx] != m0 {
                    ok = false;
                    break 'scan;
                }
                if has1 && (rid1[bx] != r10 || mv1r[bx] != m10) {
                    ok = false;
                    break 'scan;
                }
            }
        }
        ok
    };
    if packed_mb.is_some() {
    } else if let Some(k) = fast_kind {
        derive_mb_kind_into(info, mb_x, mb_y, k, bs_v, bs_h);
        if verify_kinds {
            verify_kind_matches_blind(info, mb_x, mb_y, mb_t8, k, flat_inter, &*bs_v, &*bs_h);
        }
    } else if blind_tile {
        derive_mb_bs(
            tile.as_ref().unwrap(),
            mb_x,
            mb_y,
            flat_inter,
            uniform_motion,
            mb_t8,
            bs_v,
            bs_h,
        );
    }
    flat_inter
}
pub fn filter_frame_rows_pre(
    y: &mut [u8],
    u: &mut [u8],
    v: &mut [u8],
    mb_w: usize,
    mb_h: usize,
    rows: core::ops::Range<usize>,
    mb_qp: &[u8],
    chroma_qp_offset: i32,
    offset_a: i32,
    offset_b: i32,
    info: &BlockInfo,
) {
    filter_frame_rows_impl::<true>(
        y,
        u,
        v,
        mb_w,
        mb_h,
        rows,
        mb_qp,
        chroma_qp_offset,
        offset_a,
        offset_b,
        info,
    )
}
pub fn filter_frame_rows(
    y: &mut [u8],
    u: &mut [u8],
    v: &mut [u8],
    mb_w: usize,
    mb_h: usize,
    rows: core::ops::Range<usize>,
    mb_qp: &[u8],
    chroma_qp_offset: i32,
    offset_a: i32,
    offset_b: i32,
    info: &BlockInfo,
) {
    filter_frame_rows_impl::<false>(
        y,
        u,
        v,
        mb_w,
        mb_h,
        rows,
        mb_qp,
        chroma_qp_offset,
        offset_a,
        offset_b,
        info,
    )
}
fn filter_frame_rows_impl<const PRE: bool>(
    y: &mut [u8],
    u: &mut [u8],
    v: &mut [u8],
    mb_w: usize,
    mb_h: usize,
    rows: core::ops::Range<usize>,
    mb_qp: &[u8],
    chroma_qp_offset: i32,
    offset_a: i32,
    offset_b: i32,
    info: &BlockInfo,
) {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Deblock);
    let cw = mb_w * 16;
    let ccw = mb_w * 8;
    let qpc = |qpy_val: i32| {
        crate::h264::common::predict::chroma_qp((qpy_val + chroma_qp_offset).clamp(0, 51) as u8)
            as i32
    };
    let fs = false;
    let two_pass = bs_twopass();
    let kind_off = kind_gate_off();
    let verify_kinds = verify_kind();
    let scratch = PACK_SCRATCH.take();
    let packs: Option<Vec<MbPack>> =
        if !PRE && bs_packed_on() && info.bs.is_empty() && deblock_tile() {
            let mut buf = scratch;
            {
                let _pg =
                    crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DebPack);
                pack_frame_into(info, mb_w, mb_h, &mut buf);
            }
            Some(buf)
        } else {
            PACK_SCRATCH.set(scratch);
            None
        };
    for mb_y in rows {
        let row0 = mb_y * mb_w;
        let qp_row = &mb_qp[row0..][..mb_w];
        let qp_up_row = if mb_y > 0 {
            Some(&mb_qp[row0 - mb_w..][..mb_w])
        } else {
            None
        };
        let bs_row = if !PRE && info.bs.is_empty() {
            None
        } else {
            Some(&info.bs[row0..][..mb_w])
        };
        let t8_row = if info.t8x8.is_empty() {
            None
        } else {
            Some(&info.t8x8[row0..][..mb_w])
        };
        for mb_x in 0..mb_w {
            let _dg = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DebDerive);
            let precomputed = PRE || !info.bs.is_empty();
            let pre_bs = bs_row.map(|r| &r[mb_x]);
            if let Some(m) = pre_bs {
                if m.v == [[0u8; 4]; 4] && m.h == [[0u8; 4]; 4] {
                    continue;
                }
            }
            let (vnz, hnz) = match pre_bs {
                Some(m) => (
                    core::array::from_fn(|e| u32::from_ne_bytes(m.v[e]) != 0),
                    core::array::from_fn(|e| u32::from_ne_bytes(m.h[e]) != 0),
                ),
                None => ([true; 4], [true; 4]),
            };

            let use_tile = !precomputed && deblock_tile();
            let have_bs = precomputed || use_tile;
            let mb_t8 = t8_row.is_some_and(|r| r[mb_x]);
            let mut bs_v = [[0i32; 4]; 4];
            let mut bs_h = [[0i32; 4]; 4];
            let flat_inter = if precomputed {
                false
            } else {
                derive_mb_general(
                    info,
                    mb_x,
                    mb_y,
                    mb_w,
                    mb_t8,
                    use_tile,
                    kind_off,
                    two_pass,
                    verify_kinds,
                    packs.as_ref(),
                    &mut bs_v,
                    &mut bs_h,
                )
            };
            drop(_dg);
            let qp_cur = qp_row[mb_x] as i32;
            let mut qp_left: Option<i32> = None;
            let mut qp_up: Option<i32> = None;
            let mut int_y: Option<(i32, i32, [i32; 3])> = None;
            for be in 0..4usize {
                if be == 0 && mb_x == 0 {
                    continue;
                }
                if flat_inter && be != 0 {
                    continue;
                }
                if mb_t8 && (be == 1 || be == 3) {
                    continue;
                }
                let mb_edge = be == 0;

                let bs4: [i32; 4] = if let Some(m) = pre_bs {
                    if !vnz[be] {
                        continue;
                    }
                    m.v[be].map(|b| b as i32)
                } else if have_bs {
                    let b = bs_v[be];
                    if b == [0i32; 4] {
                        continue;
                    }
                    b
                } else {
                    let abx = mb_x * 4 + be;
                    let mut b = [0i32; 4];
                    for (seg, t) in b.iter_mut().enumerate() {
                        let aby = mb_y * 4 + seg;
                        *t = info.bs(info.at(abx - 1, aby), info.at(abx, aby), mb_edge);
                    }
                    if b == [0i32; 4] {
                        continue;
                    }
                    b
                };

                let (alpha_y, beta_y, tc0a) = if mb_edge {
                    let ql = *qp_left.get_or_insert_with(|| {
                        if mb_x > 0 {
                            qp_row[mb_x - 1] as i32
                        } else {
                            qp_cur
                        }
                    });
                    let qpav = (ql + qp_cur + 1) >> 1;
                    thresholds(qpav, offset_a, offset_b)
                } else {
                    *int_y.get_or_insert_with(|| thresholds(qp_cur, offset_a, offset_b))
                };
                let tc0_luma = |bs: i32| {
                    if (1..4).contains(&bs) {
                        tc0a[bs as usize - 1]
                    } else {
                        0
                    }
                };
                let x = mb_x * 16 + be * 4;
                for (seg, &bs) in bs4.iter().enumerate() {
                    if bs == 0 {
                        continue;
                    }
                    let tc0 = tc0_luma(bs);
                    for row in 0..4 {
                        let yy = mb_y * 16 + seg * 4 + row;
                        filter_luma_line_contig(y, yy * cw + x, bs, alpha_y, beta_y, tc0);
                    }
                }
            }
            for be in 0..4usize {
                if be == 0 && mb_y == 0 {
                    continue;
                }
                if flat_inter && be != 0 {
                    continue;
                }
                if mb_t8 && (be == 1 || be == 3) {
                    continue;
                }
                let mb_edge = be == 0;

                let bs4: [i32; 4] = if let Some(m) = pre_bs {
                    if !hnz[be] {
                        continue;
                    }
                    m.h[be].map(|b| b as i32)
                } else if have_bs {
                    let b = bs_h[be];
                    if b == [0i32; 4] {
                        continue;
                    }
                    b
                } else {
                    let aby = mb_y * 4 + be;
                    let mut b = [0i32; 4];
                    for (seg, t) in b.iter_mut().enumerate() {
                        let abx = mb_x * 4 + seg;
                        *t = info.bs(info.at(abx, aby - 1), info.at(abx, aby), mb_edge);
                    }
                    if b == [0i32; 4] {
                        continue;
                    }
                    b
                };

                let (alpha_y, beta_y, tc0a) = if mb_edge {
                    let qu =
                        *qp_up.get_or_insert_with(|| qp_up_row.map_or(qp_cur, |r| r[mb_x] as i32));
                    let qpav = (qu + qp_cur + 1) >> 1;
                    thresholds(qpav, offset_a, offset_b)
                } else {
                    *int_y.get_or_insert_with(|| thresholds(qp_cur, offset_a, offset_b))
                };
                let tc0_luma = |bs: i32| {
                    if (1..4).contains(&bs) {
                        tc0a[bs as usize - 1]
                    } else {
                        0
                    }
                };
                let yy = mb_y * 16 + be * 4;
                for (seg, &bs) in bs4.iter().enumerate() {
                    if bs == 0 {
                        continue;
                    }
                    let tc0 = tc0_luma(bs);
                    for col in 0..4 {
                        let x = mb_x * 16 + seg * 4 + col;
                        let line = Line {
                            base: yy * cw + x,
                            step: cw as isize,
                        };
                        filter_luma_line(y, &line, bs, alpha_y, beta_y, tc0);
                    }
                }
            }
            {
                let cur_qpc = qpc(qp_cur);
                let (alpha_cv, beta_cv, tc0cv) = if mb_x > 0 {
                    let ql = qp_row[mb_x - 1] as i32;
                    thresholds((qpc(ql) + cur_qpc + 1) >> 1, offset_a, offset_b)
                } else {
                    (0, 0, [0; 3])
                };
                let (alpha_ch, beta_ch, tc0ch) = if mb_y > 0 {
                    let qu = qp_up_row.map_or(qp_cur, |r| r[mb_x] as i32);
                    thresholds((qpc(qu) + cur_qpc + 1) >> 1, offset_a, offset_b)
                } else {
                    (0, 0, [0; 3])
                };
                let (alpha_ci, beta_ci, tc0ci) = thresholds(cur_qpc, offset_a, offset_b);
                let tc0_of = |arr: [i32; 3], bs: i32| {
                    if (1..4).contains(&bs) {
                        arr[bs as usize - 1]
                    } else {
                        0
                    }
                };
                let chroma_bs = |stored: &[[i32; 4]; 4],
                                 edge: usize,
                                 vertical: bool,
                                 mb_edge: bool|
                 -> [i32; 4] {
                    if let Some(m) = pre_bs {
                        return (if vertical {
                            m.v[edge / 2]
                        } else {
                            m.h[edge / 2]
                        })
                        .map(|b| b as i32);
                    }
                    if have_bs {
                        return stored[edge / 2];
                    }
                    let mut bs4 = [0i32; 4];
                    for (seg, b) in bs4.iter_mut().enumerate() {
                        let (abx, aby) = if vertical {
                            (mb_x * 4 + edge / 2, mb_y * 4 + seg)
                        } else {
                            (mb_x * 4 + seg, mb_y * 4 + edge / 2)
                        };
                        let (p, q) = if vertical {
                            (info.at(abx - 1, aby), info.at(abx, aby))
                        } else {
                            (info.at(abx, aby - 1), info.at(abx, aby))
                        };
                        *b = info.bs(p, q, mb_edge);
                    }
                    bs4
                };
                for plane in [&mut *u, &mut *v] {
                    for cxe in [0usize, 4] {
                        if cxe == 0 && mb_x == 0 {
                            continue;
                        }
                        if flat_inter && cxe != 0 {
                            continue;
                        }
                        let mb_edge = cxe == 0;
                        let (alpha_c, beta_c, tc0c) = if mb_edge {
                            (alpha_cv, beta_cv, tc0cv)
                        } else {
                            (alpha_ci, beta_ci, tc0ci)
                        };
                        let bs4 = chroma_bs(&bs_v, cxe, true, mb_edge);
                        let x = mb_x * 8 + cxe;
                        for row in 0..8 {
                            let bs = bs4[(row * 2) / 4];
                            if bs == 0 {
                                continue;
                            }
                            let yy = mb_y * 8 + row;
                            filter_chroma_line_contig(
                                plane,
                                yy * ccw + x,
                                bs,
                                alpha_c,
                                beta_c,
                                tc0_of(tc0c, bs),
                            );
                        }
                    }
                    for cye in [0usize, 4] {
                        if cye == 0 && mb_y == 0 {
                            continue;
                        }
                        if flat_inter && cye != 0 {
                            continue;
                        }
                        let mb_edge = cye == 0;
                        let (alpha_c, beta_c, tc0c) = if mb_edge {
                            (alpha_ch, beta_ch, tc0ch)
                        } else {
                            (alpha_ci, beta_ci, tc0ci)
                        };
                        let bs4 = chroma_bs(&bs_h, cye, false, mb_edge);
                        let yy = mb_y * 8 + cye;
                        for col in 0..8 {
                            let bs = bs4[(col * 2) / 4];
                            if bs == 0 {
                                continue;
                            }
                            let line = Line {
                                base: yy * ccw + (mb_x * 8 + col),
                                step: ccw as isize,
                            };
                            filter_chroma_line(plane, &line, bs, alpha_c, beta_c, tc0_of(tc0c, bs));
                        }
                    }
                }
            }
        }
    }
    if let Some(buf) = packs {
        PACK_SCRATCH.set(buf);
    }
}
