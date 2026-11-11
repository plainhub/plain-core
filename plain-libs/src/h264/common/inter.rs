use std::vec;
use std::vec::Vec;
#[inline]
pub fn median3(a: i32, b: i32, c: i32) -> i32 {
    a + b + c - a.min(b).min(c) - a.max(b).max(c)
}
#[derive(Clone, Copy)]
pub struct MvNeighbor {
    pub available: bool,
    pub mv: (i32, i32),
    pub ref_idx: i32,
}
impl MvNeighbor {
    pub const NONE: MvNeighbor = MvNeighbor {
        available: false,
        mv: (0, 0),
        ref_idx: -1,
    };
}
pub fn predict_mv(a: MvNeighbor, b: MvNeighbor, c: MvNeighbor, cur_ref: i32) -> (i32, i32) {
    let resolve = |n: MvNeighbor| -> ((i32, i32), i32) {
        if n.available && n.ref_idx >= 0 {
            (n.mv, n.ref_idx)
        } else {
            ((0, 0), -1)
        }
    };
    let (mva, ra) = resolve(a);
    let (mut mvb, mut rb) = resolve(b);
    let (mut mvc, mut rc) = resolve(c);
    if !b.available && !c.available && a.available {
        mvb = mva;
        rb = ra;
        mvc = mva;
        rc = ra;
    }
    let matches = (ra == cur_ref) as i32 + (rb == cur_ref) as i32 + (rc == cur_ref) as i32;
    if matches == 1 {
        if ra == cur_ref {
            mva
        } else if rb == cur_ref {
            mvb
        } else {
            mvc
        }
    } else {
        (median3(mva.0, mvb.0, mvc.0), median3(mva.1, mvb.1, mvc.1))
    }
}
pub fn predict_partition_mv(
    mode: u8,
    part: usize,
    a: MvNeighbor,
    b: MvNeighbor,
    c: MvNeighbor,
    cur_ref: i32,
) -> (i32, i32) {
    if mode == 0 {
        return predict_mv(a, b, c, cur_ref);
    }
    let m = |n: MvNeighbor| n.available && n.ref_idx == cur_ref;
    match (mode, part) {
        (1, 0) if m(b) => b.mv,
        (1, 1) if m(a) => a.mv,
        (2, 0) if m(a) => a.mv,
        (2, 1) if m(c) => c.mv,
        _ => predict_mv(a, b, c, cur_ref),
    }
}
pub fn inter_partitions(mode: u8) -> &'static [(usize, usize, usize, usize)] {
    match mode {
        1 => &[(0, 0, 16, 8), (0, 8, 16, 8)],
        2 => &[(0, 0, 8, 16), (8, 0, 8, 16)],
        3 => &[(0, 0, 8, 8), (8, 0, 8, 8), (0, 8, 8, 8), (8, 8, 8, 8)],
        _ => &[(0, 0, 16, 16)],
    }
}
#[inline]
fn at(reference: &[u8], cw: usize, ch: usize, x: isize, y: isize) -> i32 {
    let xx = x.clamp(0, cw as isize - 1) as usize;
    let yy = y.clamp(0, ch as isize - 1) as usize;
    reference.get(yy * cw + xx).copied().unwrap_or(0) as i32
}
#[inline]
fn clip_u8(v: i32) -> i32 {
    v.clamp(0, 255)
}
const LUMA_TILE: usize = 21;
pub struct McScratch {
    tile: [u8; LUMA_TILE * LUMA_TILE],
    a: [u8; 256],
    b: [u8; 256],
}
#[inline]
pub fn with_mc_scratch<R>(f: impl FnOnce(&mut McScratch) -> R) -> R {
    MC_SCRATCH.with(|s| f(&mut s.borrow_mut()))
}
thread_local! { static MC_SCRATCH : core :: cell :: RefCell < McScratch > = const { core :: cell :: RefCell :: new (McScratch { tile : [0 ; LUMA_TILE * LUMA_TILE] , a : [0 ; 256] , b : [0 ; 256] , }) } ; }
fn luma_tile_into(
    t: &mut [u8],
    reference: &[u8],
    cw: usize,
    ch: usize,
    ix0: isize,
    iy0: isize,
    bw: usize,
    bh: usize,
) -> usize {
    let ts = bw + 5;
    if ix0 - 2 >= 0
        && iy0 - 2 >= 0
        && ix0 - 2 + ts as isize <= cw as isize
        && iy0 - 2 + (bh + 5) as isize <= ch as isize
        && reference.len() >= cw * ch
    {
        let (rx0, ry0) = ((ix0 - 2) as usize, (iy0 - 2) as usize);
        for ty in 0..bh + 5 {
            let src = (ry0 + ty) * cw + rx0;
            t[ty * ts..ty * ts + ts].copy_from_slice(&reference[src..src + ts]);
        }
        return ts;
    }
    for ty in 0..bh + 5 {
        let ry = (iy0 - 2 + ty as isize).clamp(0, ch as isize - 1) as usize * cw;
        let trow = &mut t[ty * ts..][..ts];
        for (tx, o) in trow.iter_mut().enumerate() {
            let rx = (ix0 - 2 + tx as isize).clamp(0, cw as isize - 1) as usize;
            *o = reference.get(ry + rx).copied().unwrap_or(0);
        }
    }
    ts
}
fn luma_h(t: &[u8], ts: usize, bw: usize, bh: usize, dr: usize, dc: usize, dst: &mut [u8]) {
    for r in 0..bh {
        let base = (2 + r + dr) * ts + 2 + dc;
        let src = &t[base - 2..][..bw + 5];
        let drow = &mut dst[r * bw..][..bw];
        for (o, w) in drow.iter_mut().zip(src.windows(6)) {
            let f = w[0] as i32 - 5 * w[1] as i32 + 20 * w[2] as i32 + 20 * w[3] as i32
                - 5 * w[4] as i32
                + w[5] as i32;
            *o = clip_u8((f + 16) >> 5) as u8;
        }
    }
}
fn luma_v(t: &[u8], ts: usize, bw: usize, bh: usize, dr: usize, dc: usize, dst: &mut [u8]) {
    for r in 0..bh {
        let base = (2 + r + dr) * ts + 2 + dc;
        let drow = &mut dst[r * bw..][..bw];
        for c in 0..bw {
            let p = base + c;
            let g = |q: usize| t.get(q).copied().unwrap_or(0) as i32;
            let f = g(p - 2 * ts) - 5 * g(p - ts) + 20 * g(p) + 20 * g(p + ts) - 5 * g(p + 2 * ts)
                + g(p + 3 * ts);
            drow[c] = clip_u8((f + 16) >> 5) as u8;
        }
    }
}
fn luma_centre(t: &[u8], ts: usize, bw: usize, bh: usize, dst: &mut [u8]) {
    let mut itmp = [0i32; LUMA_TILE];
    for r in 0..bh {
        let base = (2 + r) * ts;
        for (j, slot) in itmp[..bw + 5].iter_mut().enumerate() {
            let p = base + j;
            let g = |q: usize| t.get(q).copied().unwrap_or(0) as i32;
            *slot = g(p - 2 * ts) - 5 * g(p - ts) + 20 * g(p) + 20 * g(p + ts) - 5 * g(p + 2 * ts)
                + g(p + 3 * ts);
        }
        let drow = &mut dst[r * bw..][..bw];
        for (o, w) in drow.iter_mut().zip(itmp[..bw + 5].windows(6)) {
            let f = w[0] - 5 * w[1] + 20 * w[2] + 20 * w[3] - 5 * w[4] + w[5];
            *o = clip_u8((f + 512) >> 10) as u8;
        }
    }
}
pub fn avg_row_into(a: &[u8], b: &[u8], w: usize, out: &mut [u8]) {
    for (d, (&x, &y)) in out[..w].iter_mut().zip(a[..w].iter().zip(&b[..w])) {
        *d = ((x as u16 + y as u16 + 1) >> 1) as u8;
    }
}
fn pixel_avg(a: &[u8], b: &[u8], bw: usize, bh: usize, dst: &mut [u8]) {
    let n = bw * bh;
    for (d, (&x, &y)) in dst[..n].iter_mut().zip(a[..n].iter().zip(&b[..n])) {
        *d = ((x as i32 + y as i32 + 1) >> 1) as u8;
    }
}
fn avg_full(
    t: &[u8],
    ts: usize,
    bw: usize,
    bh: usize,
    dr: usize,
    dc: usize,
    half: &[u8],
    dst: &mut [u8],
) {
    for (r, drow) in dst[..bw * bh].chunks_mut(bw).enumerate() {
        let base = (2 + r + dr) * ts + 2 + dc;
        let trow = &t[base..][..bw];
        let hrow = &half[r * bw..][..bw];
        for ((d, &x), &y) in drow.iter_mut().zip(trow).zip(hrow) {
            *d = ((x as i32 + y as i32 + 1) >> 1) as u8;
        }
    }
}
#[inline]
fn qpel_compose() -> bool {
    {
        return false;
    }
}
fn qpel_one_filter(
    t: &[u8],
    ts: usize,
    bw: usize,
    bh: usize,
    horiz: bool,
    fshift: usize,
    out: &mut [u8],
) {
    let mut half = [0u8; 256];
    let n = bw * bh;
    debug_assert!(n <= 256);
    if horiz {
        luma_h(t, ts, bw, bh, 0, 0, &mut half[..n]);
        avg_full(t, ts, bw, bh, 0, fshift, &half[..n], out);
    } else {
        luma_v(t, ts, bw, bh, 0, 0, &mut half[..n]);
        avg_full(t, ts, bw, bh, fshift, 0, &half[..n], out);
    }
}
fn qpel_hv(
    t: &[u8],
    ts: usize,
    bw: usize,
    bh: usize,
    hdr: usize,
    hdc: usize,
    vdr: usize,
    vdc: usize,
    a: &mut [u8],
    out: &mut [u8],
) {
    luma_h(t, ts, bw, bh, hdr, hdc, a);
    let n = bw * bh;
    let mut b = [0u8; 256];
    debug_assert!(n <= 256);
    luma_v(t, ts, bw, bh, vdr, vdc, &mut b[..n]);
    pixel_avg(a, &b[..n], bw, bh, out);
}
fn qpel_centre_adj(
    t: &[u8],
    ts: usize,
    bw: usize,
    bh: usize,
    horiz: bool,
    fshift: usize,
    a: &mut [u8],
    b: &mut [u8],
    out: &mut [u8],
) {
    if horiz {
        luma_h(t, ts, bw, bh, fshift, 0, a);
    } else {
        luma_v(t, ts, bw, bh, 0, fshift, a);
    }
    luma_centre(t, ts, bw, bh, b);
    pixel_avg(a, b, bw, bh, out);
}
fn mc_luma_subpel(
    t: &[u8],
    ts: usize,
    bw: usize,
    bh: usize,
    fx: i32,
    fy: i32,
    a: &mut [u8],
    b: &mut [u8],
    out: &mut [u8],
) {
    match (fx, fy) {
        (2, 0) => luma_h(t, ts, bw, bh, 0, 0, out),
        (0, 2) => luma_v(t, ts, bw, bh, 0, 0, out),
        (2, 2) => luma_centre(t, ts, bw, bh, out),
        (1, 0) => qpel_one_filter(t, ts, bw, bh, true, 0, out),
        (3, 0) => qpel_one_filter(t, ts, bw, bh, true, 1, out),
        (0, 1) => qpel_one_filter(t, ts, bw, bh, false, 0, out),
        (0, 3) => qpel_one_filter(t, ts, bw, bh, false, 1, out),
        (1, 1) => qpel_hv(t, ts, bw, bh, 0, 0, 0, 0, a, out),
        (3, 1) => qpel_hv(t, ts, bw, bh, 0, 0, 0, 1, a, out),
        (1, 3) => qpel_hv(t, ts, bw, bh, 1, 0, 0, 0, a, out),
        (3, 3) => qpel_hv(t, ts, bw, bh, 1, 0, 0, 1, a, out),
        (2, 1) => qpel_centre_adj(t, ts, bw, bh, true, 0, a, b, out),
        (2, 3) => qpel_centre_adj(t, ts, bw, bh, true, 1, a, b, out),
        (1, 2) => qpel_centre_adj(t, ts, bw, bh, false, 0, a, b, out),
        (3, 2) => qpel_centre_adj(t, ts, bw, bh, false, 1, a, b, out),
        _ => unreachable!("(0,0) is the full-pel path"),
    }
}
#[derive(Clone, Debug)]
pub struct HpelPlanes {
    pub f: Vec<u8>,
    pub h: Vec<u8>,
    pub v: Vec<u8>,
    pub c: Vec<u8>,
    pub stride: usize,
    pub pad: usize,
    pub pw: usize,
    pub ph: usize,
    pub cw: usize,
    pub ch: usize,
}
pub const HPEL_PAD_DEFAULT: usize = 32;
pub fn hpel_pad() -> usize {
    crate::h264::common::cached_knob!(usize, HPEL_PAD_DEFAULT, {
        crate::h264::common::knob("RFF_HPEL_PAD")
            .and_then(|s| s.parse().ok())
            .unwrap_or(HPEL_PAD_DEFAULT)
            .clamp(0, 128)
    })
}
pub fn pad_plane(src: &[u8], w: usize, h: usize, pad: usize) -> Vec<u8> {
    pad_plane_into(Vec::new(), src, w, h, pad)
}
pub fn pad_plane_into(mut f: Vec<u8>, src: &[u8], w: usize, h: usize, pad: usize) -> Vec<u8> {
    let (pw, ph) = (w + 2 * pad, h + 2 * pad);
    if f.len() != pw * ph {
        f.clear();
        f.resize(pw * ph, 0);
    }
    for y in 0..ph {
        let sy = (y as isize - pad as isize).clamp(0, h as isize - 1) as usize;
        let row = &src[sy * w..sy * w + w];
        let d = &mut f[y * pw..y * pw + pw];
        let (Some(&l), Some(&r)) = (row.first(), row.last()) else {
            continue;
        };
        d[..pad].fill(l);
        d[pad..pad + w].copy_from_slice(row);
        d[pad + w..].fill(r);
    }
    f
}

fn hpel_fused_enabled() -> bool {
    crate::h264::common::cached_knob!(
        bool,
        false,
        crate::h264::common::knob("RFF_HPEL_FUSED")
            .map(|s| s == "1")
            .unwrap_or(false)
    )
}
fn build_hpel_tiles(f: &[u8], pw: usize, ph: usize, h: &mut [u8], v: &mut [u8], c: &mut [u8]) {
    let mut tile = [0u8; LUMA_TILE * LUMA_TILE];
    let mut blk = [0u8; 256];
    let mut by = 0;
    while by < ph {
        let mut bx = 0;
        while bx < pw {
            let bh = 16.min(ph - by);
            let bwid = 16.min(pw - bx);
            let ts = luma_tile_into(&mut tile, f, pw, ph, bx as isize, by as isize, bwid, bh);
            for kind in 0u8..3 {
                let plane: &mut [u8] = match kind {
                    0 => &mut *h,
                    1 => &mut *v,
                    _ => &mut *c,
                };
                match kind {
                    0 => luma_h(&tile, ts, bwid, bh, 0, 0, &mut blk),
                    1 => luma_v(&tile, ts, bwid, bh, 0, 0, &mut blk),
                    _ => luma_centre(&tile, ts, bwid, bh, &mut blk),
                }
                for r in 0..bh {
                    plane[(by + r) * pw + bx..][..bwid].copy_from_slice(&blk[r * bwid..][..bwid]);
                }
            }
            bx += 16;
        }
        by += 16;
    }
}
fn build_hpel_fused(f: &[u8], pw: usize, ph: usize, h: &mut [u8], v: &mut [u8], c: &mut [u8]) {
    let cl = |i: isize, hi: usize| i.clamp(0, hi as isize - 1) as usize;
    let mut vt_buf = vec![0i32; pw + 5];
    let mut hb_buf = vec![0u8; pw + 5];
    let vt = &mut vt_buf[..pw + 5];
    let hb = &mut hb_buf[..pw + 5];
    for y in 0..ph {
        let (ym2, ym1, y0, yp1, yp2, yp3) = (
            cl(y as isize - 2, ph) * pw,
            cl(y as isize - 1, ph) * pw,
            y * pw,
            cl(y as isize + 1, ph) * pw,
            cl(y as isize + 2, ph) * pw,
            cl(y as isize + 3, ph) * pw,
        );
        let rows: [&[u8]; 6] = [
            &f[ym2..][..pw],
            &f[ym1..][..pw],
            &f[y0..][..pw],
            &f[yp1..][..pw],
            &f[yp2..][..pw],
            &f[yp3..][..pw],
        ];
        let tap = |x: usize| {
            let g = |k: usize| rows[k & 7].get(x).copied().unwrap_or(0) as i32;
            g(0) - 5 * g(1) + 20 * g(2) + 20 * g(3) - 5 * g(4) + g(5)
        };
        let mid = rows[2];
        let (lo, edge) = (
            mid.first().copied().unwrap_or(0),
            mid.last().copied().unwrap_or(0),
        );
        let tap_edge = tap(pw.saturating_sub(1));
        for (v, h) in vt[..2].iter_mut().zip(hb[..2].iter_mut()) {
            (*v, *h) = (tap(0), lo);
        }
        for (x, (v, h)) in vt[2..pw + 2]
            .iter_mut()
            .zip(hb[2..pw + 2].iter_mut())
            .enumerate()
        {
            (*v, *h) = (tap(x), mid.get(x).copied().unwrap_or(0));
        }
        for (v, h) in vt[pw + 2..].iter_mut().zip(hb[pw + 2..].iter_mut()) {
            (*v, *h) = (tap_edge, edge);
        }
        let hrow = &mut h[y0..y0 + pw];
        let vrow = &mut v[y0..y0 + pw];
        let crow = &mut c[y0..y0 + pw];
        for (o, &t) in vrow.iter_mut().zip(&vt[2..]) {
            *o = clip_u8((t + 16) >> 5) as u8;
        }
        for (o, w) in crow.iter_mut().zip(vt.windows(6)) {
            let s = w[0] - 5 * w[1] + 20 * w[2] + 20 * w[3] - 5 * w[4] + w[5];
            *o = clip_u8((s + 512) >> 10) as u8;
        }
        for (o, w) in hrow.iter_mut().zip(hb.windows(6)) {
            let s = w[0] as i32 - 5 * w[1] as i32 + 20 * w[2] as i32 + 20 * w[3] as i32
                - 5 * w[4] as i32
                + w[5] as i32;
            *o = clip_u8((s + 16) >> 5) as u8;
        }
    }
}

#[inline]
pub fn hpel_qpel_refs<'a>(
    p: &'a HpelPlanes,
    x0: usize,
    y0: usize,
    bw: usize,
    bh: usize,
    mvx: i32,
    mvy: i32,
) -> Option<(&'a [u8], usize, &'a [u8], usize, usize)> {
    let (fx, fy) = (mvx & 3, mvy & 3);
    if fx & 1 == 0 && fy & 1 == 0 {
        return None;
    }
    let (ix0, iy0) = (
        x0 as isize + (mvx >> 2) as isize,
        y0 as isize + (mvy >> 2) as isize,
    );
    let (px, py) = (ix0 + p.pad as isize, iy0 + p.pad as isize);
    if px < 0
        || py < 0
        || px + bw as isize + 1 > p.pw as isize
        || py + bh as isize + 1 > p.ph as isize
    {
        return None;
    }
    let stride = p.stride;
    let base = py as usize * stride + px as usize;
    let g: &[u8] = &p.f;
    let (pa, oa, pb, ob): (&[u8], usize, &[u8], usize) = match (fx, fy) {
        (1, 0) => (g, 0, &p.h, 0),
        (3, 0) => (g, 1, &p.h, 0),
        (0, 1) => (g, 0, &p.v, 0),
        (0, 3) => (g, stride, &p.v, 0),
        (1, 1) => (&p.h, 0, &p.v, 0),
        (3, 1) => (&p.h, 0, &p.v, 1),
        (1, 3) => (&p.h, stride, &p.v, 0),
        (3, 3) => (&p.h, stride, &p.v, 1),
        (2, 1) => (&p.h, 0, &p.c, 0),
        (2, 3) => (&p.h, stride, &p.c, 0),
        (1, 2) => (&p.v, 0, &p.c, 0),
        _ => (&p.v, 1, &p.c, 0),
    };
    if base + oa + (bh - 1) * stride + bw > pa.len()
        || base + ob + (bh - 1) * stride + bw > pb.len()
    {
        return None;
    }
    Some((pa, base + oa, pb, base + ob, stride))
}

#[inline]
fn avg_rows<const BW: usize>(
    pa: &[u8],
    oa: usize,
    pb: &[u8],
    ob: usize,
    cw: usize,
    bh: usize,
    out: &mut [u8],
) {
    for r in 0..bh {
        let sa = &pa[oa + r * cw..][..BW];
        let sb = &pb[ob + r * cw..][..BW];
        let o = &mut out[r * BW..r * BW + BW];
        for i in 0..BW {
            o[i] = ((sa[i] as u16 + sb[i] as u16 + 1) >> 1) as u8;
        }
    }
}

pub const PAD_L: usize = 32;
pub const PAD_C: usize = 16;

#[inline]
pub(crate) fn abl_mc() -> bool {
    {
        return false;
    }
}
pub fn mc_luma_padded(
    padded: &[u8],
    stride: usize,
    pad: usize,
    pw: usize,
    ph: usize,
    x0: usize,
    y0: usize,
    bw: usize,
    bh: usize,
    mvx: i32,
    mvy: i32,
    out: &mut [u8],
) {
    with_mc_scratch(|scr| {
        mc_luma_padded_pre(
            scr, padded, stride, pad, pw, ph, x0, y0, bw, bh, mvx, mvy, out,
        )
    })
}
pub fn mc_luma_padded_pre(
    scr: &mut McScratch,
    padded: &[u8],
    stride: usize,
    pad: usize,
    pw: usize,
    ph: usize,
    x0: usize,
    y0: usize,
    bw: usize,
    bh: usize,
    mvx: i32,
    mvy: i32,
    out: &mut [u8],
) {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::InterMc);
    let (ix0, iy0) = (
        x0 as isize + (mvx >> 2) as isize,
        y0 as isize + (mvy >> 2) as isize,
    );
    let (fx, fy) = (mvx & 3, mvy & 3);
    let p = pad as isize;
    let (lo_x, lo_y) = (ix0 - 2, iy0 - 2);
    let intact = stride >= pw + 2 * pad && padded.len() >= stride * (ph + 2 * pad);
    let in_range = intact
        && lo_x >= -p
        && lo_y >= -p
        && lo_x + (bw + 5) as isize <= pw as isize + p
        && lo_y + (bh + 5) as isize <= ph as isize + p;
    if in_range {
        if fx == 0 && fy == 0 {
            let sb = ((iy0 + p) as usize) * stride + (ix0 + p) as usize;
            macro_rules! rows {
                ($ n : expr) => {{
                    let sw = &padded[sb..sb + (bh - 1) * stride + $n];
                    let ow = &mut out[..(bh - 1) * $n + $n];
                    for dy in 0..bh {
                        ow[dy * $n..dy * $n + $n]
                            .copy_from_slice(&sw[dy * stride..dy * stride + $n]);
                    }
                }};
            }
            match bw {
                16 => rows!(16),
                8 => rows!(8),
                4 => rows!(4),
                _ => {
                    let sw = &padded[sb..sb + (bh - 1) * stride + bw];
                    let ow = &mut out[..(bh - 1) * bw + bw];
                    for dy in 0..bh {
                        ow[dy * bw..dy * bw + bw]
                            .copy_from_slice(&sw[dy * stride..dy * stride + bw]);
                    }
                }
            }
        } else {
            let halo = ((lo_y + p) as usize) * stride + (lo_x + p) as usize;
            mc_luma_subpel(
                &padded[halo..],
                stride,
                bw,
                bh,
                fx,
                fy,
                &mut scr.a,
                &mut scr.b,
                out,
            );
        }
        return;
    }
    let ts = bw + 5;
    let mut t = [0u8; LUMA_TILE * LUMA_TILE];
    for ty in 0..bh + 5 {
        let py = (lo_y + ty as isize).clamp(0, ph as isize - 1) as usize;
        let ry = (py + pad) * stride;
        let trow = &mut t[ty * ts..][..ts];
        for (tx, o) in trow.iter_mut().enumerate() {
            let px = (lo_x + tx as isize).clamp(0, pw as isize - 1) as usize;
            *o = padded.get(ry + px + pad).copied().unwrap_or(128);
        }
    }
    if fx == 0 && fy == 0 {
        for dy in 0..bh {
            let s = (dy + 2) * ts + 2;
            out[dy * bw..dy * bw + bw].copy_from_slice(&t[s..s + bw]);
        }
    } else {
        mc_luma_subpel(&t, ts, bw, bh, fx, fy, &mut scr.a, &mut scr.b, out);
    }
}
pub fn mc_chroma_padded_pair(
    pu: &[u8],
    pv: &[u8],
    stride: usize,
    pad: usize,
    pw: usize,
    ph: usize,
    x0: usize,
    y0: usize,
    bw: usize,
    bh: usize,
    mvx: i32,
    mvy: i32,
    outu: &mut [u8],
    outv: &mut [u8],
) {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::InterMc);
    let (ix0, iy0) = (
        x0 as isize + (mvx >> 3) as isize,
        y0 as isize + (mvy >> 3) as isize,
    );
    let (fx, fy) = (mvx & 7, mvy & 7);
    let p = pad as isize;
    let intact = stride >= pw + 2 * pad
        && pu.len() >= stride * (ph + 2 * pad)
        && pv.len() >= stride * (ph + 2 * pad);
    let in_range = intact
        && ix0 >= -p
        && iy0 >= -p
        && ix0 + (bw + 1) as isize <= pw as isize + p
        && iy0 + (bh + 1) as isize <= ph as isize + p;
    if !in_range {
        mc_chroma_padded(pu, stride, pad, pw, ph, x0, y0, bw, bh, mvx, mvy, outu);
        mc_chroma_padded(pv, stride, pad, pw, ph, x0, y0, bw, bh, mvx, mvy, outv);
        return;
    }
    if fx == 0 && fy == 0 {
        for dy in 0..bh {
            let src = ((iy0 + dy as isize + p) as usize) * stride + (ix0 + p) as usize;
            outu[dy * bw..dy * bw + bw].copy_from_slice(&pu[src..src + bw]);
            outv[dy * bw..dy * bw + bw].copy_from_slice(&pv[src..src + bw]);
        }
        return;
    }
    let (wa, wb, wc, wd) = ((8 - fx) * (8 - fy), fx * (8 - fy), (8 - fx) * fy, fx * fy);
    let halo = ((iy0 + p) as usize) * stride + (ix0 + p) as usize;
    for (pl, out) in [(pu, &mut *outu), (pv, &mut *outv)] {
        for r in 0..bh {
            let base = halo + r * stride;
            let (r0, r1) = (&pl[base..][..bw + 1], &pl[base + stride..][..bw + 1]);
            let orow = &mut out[r * bw..][..bw];
            for ((o, a), b) in orow.iter_mut().zip(r0.windows(2)).zip(r1.windows(2)) {
                let v = wa * a[0] as i32 + wb * a[1] as i32 + wc * b[0] as i32 + wd * b[1] as i32;
                *o = ((v + 32) >> 6) as u8;
            }
        }
    }
}
pub fn mc_chroma_padded(
    padded: &[u8],
    stride: usize,
    pad: usize,
    pw: usize,
    ph: usize,
    x0: usize,
    y0: usize,
    bw: usize,
    bh: usize,
    mvx: i32,
    mvy: i32,
    out: &mut [u8],
) {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::InterMc);
    let (ix0, iy0) = (
        x0 as isize + (mvx >> 3) as isize,
        y0 as isize + (mvy >> 3) as isize,
    );
    let (fx, fy) = (mvx & 7, mvy & 7);
    let p = pad as isize;
    let intact = stride >= pw + 2 * pad && padded.len() >= stride * (ph + 2 * pad);
    let in_range = intact
        && ix0 >= -p
        && iy0 >= -p
        && ix0 + (bw + 1) as isize <= pw as isize + p
        && iy0 + (bh + 1) as isize <= ph as isize + p;
    let (wa, wb, wc, wd) = ((8 - fx) * (8 - fy), fx * (8 - fy), (8 - fx) * fy, fx * fy);
    if in_range {
        if fx == 0 && fy == 0 {
            let sb = ((iy0 + p) as usize) * stride + (ix0 + p) as usize;
            let sw = &padded[sb..sb + (bh - 1) * stride + bw];
            let ow = &mut out[..(bh - 1) * bw + bw];
            for dy in 0..bh {
                ow[dy * bw..dy * bw + bw].copy_from_slice(&sw[dy * stride..dy * stride + bw]);
            }
            return;
        }
        let halo = ((iy0 + p) as usize) * stride + (ix0 + p) as usize;
        for r in 0..bh {
            let base = halo + r * stride;
            let (r0, r1) = (
                &padded[base..][..bw + 1],
                &padded[base + stride..][..bw + 1],
            );
            let orow = &mut out[r * bw..][..bw];
            for ((o, a), b) in orow.iter_mut().zip(r0.windows(2)).zip(r1.windows(2)) {
                let v = wa * a[0] as i32 + wb * a[1] as i32 + wc * b[0] as i32 + wd * b[1] as i32;
                *o = ((v + 32) >> 6) as u8;
            }
        }
        return;
    }
    let ts = bw + 1;
    let mut t = [0u8; 9 * 9];
    for ty in 0..bh + 1 {
        let py = (iy0 + ty as isize).clamp(0, ph as isize - 1) as usize;
        let ry = (py + pad) * stride;
        let trow = &mut t[ty * ts..][..ts];
        for (tx, o) in trow.iter_mut().enumerate() {
            let px = (ix0 + tx as isize).clamp(0, pw as isize - 1) as usize;
            *o = padded.get(ry + px + pad).copied().unwrap_or(128);
        }
    }
    for r in 0..bh {
        let (r0, r1) = (&t[r * ts..][..bw + 1], &t[(r + 1) * ts..][..bw + 1]);
        let orow = &mut out[r * bw..][..bw];
        for ((o, a), b) in orow.iter_mut().zip(r0.windows(2)).zip(r1.windows(2)) {
            let v = wa * a[0] as i32 + wb * a[1] as i32 + wc * b[0] as i32 + wd * b[1] as i32;
            *o = ((v + 32) >> 6) as u8;
        }
    }
}
