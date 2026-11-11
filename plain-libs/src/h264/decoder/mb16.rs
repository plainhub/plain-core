use crate::h264::common::bit_reader::OutOfData;
use crate::h264::common::cavlc::{decode_residual_block_into, read_cbp_inter, read_cbp_intra};
use crate::h264::common::inter::{
    MvNeighbor, inter_partitions, mc_chroma_padded, mc_luma_padded, predict_mv,
    predict_partition_mv,
};
use crate::h264::common::predict::{
    CHROMA_4X4_SCAN_XY, I16Mode, LUMA_4X4_SCAN_XY, add_residual_8x8, chroma_qp, chroma8x8_pred,
    intra4x4_pred, intra8x8_pred, luma16x16_pred, nnz_raster_from_z, reconstruct_4x4_dc_into,
    reconstruct_4x4_scan_into,
};
use crate::h264::common::transform::{
    DQ_FLAT, DQ8_FLAT, Dequant8Qp, DequantQp, inverse_quant_8x8_dq, inverse_quant_chroma_dc,
    inverse_quant_chroma_dc_weighted, inverse_quant_luma_dc_scan,
};
use crate::h264::common::{BitReader, YuvFrame};
use std::borrow::ToOwned;
use std::boxed::Box;
use std::format;
use std::string::{String, ToString};
use std::vec;
use std::vec::Vec;
pub struct MvField {
    pub mb_w: usize,
    pub mb_h: usize,
    pub mv: Vec<(i32, i32)>,
    pub ref_idx: Vec<i32>,
    pub inter: Vec<bool>,
}
pub static MV_DUMP: crate::h264::decoder::sync::Mutex<Vec<MvField>> =
    crate::h264::decoder::sync::Mutex::new(Vec::new());
pub(crate) fn route_dump_on() -> bool {
    {
        return false;
    }
}
pub(crate) fn dump_mb_on() -> bool {
    {
        return false;
    }
}
pub fn mv_dump_on() -> bool {
    {
        return false;
    }
}
fn publish_filtered_rows_to_slot(
    slot: &crate::h264::decoder::RefFrame,
    rec_y: &[u8],
    rec_u: &[u8],
    rec_v: &[u8],
    cw: usize,
    ccw: usize,
    ch: usize,
    mb_rows: usize,
) {
    if mb_rows == 0 || slot.frozen.get().is_some() {
        return;
    }
    if !crate::h264::decoder::row_publish_on() {
        return;
    }
    let mb_h = (ch + 15) / 16;
    if mb_rows < mb_h && (mb_rows & 1) != 0 {
        return;
    }
    let Some(live) = &slot.live else {
        slot.publish_ready_rows(mb_rows * 16);
        return;
    };
    let cch = ch / 2;
    let prev = slot.ready_rows.load(core::sync::atomic::Ordering::Acquire) / 16;
    if mb_rows <= prev {
        return;
    }
    let mut py = live.py.write().unwrap();
    let mut pu = live.pu.write().unwrap();
    let mut pv = live.pv.write().unwrap();
    let ls = cw + 2 * crate::h264::decoder::LPAD;
    let cs = ccw + 2 * crate::h264::decoder::CPAD;
    for mr in prev..mb_rows {
        for dy in 0..16 {
            let y = mr * 16 + dy;
            if y >= ch {
                break;
            }
            let src = &rec_y[y * cw..(y + 1) * cw];
            let dst_y = y + crate::h264::decoder::LPAD;
            let seg = &mut py[dst_y * ls..][..2 * crate::h264::decoder::LPAD + cw];
            let (lpad, rest) = seg.split_at_mut(crate::h264::decoder::LPAD);
            let (mid, rpad) = rest.split_at_mut(cw);
            mid.copy_from_slice(src);
            let (Some(&left), Some(&right)) = (mid.first(), mid.last()) else {
                continue;
            };
            lpad.fill(left);
            rpad[..crate::h264::decoder::LPAD].fill(right);
        }
        for dy in 0..8 {
            let cy = mr * 8 + dy;
            if cy >= cch {
                break;
            }
            for (rec, plane) in [(rec_u, &mut *pu), (rec_v, &mut *pv)] {
                let src = &rec[cy * ccw..(cy + 1) * ccw];
                let dst_y = cy + crate::h264::decoder::CPAD;
                let seg = &mut plane[dst_y * cs..][..2 * crate::h264::decoder::CPAD + ccw];
                let (lpad, rest) = seg.split_at_mut(crate::h264::decoder::CPAD);
                let (mid, rpad) = rest.split_at_mut(ccw);
                mid.copy_from_slice(src);
                let (Some(&left), Some(&right)) = (mid.first(), mid.last()) else {
                    continue;
                };
                lpad.fill(left);
                rpad[..crate::h264::decoder::CPAD].fill(right);
            }
        }
    }
    if prev == 0 {
        let (pad, rest) = py.split_at_mut(crate::h264::decoder::LPAD * ls);
        if let Some(first) = rest.get(..ls) {
            for row in pad.chunks_mut(ls) {
                row.copy_from_slice(first);
            }
        }
        for plane in [&mut *pu, &mut *pv] {
            let (pad, rest) = plane.split_at_mut(crate::h264::decoder::CPAD * cs);
            if let Some(first) = rest.get(..cs) {
                for row in pad.chunks_mut(cs) {
                    row.copy_from_slice(first);
                }
            }
        }
    }
    slot.publish_ready_rows(mb_rows * 16);
}
pub struct FrameDecoder {
    mb_w: usize,
    mb_h: usize,
    qp: u8,
    cur_qp: u8,
    chroma_qp_offset: i32,
    cw: usize,
    ch: usize,
    ccw: usize,
    cch: usize,
    rec_y: Vec<u8>,
    rec_u: Vec<u8>,
    rec_v: Vec<u8>,
    mb_qp: Vec<u8>,
    slice_first_mb: usize,
    nnz_y: Vec<u8>,
    nnz_c: [Vec<u8>; 2],
    modes_y: Vec<u8>,
    coded_y: Vec<bool>,
    mv_y: Vec<(i32, i32)>,
    inter_y: Vec<bool>,
    ref_idx_y: Vec<i32>,
    mv1: Vec<(i32, i32)>,
    ref_idx1: Vec<i32>,
    refs1: Vec<crate::h264::decoder::Ref>,
    num_ref_active1: usize,
    is_b: bool,
    b_possible: bool,
    direct_spatial: bool,
    nnz_l_cache: [u8; 25],
    nnz_c_cache: [[u8; 9]; 2],
    refs: Vec<crate::h264::decoder::Ref>,
    num_ref_active: usize,
    constrained_intra: bool,
    scaling: Option<[[i32; 16]; 6]>,
    scaling8: Option<[[i32; 64]; 2]>,
    transform_8x8_mode: bool,
    transform_bypass: bool,
    route_skip_mbs: u32,
    route_coded_mbs: u32,
    slice_bounds: Vec<(usize, bool)>,
    cur_idc2: bool,
    mb_t8x8: Vec<bool>,
    bs_frame: Vec<crate::h264::common::deblock::MbBs>,
    bs_rows: usize,
    flt_rows: usize,
    pk_prev: Vec<crate::h264::common::deblock::MbPack>,
    pk_cur: Vec<crate::h264::common::deblock::MbPack>,
    nnz_dbr: Vec<u8>,
    bak_y: Vec<u8>,
    bak_u: Vec<u8>,
    bak_v: Vec<u8>,
    edc_jobs: Vec<EdcJob>,
    edc_job_pool: Vec<Box<PInterJob>>,
    edc_nores_pool: Vec<Box<PInterNoResJob>>,
    scratch_luma: Option<Box<[[i32; 16]; 16]>>,
    scratch_luma8: Option<Box<[[i32; 64]; 4]>>,
    scratch_cac: Option<Box<[[[i32; 16]; 4]; 2]>>,
    edc_active: bool,
    edc_tx: Option<crate::h264::decoder::sync::mpsc::SyncSender<EdcMsg>>,
    edc_ctx_rx: Option<crate::h264::decoder::sync::mpsc::Receiver<PixelCtx>>,
    edc_back_tx: Option<crate::h264::decoder::sync::mpsc::Sender<PixelCtx>>,
    edc_parked: Option<PixelCtx>,
    edc_regions: Option<Vec<BRegion>>,
    bsk_last: Option<(usize, usize, i32, i32, (i32, i32), (i32, i32))>,
    edc_batch: Vec<EdcJob>,
    bits_per_mb: f64,
    db_ena: bool,
    db_oa: i32,
    db_ob: i32,
    mb_kind: Vec<u8>,
    weights: Option<WeightTable>,
    row_wait: bool,
    col_ok: bool,
    col_w4: usize,
    any_t8: bool,
    any_idc2: bool,
    skip_zero_next: usize,
    bzspan: Option<(usize, usize, usize, BzKind)>,
    pzspan: Option<(usize, usize, usize, bool)>,
    bzero: Vec<bool>,
    sc_cat: Vec<u8>,
    sc_cbp: Vec<u8>,
    sc_cmode: Vec<i32>,
    sc_nzc: Vec<[u8; 24]>,
    sc_cbfdc: Vec<u16>,
    sc_skip: Vec<bool>,
    sc_ref: Vec<[i8; 16]>,
    sc_mvd: Vec<[[i16; 2]; 16]>,
    sc_ref1: Vec<[i8; 16]>,
    sc_mvd1: Vec<[[i16; 2]; 16]>,
    sc_direct: Vec<bool>,
    iw00: Option<Option<(i32, i32)>>,
    weights_l0id: bool,
    weights_id0: bool,
    cur_poc: i32,
    weighted_bipred_idc: u8,
    direct_8x8_inference: bool,
    progress: Option<crate::h264::decoder::Ref>,
    ref_poc0: Vec<i32>,
    ref_poc1: Vec<i32>,
}
#[derive(Clone, Default)]
pub struct WeightTable {
    pub luma_log2_denom: i32,
    pub chroma_log2_denom: i32,
    pub luma: [Vec<(i32, i32)>; 2],
    pub chroma: [Vec<[(i32, i32); 2]>; 2],
}
impl WeightTable {
    fn ref0_identity(&self) -> bool {
        self.luma[0]
            .first()
            .is_some_and(|&(w, o)| w == 1 << self.luma_log2_denom && o == 0)
            && self.chroma[0].first().is_some_and(|c| {
                c.iter()
                    .all(|&(w, o)| w == 1 << self.chroma_log2_denom && o == 0)
            })
    }
    fn list_identity(&self, list: usize) -> bool {
        self.luma[list]
            .iter()
            .all(|&(w, o)| w == 1 << self.luma_log2_denom && o == 0)
            && self.chroma[list].iter().all(|c| {
                c.iter()
                    .all(|&(w, o)| w == 1 << self.chroma_log2_denom && o == 0)
            })
    }
    #[inline]
    fn luma_wo(&self, list: usize, refi: usize) -> (i32, i32) {
        self.luma[list & 1]
            .get(refi)
            .copied()
            .unwrap_or((1 << self.luma_log2_denom, 0))
    }
    #[inline]
    fn chroma_wo(&self, list: usize, refi: usize, cc: usize) -> (i32, i32) {
        self.chroma[list & 1]
            .get(refi)
            .map(|c| c[cc & 1])
            .unwrap_or((1 << self.chroma_log2_denom, 0))
    }
    fn apply_luma(&self, sample: u8, list: usize, refi: usize) -> u8 {
        let (w, o) = self.luma_wo(list, refi);
        let lwd = self.luma_log2_denom;
        let v = if lwd >= 1 {
            ((sample as i32 * w + (1 << (lwd - 1))) >> lwd) + o
        } else {
            sample as i32 * w + o
        };
        v.clamp(0, 255) as u8
    }
    fn apply_chroma(&self, sample: u8, list: usize, refi: usize, cc: usize) -> u8 {
        let (w, o) = self.chroma_wo(list, refi, cc);
        let cwd = self.chroma_log2_denom;
        let v = if cwd >= 1 {
            ((sample as i32 * w + (1 << (cwd - 1))) >> cwd) + o
        } else {
            sample as i32 * w + o
        };
        v.clamp(0, 255) as u8
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MbError {
    Truncated,
    Unsupported(&'static str),
}
impl From<OutOfData> for MbError {
    fn from(_: OutOfData) -> Self {
        MbError::Truncated
    }
}
#[derive(Default)]
pub struct GridPool {
    job_pool: Vec<Box<PInterJob>>,
    nores_pool: Vec<Box<PInterNoResJob>>,
    bits_per_mb: f64,
    mb_qp: Vec<u8>,
    bs_frame: Vec<crate::h264::common::deblock::MbBs>,
    pk_prev: Vec<crate::h264::common::deblock::MbPack>,
    pk_cur: Vec<crate::h264::common::deblock::MbPack>,
    nnz_dbr: Vec<u8>,
    bak_y: Vec<u8>,
    bak_u: Vec<u8>,
    bak_v: Vec<u8>,
    nnz_y: Vec<u8>,
    nnz_c0: Vec<u8>,
    nnz_c1: Vec<u8>,
    modes_y: Vec<u8>,
    coded_y: Vec<bool>,
    mv_y: Vec<(i32, i32)>,
    inter_y: Vec<bool>,
    ref_idx_y: Vec<i32>,
    mv1: Vec<(i32, i32)>,
    ref_idx1: Vec<i32>,
    mb_t8x8: Vec<bool>,
    mb_kind: Vec<u8>,
    bzero: Vec<bool>,
    sc_cat: Vec<u8>,
    sc_cbp: Vec<u8>,
    sc_cmode: Vec<i32>,
    sc_nzc: Vec<[u8; 24]>,
    sc_cbfdc: Vec<u16>,
    sc_skip: Vec<bool>,
    sc_ref: Vec<[i8; 16]>,
    sc_mvd: Vec<[[i16; 2]; 16]>,
    sc_ref1: Vec<[i8; 16]>,
    sc_mvd1: Vec<[[i16; 2]; 16]>,
    sc_direct: Vec<bool>,
    ref_poc0: Vec<i32>,
    ref_poc1: Vec<i32>,
}
#[inline]
fn refill<T: Clone>(mut v: Vec<T>, n: usize, val: T) -> Vec<T> {
    v.clear();
    v.resize(n, val);
    v
}
impl FrameDecoder {
    pub fn with_pool(
        mb_w: usize,
        mb_h: usize,
        qp: u8,
        chroma_qp_offset: i32,
        refs: Vec<crate::h264::decoder::Ref>,
        num_ref_active: usize,
        constrained_intra: bool,
        transform_8x8_mode: bool,
        b_possible: bool,
        pool: GridPool,
    ) -> Self {
        let (cw, ch) = (mb_w * 16, mb_h * 16);
        let (ccw, cch) = (cw / 2, ch / 2);
        let bits_per_mb = pool.bits_per_mb;
        let ref_poc0 = {
            let mut v = pool.ref_poc0;
            v.clear();
            v.extend(refs.iter().map(|f| f.pic_poc()));
            v
        };
        Self {
            mb_w,
            mb_h,
            qp,
            cur_qp: qp,
            chroma_qp_offset,
            cw,
            ch,
            ccw,
            cch,
            rec_y: vec![0; cw * ch],
            rec_u: vec![0; ccw * cch],
            rec_v: vec![0; ccw * cch],
            mb_qp: refill(pool.mb_qp, mb_w * mb_h, qp),
            slice_first_mb: 0,
            nnz_y: refill(pool.nnz_y, (mb_w * 4) * (mb_h * 4), 0),
            nnz_c: [
                refill(pool.nnz_c0, (mb_w * 2) * (mb_h * 2), 0),
                refill(pool.nnz_c1, (mb_w * 2) * (mb_h * 2), 0),
            ],
            modes_y: refill(pool.modes_y, (mb_w * 4) * (mb_h * 4), 2),
            coded_y: refill(pool.coded_y, (mb_w * 4) * (mb_h * 4), false),
            mv_y: refill(pool.mv_y, (mb_w * 4) * (mb_h * 4), (0, 0)),
            inter_y: refill(pool.inter_y, (mb_w * 4) * (mb_h * 4), false),
            ref_idx_y: refill(pool.ref_idx_y, (mb_w * 4) * (mb_h * 4), -1),
            mv1: refill(pool.mv1, (mb_w * 4) * (mb_h * 4), (0, 0)),
            ref_idx1: refill(pool.ref_idx1, (mb_w * 4) * (mb_h * 4), -1),
            refs1: Vec::new(),
            num_ref_active1: 0,
            is_b: false,
            b_possible,
            direct_spatial: true,
            nnz_l_cache: [0x80; 25],
            nnz_c_cache: [[0x80; 9]; 2],
            refs,
            num_ref_active,
            constrained_intra,
            scaling: None,
            scaling8: None,
            transform_8x8_mode,
            transform_bypass: false,
            route_skip_mbs: 0,
            route_coded_mbs: 0,
            slice_bounds: Vec::new(),
            cur_idc2: false,
            mb_t8x8: refill(pool.mb_t8x8, mb_w * mb_h, false),
            bs_frame: refill(pool.bs_frame, mb_w * mb_h, Default::default()),
            bs_rows: 0,
            flt_rows: 0,
            pk_prev: {
                let mut v = pool.pk_prev;
                v.clear();
                v
            },
            pk_cur: {
                let mut v = pool.pk_cur;
                v.clear();
                v
            },
            nnz_dbr: refill(pool.nnz_dbr, (mb_w * 4) * (mb_h * 4), 0),
            bak_y: refill(pool.bak_y, cw, 0),
            bak_u: refill(pool.bak_u, ccw, 0),
            bak_v: refill(pool.bak_v, ccw, 0),
            edc_jobs: Vec::new(),
            edc_job_pool: pool.job_pool,
            edc_nores_pool: pool.nores_pool,
            scratch_luma: None,
            scratch_luma8: None,
            scratch_cac: None,
            edc_active: false,
            edc_tx: None,
            edc_ctx_rx: None,
            edc_back_tx: None,
            edc_parked: None,
            edc_regions: None,
            bsk_last: None,
            edc_batch: Vec::new(),
            bits_per_mb,
            db_ena: false,
            db_oa: 0,
            db_ob: 0,
            mb_kind: refill(
                pool.mb_kind,
                mb_w * mb_h,
                crate::h264::common::deblock::MB_KIND_UNSET,
            ),
            weights: None,
            row_wait: crate::h264::decoder::row_progress_on(),
            col_ok: false,
            col_w4: 0,
            any_t8: false,
            any_idc2: false,
            skip_zero_next: usize::MAX,
            bzero: refill(pool.bzero, mb_w * mb_h, false),
            sc_cat: pool.sc_cat,
            sc_cbp: pool.sc_cbp,
            sc_cmode: pool.sc_cmode,
            sc_nzc: pool.sc_nzc,
            sc_cbfdc: pool.sc_cbfdc,
            sc_skip: pool.sc_skip,
            sc_ref: pool.sc_ref,
            sc_mvd: pool.sc_mvd,
            sc_ref1: pool.sc_ref1,
            sc_mvd1: pool.sc_mvd1,
            sc_direct: pool.sc_direct,
            bzspan: None,
            pzspan: None,
            iw00: None,
            weights_id0: false,
            weights_l0id: false,
            cur_poc: 0,
            weighted_bipred_idc: 0,
            direct_8x8_inference: false,
            progress: None,
            ref_poc0,
            ref_poc1: {
                let mut v = pool.ref_poc1;
                v.clear();
                v
            },
        }
    }
    fn refresh_ref_pocs(&mut self) {
        self.ref_poc0.clear();
        self.ref_poc0.extend(self.refs.iter().map(|f| f.pic_poc()));
        self.ref_poc1.clear();
        self.ref_poc1.extend(self.refs1.iter().map(|f| f.pic_poc()));
    }
    pub fn set_progress_slot(&mut self, slot: crate::h264::decoder::Ref) {
        self.progress = Some(slot);
    }
    #[inline(always)]
    fn wait_refs_for_mb(&self, mb_y: usize) {
        if !self.row_wait {
            return;
        }
        let need = crate::h264::decoder::RefFrame::rows_needed_for_mb(mb_y, self.ch);
        crate::h264::decoder::RefFrame::set_mc_row_need(mb_y, self.ch);
        for r in self.refs.iter().chain(self.refs1.iter()) {
            if r.live.is_some() && r.frozen.get().is_none() {
                r.wait_ready_rows(need);
            }
        }
    }
    fn publish_progress(&mut self) {
        let Some(slot) = &self.progress else {
            return;
        };
        if self.edc_tx.is_some() {
            return;
        }
        if !rowdb_on() || !self.db_ena || self.flt_rows == 0 {
            return;
        }
        publish_filtered_rows_to_slot(
            slot,
            &self.rec_y,
            &self.rec_u,
            &self.rec_v,
            self.cw,
            self.ccw,
            self.ch,
            self.flt_rows,
        );
    }
    pub fn set_weights(&mut self, weights: WeightTable) {
        self.weights_id0 = weights.ref0_identity();
        self.weights_l0id = weights.list_identity(0);
        self.weights = Some(weights);
    }
    fn weight_partition(
        &self,
        pred_y: &mut [u8; 256],
        c_pred: &mut [[u8; 64]; 2],
        list: usize,
        refi: usize,
        rx: usize,
        ry: usize,
        rw: usize,
        rh: usize,
    ) {
        let Some(wt) = &self.weights else { return };
        if list == 0 && self.weights_l0id && !no_skipfp() {
            return;
        }
        let (lw, lo) = wt.luma_wo(list, refi);
        weight_block(pred_y, 16, rx, ry, rw, rh, lw, lo, wt.luma_log2_denom);
        let (crx, cry, crw, crh) = (rx / 2, ry / 2, rw / 2, rh / 2);
        for cc in 0..2 {
            let (cw, co) = wt.chroma_wo(list, refi, cc);
            weight_block(
                &mut c_pred[cc & 1],
                8,
                crx,
                cry,
                crw,
                crh,
                cw,
                co,
                wt.chroma_log2_denom,
            );
        }
    }
    pub fn route_counters(&self) -> (u32, u32) {
        (self.route_skip_mbs, self.route_coded_mbs)
    }
    pub fn set_transform_bypass(&mut self, on: bool) {
        self.transform_bypass = on;
    }
    pub fn set_scaling(&mut self, scaling: [[i32; 16]; 6], scaling8: [[i32; 64]; 2]) {
        self.scaling = Some(scaling);
        self.scaling8 = Some(scaling8);
    }
    #[inline]
    fn dq_const(&self, qp: u8, list: usize) -> DequantQp {
        match &self.scaling {
            Some(s) => DequantQp::weighted(qp, &s[list]),
            None => DQ_FLAT[(qp as usize).min(51)],
        }
    }
    fn dequant_dc4(&self, level: i32, qp: u8, list: usize) -> i32 {
        crate::h264::common::transform::dequantize_dc4(
            level,
            qp,
            self.scaling.as_ref().map(|s| s[list][0]),
        )
    }
    fn dequant_chroma_dc(&self, levels: &[i32; 4], qp: u8, list: usize) -> [i32; 4] {
        match &self.scaling {
            Some(s) => inverse_quant_chroma_dc_weighted(levels, qp, s[list][0]),
            None => inverse_quant_chroma_dc(levels, qp),
        }
    }
    pub fn set_b_context(
        &mut self,
        refs1: Vec<crate::h264::decoder::Ref>,
        num_ref_active1: usize,
        direct_spatial: bool,
        cur_poc: i32,
        weighted_bipred_idc: u8,
        direct_8x8_inference: bool,
    ) {
        self.is_b = true;
        self.refs1 = refs1;
        self.num_ref_active1 = num_ref_active1;
        self.direct_spatial = direct_spatial;
        self.cur_poc = cur_poc;
        self.weighted_bipred_idc = weighted_bipred_idc;
        self.direct_8x8_inference = direct_8x8_inference;
        self.iw00 = None;
        self.col_ok = self
            .refs1
            .first()
            .is_some_and(|c| c.live.is_none() && !c.long_term && c.w4 != 0);
        self.col_w4 = self.refs1.first().map_or(0, |c| c.w4);
        self.refresh_ref_pocs();
    }
    fn iw00(&mut self) -> Option<(i32, i32)> {
        match self.iw00 {
            Some(v) => v,
            None => {
                let v = if self.refs.is_empty() || self.refs1.is_empty() {
                    None
                } else {
                    self.implicit_weights(0, 0)
                };
                self.iw00 = Some(v);
                v
            }
        }
    }
    fn step_qp(&mut self, delta: i32) -> Result<(), MbError> {
        self.route_coded_mbs += 1;
        self.cur_qp = (self.cur_qp as i32 + delta + 52).rem_euclid(52) as u8;
        if self.transform_bypass && self.cur_qp == 0 {
            return Err(MbError::Unsupported(
                "transform-bypass (lossless) macroblock",
            ));
        }
        Ok(())
    }
    fn chroma_qp_for(&self, qp_y: u8) -> u8 {
        let qpi = (qp_y as i32 + self.chroma_qp_offset).clamp(0, 51) as u8;
        chroma_qp(qpi)
    }
    pub fn begin_slice(
        &mut self,
        slice_qp: u8,
        refs: Vec<crate::h264::decoder::Ref>,
        num_ref_active: usize,
    ) {
        self.cur_qp = slice_qp;
        self.qp = slice_qp;
        self.refs = refs;
        self.num_ref_active = num_ref_active;
        self.weights = None;
        self.weights_id0 = false;
        self.weights_l0id = false;
        self.refresh_ref_pocs();
    }
    #[inline]
    fn nbr_in_slice(&self, nbx: usize, nby: usize) -> bool {
        nby * self.mb_w + nbx >= self.slice_first_mb
    }
    #[inline]
    fn intra_nbr_ok(&self, nbx: usize, nby: usize) -> bool {
        !self.constrained_intra
            || !self
                .inter_y
                .get(nby * (self.mb_w * 4) + nbx)
                .copied()
                .unwrap_or(true)
    }
    fn mv_neighbors(&self, mb_x: usize, mb_y: usize) -> [MvNeighbor; 3] {
        let w4 = self.mb_w * 4;
        let get = |avail: bool, bx: isize, by: isize| {
            if avail {
                let idx = by as usize * w4 + bx as usize;
                match (self.mv_y.get(idx), self.ref_idx_y.get(idx)) {
                    (Some(&m), Some(&r)) => MvNeighbor {
                        available: true,
                        mv: m,
                        ref_idx: r,
                    },
                    _ => MvNeighbor::NONE,
                }
            } else {
                MvNeighbor::NONE
            }
        };
        let (bx, by) = (mb_x as isize * 4, mb_y as isize * 4);
        let a = get(mb_x > 0 && self.nbr_in_slice(mb_x - 1, mb_y), bx - 1, by);
        let b = get(mb_y > 0 && self.nbr_in_slice(mb_x, mb_y - 1), bx, by - 1);
        let c = if mb_y > 0 && mb_x + 1 < self.mb_w && self.nbr_in_slice(mb_x + 1, mb_y - 1) {
            get(true, bx + 4, by - 1)
        } else {
            get(
                mb_x > 0 && mb_y > 0 && self.nbr_in_slice(mb_x - 1, mb_y - 1),
                bx - 1,
                by - 1,
            )
        };
        [a, b, c]
    }
    fn mv_neighbors_block(&self, pbx: isize, pby: isize, pwb: isize) -> [MvNeighbor; 3] {
        let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Neighbors);
        self.mv_neighbors_block_grid(pbx, pby, pwb, 0)
    }
    fn skip_mv(&self, mb_x: usize, mb_y: usize) -> (i32, i32) {
        let [a, b, c] = self.mv_neighbors(mb_x, mb_y);
        if !a.available
            || !b.available
            || (a.ref_idx == 0 && a.mv == (0, 0))
            || (b.ref_idx == 0 && b.mv == (0, 0))
        {
            (0, 0)
        } else {
            predict_mv(a, b, c, 0)
        }
    }
    fn set_mb_mv(&mut self, mb_x: usize, mb_y: usize, mv: (i32, i32), inter: bool, refi: i32) {
        let w4 = self.mb_w * 4;
        let r = if inter { refi } else { -1 };
        for dy in 0..4 {
            let a = (mb_y * 4 + dy) * w4 + mb_x * 4;
            self.mv_y[a..a + 4].fill(mv);
            self.inter_y[a..a + 4].fill(inter);
            self.ref_idx_y[a..a + 4].fill(r);
        }
    }
    fn commit_inter_grid(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        rx: usize,
        ry: usize,
        rw: usize,
        rh: usize,
        mv: (i32, i32),
        refi: i8,
    ) {
        let w4 = self.mb_w * 4;
        let (bx0, bw) = (mb_x * 4 + rx / 4, rw / 4);
        for by in ry / 4..ry / 4 + rh / 4 {
            let a = (mb_y * 4 + by) * w4 + bx0;
            self.mv_y[a..a + bw].fill(mv);
            self.inter_y[a..a + bw].fill(true);
            self.ref_idx_y[a..a + bw].fill(refi as i32);
            self.coded_y[a..a + bw].fill(true);
        }
    }
    pub fn set_deblock_params(&mut self, ena: bool, oa: i32, ob: i32, idc2: bool) {
        self.db_ena = ena && (self.flt_rows == 0 || self.db_ena);
        self.db_oa = oa;
        self.db_ob = ob;
        self.cur_idc2 = idc2;
    }
    fn slice_of(&self, addr: usize) -> usize {
        match self.slice_bounds.binary_search_by(|&(f, _)| f.cmp(&addr)) {
            Ok(i) => i,
            Err(i) => i - 1,
        }
    }
    fn derive_bs_row(&mut self, r: usize) {
        self.span_flush();
        let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DebDerive);
        use crate::h264::common::deblock::{
            BlockInfo, MbBs, MbKind, derive_mb_kind, derive_mb_records, pack_mb,
        };
        let (mb_w, w4) = (self.mb_w, self.mb_w * 4);
        if self.any_t8 {
            for br in r * 4..r * 4 + 4 {
                let a = br * w4;
                self.nnz_dbr[a..a + w4].copy_from_slice(&self.nnz_y[a..a + w4]);
            }
            let t8_row_pre = &self.mb_t8x8[r * mb_w..][..mb_w];
            for mb_x in 0..mb_w {
                if !t8_row_pre[mb_x] {
                    continue;
                }
                let base = r * 4 * w4 + mb_x * 4;
                let mut src = [[0u8; 4]; 4];
                for (k, row) in src.iter_mut().enumerate() {
                    row.copy_from_slice(&self.nnz_y[base + k * w4..][..4]);
                }
                let mut out = [[0u8; 4]; 4];
                for b8 in 0..4usize {
                    let (cx, cy) = ((b8 % 2) * 2, (b8 / 2) * 2);
                    let any =
                        (src[cy][cx] | src[cy][cx + 1] | src[cy + 1][cx] | src[cy + 1][cx + 1]) > 0;
                    for sy in 0..2 {
                        for sx in 0..2 {
                            out[cy + sy][cx + sx] = any as u8;
                        }
                    }
                }
                for (k, row) in out.iter().enumerate() {
                    self.nnz_dbr[base + k * w4..][..4].copy_from_slice(row);
                }
            }
        }
        let info = BlockInfo {
            inter: &self.inter_y,
            nnz: if self.any_t8 {
                &self.nnz_dbr
            } else {
                &self.nnz_y
            },
            mv: &self.mv_y,
            ref_id: &self.ref_idx_y,
            mv1: &self.mv1,
            ref_id1: if self.ref_poc1.is_empty() {
                &[]
            } else {
                &self.ref_idx1
            },
            w4,
            t8x8: &self.mb_t8x8,
            bs: &[],
            poc0: &self.ref_poc0,
            poc1: &self.ref_poc1,
            kind: &self.mb_kind,
        };
        let has1 = !info.ref_id1.is_empty();
        core::mem::swap(&mut self.pk_prev, &mut self.pk_cur);
        self.pk_cur.clear();
        let kl = kind_loads();
        let stats = false;
        let row0 = r * mb_w;
        let bs_row = &mut self.bs_frame[row0..][..mb_w];
        let t8_row = &self.mb_t8x8[row0..][..mb_w];
        let kind_row: &[u8] = if self.mb_kind.is_empty() {
            &[]
        } else {
            &self.mb_kind[row0..][..mb_w]
        };
        for mb_x in 0..mb_w {
            self.pk_cur.push(pack_mb(&info, has1, mb_x, r));

            match kind_row.get(mb_x).copied().and_then(MbKind::from_u8) {
                Some(MbKind::Intra) => {
                    bs_row[mb_x] = crate::h264::common::deblock::intra_mb_bs(mb_x, r);
                }
                Some(k @ (MbKind::Skip | MbKind::InterUniform)) if kl => {
                    bs_row[mb_x] = derive_mb_kind(&info, mb_x, r, k);
                }
                _ => {
                    let Some(cur) = self.pk_cur.get(mb_x) else {
                        continue;
                    };
                    let left = if mb_x > 0 {
                        self.pk_cur.get(mb_x - 1)
                    } else {
                        None
                    };
                    let top = if r > 0 { self.pk_prev.get(mb_x) } else { None };
                    let mb_t8 = t8_row[mb_x];
                    let (mut bv, mut bh) = ([[0i32; 4]; 4], [[0i32; 4]; 4]);
                    crate::h264::common::deblock::census_note_packed();
                    let flat = derive_mb_records(cur, left, top, mb_t8, &mut bv, &mut bh);

                    if stats
                        && matches!(
                            kind_row.get(mb_x).copied().and_then(MbKind::from_u8),
                            Some(MbKind::Skip | MbKind::InterUniform)
                        )
                    {}
                    let m = if flat {
                        debug_assert!(bv[1..] == [[0i32; 4]; 3] && bh[1..] == [[0i32; 4]; 3]);
                        let mut m = MbBs::default();
                        m.v[0] = core::array::from_fn(|sg| bv[0][sg] as u8);
                        m.h[0] = core::array::from_fn(|sg| bh[0][sg] as u8);
                        m
                    } else {
                        MbBs {
                            v: core::array::from_fn(|e| core::array::from_fn(|sg| bv[e][sg] as u8)),
                            h: core::array::from_fn(|e| core::array::from_fn(|sg| bh[e][sg] as u8)),
                        }
                    };
                    if stats && m.v == [[0u8; 4]; 4] && m.h == [[0u8; 4]; 4] {}
                    bs_row[mb_x] = m;
                }
            }
        }
        if self.any_idc2 && self.slice_bounds.len() > 1 {
            for mb_x in 0..mb_w {
                let slot = r * mb_w + mb_x;
                let si = self.slice_of(slot);
                if !self.slice_bounds.get(si).is_some_and(|b| b.1) {
                    continue;
                }
                if mb_x > 0 && self.slice_of(slot - 1) != si {
                    if let Some(b) = self.bs_frame.get_mut(slot) {
                        b.v[0] = [0; 4];
                    }
                }
                if r > 0 && self.slice_of(slot - mb_w) != si {
                    if let Some(b) = self.bs_frame.get_mut(slot) {
                        b.h[0] = [0; 4];
                    }
                }
            }
        }
    }
    #[inline(always)]
    fn row_hook_at(&mut self, addr: usize, mby: usize) {
        if rowdb_on() {
            if !rowhook_eager() && mby <= self.bs_rows {
                return;
            }
        }
        self.row_hook(addr, mby);
    }
    fn row_hook(&mut self, addr: usize, mby: usize) {
        debug_assert_eq!(mby, addr / self.mb_w, "row_hook: mby must be addr's row");
        let eager = rowhook_eager();
        if !rowdb_on() {
            let _rh =
                crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecRowHook);
            self.edc_flush();
            let done = mby;
            if done > self.bs_rows {
                self.bs_rows = done;
                self.publish_progress();
            }
            return;
        }
        let done = mby;
        if !eager && done <= self.bs_rows {
            return;
        }
        let _rh = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecRowHook);
        if done <= self.bs_rows {
            return;
        }
        if self.edc_tx.is_some() {
            self.edc_giveback();
            while self.bs_rows < done {
                let r = self.bs_rows;
                self.derive_bs_row(r);
                self.bs_rows += 1;
                let base = r * self.mb_w;
                self.edc_flush_batch();
                let msg = EdcMsg::Row {
                    r,
                    bs: self.bs_frame[base..base + self.mb_w].to_vec(),
                    qp: self.mb_qp[base..base + self.mb_w].to_vec(),
                    t8: self.mb_t8x8[base..base + self.mb_w].to_vec(),
                };
                self.edc_tx
                    .as_ref()
                    .unwrap()
                    .send(msg)
                    .expect("worker alive");
            }
            return;
        }
        self.edc_flush();
        while self.bs_rows < done {
            let r = self.bs_rows;
            self.derive_bs_row(r);
            self.bs_rows += 1;
            if self.db_ena {
                self.save_bak(r);
                self.filter_row(r);
                self.flt_rows = r + 1;
            }
            self.publish_progress();
        }
    }
    fn save_bak(&mut self, r: usize) {
        let y0 = (r * 16 + 15) * self.cw;
        self.bak_y.copy_from_slice(&self.rec_y[y0..y0 + self.cw]);
        let c0 = (r * 8 + 7) * self.ccw;
        self.bak_u.copy_from_slice(&self.rec_u[c0..c0 + self.ccw]);
        self.bak_v.copy_from_slice(&self.rec_v[c0..c0 + self.ccw]);
    }
    fn filter_row(&mut self, r: usize) {
        let info = crate::h264::common::deblock::BlockInfo {
            inter: &self.inter_y,
            nnz: if self.any_t8 {
                &self.nnz_dbr
            } else {
                &self.nnz_y
            },
            mv: &self.mv_y,
            ref_id: &self.ref_idx_y,
            mv1: &self.mv1,
            ref_id1: &self.ref_idx1,
            w4: self.mb_w * 4,
            t8x8: &self.mb_t8x8,
            bs: &self.bs_frame,
            poc0: &[],
            poc1: &[],
            kind: &self.mb_kind,
        };
        crate::h264::common::deblock::filter_frame_rows_pre(
            &mut self.rec_y,
            &mut self.rec_u,
            &mut self.rec_v,
            self.mb_w,
            self.mb_h,
            r..r + 1,
            &self.mb_qp,
            self.chroma_qp_offset,
            self.db_oa,
            self.db_ob,
            &info,
        );
    }
    #[inline]
    fn top_y_px(&self, py: usize, x: usize) -> u8 {
        if py % 16 == 0 && self.flt_rows * 16 >= py {
            self.bak_y.get(x).copied().unwrap_or(128)
        } else {
            self.rec_y
                .get((py - 1) * self.cw + x)
                .copied()
                .unwrap_or(128)
        }
    }
    #[inline]
    fn top_y_row(&self, py: usize, x: usize, n: usize) -> &[u8] {
        if py % 16 == 0 && self.flt_rows * 16 >= py {
            &self.bak_y[x..x + n]
        } else {
            &self.rec_y[(py - 1) * self.cw + x..][..n]
        }
    }
    #[inline]
    fn top_c_px(&self, c: usize, cy: usize, x: usize) -> u8 {
        if cy % 8 == 0 && self.flt_rows * 8 >= cy {
            let bak = if c == 0 { &self.bak_u } else { &self.bak_v };
            bak.get(x).copied().unwrap_or(128)
        } else {
            let rec = if c == 0 { &self.rec_u } else { &self.rec_v };
            rec.get((cy - 1) * self.ccw + x).copied().unwrap_or(128)
        }
    }
    #[inline]
    fn top_c_row(&self, c: usize, cy: usize, x: usize, n: usize) -> &[u8] {
        if cy % 8 == 0 && self.flt_rows * 8 >= cy {
            if c == 0 {
                &self.bak_u[x..x + n]
            } else {
                &self.bak_v[x..x + n]
            }
        } else {
            let rec = if c == 0 { &self.rec_u } else { &self.rec_v };
            &rec[(cy - 1) * self.ccw + x..][..n]
        }
    }
    pub fn as_reference_pooled(&self, pool: &mut Vec<Vec<u8>>) -> crate::h264::decoder::RefFrame {
        if mv_dump_on() {
            MV_DUMP.lock().unwrap().push(MvField {
                mb_w: self.mb_w,
                mb_h: self.mb_h,
                mv: self.mv_y.clone(),
                ref_idx: self.ref_idx_y.clone(),
                inter: self.inter_y.clone(),
            });
        }
        let (mv, ref_idx, mv1, ref_idx1, ref_poc, w4) = if self.b_possible {
            (
                self.mv_y.clone(),
                self.ref_idx_y.clone(),
                self.mv1.clone(),
                self.ref_idx1.clone(),
                {
                    let mut poc_lut = [i32::MIN; 32];
                    for (i, f) in self.refs.iter().take(32).enumerate() {
                        poc_lut[i & 31] = f.pic_poc();
                    }
                    self.ref_idx_y
                        .iter()
                        .map(|&r| {
                            if (0..32).contains(&r) {
                                poc_lut[r as usize]
                            } else {
                                i32::MIN
                            }
                        })
                        .collect()
                },
                self.mb_w * 4,
            )
        } else {
            (
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                0,
            )
        };
        let mut take = |len: usize| -> Vec<u8> {
            match pool.iter().position(|v| v.len() == len) {
                Some(i) => pool.swap_remove(i),
                None => Vec::new(),
            }
        };
        let (lpw, lph) = (
            self.cw + 2 * crate::h264::decoder::LPAD,
            self.ch + 2 * crate::h264::decoder::LPAD,
        );
        let (cpw, cph) = (
            self.ccw + 2 * crate::h264::decoder::CPAD,
            self.ch / 2 + 2 * crate::h264::decoder::CPAD,
        );
        crate::h264::decoder::RefFrame {
            py: crate::h264::common::inter::pad_plane_into(
                take(lpw * lph),
                &self.rec_y,
                self.cw,
                self.ch,
                crate::h264::decoder::LPAD,
            ),
            pu: crate::h264::common::inter::pad_plane_into(
                take(cpw * cph),
                &self.rec_u,
                self.ccw,
                self.ch / 2,
                crate::h264::decoder::CPAD,
            ),
            pv: crate::h264::common::inter::pad_plane_into(
                take(cpw * cph),
                &self.rec_v,
                self.ccw,
                self.ch / 2,
                crate::h264::decoder::CPAD,
            ),
            cw: self.cw,
            ch: self.ch,
            ready_rows: core::sync::atomic::AtomicUsize::new(0),
            live: None,
            frozen: crate::h264::decoder::sync::Once::new(),
            frame_num: 0,
            poc: 0,
            mv,
            ref_idx,
            mv1,
            ref_idx1,
            ref_poc,
            w4,
            long_term: false,
            long_term_idx: 0,
        }
    }
    fn nnz_cache_load(&mut self, mb_x: usize, mb_y: usize) {
        let w4 = self.mb_w * 4;
        let top_unavail = mb_y == 0 || !self.nbr_in_slice(mb_x, mb_y - 1);
        let left_unavail = mb_x == 0 || !self.nbr_in_slice(mb_x - 1, mb_y);
        if top_unavail {
            self.nnz_l_cache[1..5].fill(0x80);
        } else {
            let src = &self.nnz_y[(mb_y * 4 - 1) * w4 + mb_x * 4..][..4];
            self.nnz_l_cache[1..5].copy_from_slice(src);
        }
        for lby in 0..4 {
            self.nnz_l_cache[(lby + 1) * 5] = if left_unavail {
                0x80
            } else {
                self.nnz_y
                    .get((mb_y * 4 + lby) * w4 + (mb_x * 4 - 1))
                    .copied()
                    .unwrap_or(0)
            };
        }
    }
    #[inline]
    fn nc_pred(&self, lbx: usize, lby: usize) -> i32 {
        let (lbx, lby) = (lbx & 3, lby & 3);
        let left = self.nnz_l_cache[(lby + 1) * 5 + lbx] as i32;
        let top = self.nnz_l_cache[lby * 5 + (lbx + 1)] as i32;
        let r = left + top;
        if r < 0x80 { (r + 1) >> 1 } else { r & 0x7f }
    }
    #[inline]
    fn nnz_cache_set(&mut self, lbx: usize, lby: usize, total: u8) {
        self.nnz_l_cache[((lby & 3) + 1) * 5 + ((lbx & 3) + 1)] = total;
    }
    fn chroma_cache_load(&mut self, mb_x: usize, mb_y: usize) {
        let w2 = self.mb_w * 2;
        let top_unavail = mb_y == 0 || !self.nbr_in_slice(mb_x, mb_y - 1);
        let left_unavail = mb_x == 0 || !self.nbr_in_slice(mb_x - 1, mb_y);
        for c in 0..2 {
            if top_unavail {
                self.nnz_c_cache[c][1..3].fill(0x80);
            } else {
                let src = &self.nnz_c[c][(mb_y * 2 - 1) * w2 + mb_x * 2..][..2];
                self.nnz_c_cache[c][1..3].copy_from_slice(src);
            }
            for by in 0..2 {
                self.nnz_c_cache[c][(by + 1) * 3] = if left_unavail {
                    0x80
                } else {
                    self.nnz_c[c & 1]
                        .get((mb_y * 2 + by) * w2 + (mb_x * 2 - 1))
                        .copied()
                        .unwrap_or(0)
                };
            }
        }
    }
    #[inline]
    fn chroma_nc_pred(&self, c: usize, bx: usize, by: usize) -> i32 {
        let (c, bx, by) = (c & 1, bx & 1, by & 1);
        let left = self.nnz_c_cache[c][(by + 1) * 3 + bx] as i32;
        let top = self.nnz_c_cache[c][by * 3 + (bx + 1)] as i32;
        let r = left + top;
        if r < 0x80 { (r + 1) >> 1 } else { r & 0x7f }
    }
    #[inline]
    fn chroma_nnz_cache_set(&mut self, c: usize, bx: usize, by: usize, total: u8) {
        self.nnz_c_cache[c & 1][((by & 1) + 1) * 3 + ((bx & 1) + 1)] = total;
    }
    pub fn decode_slice_data_cabac(
        &mut self,
        rbsp: &[u8],
        start_byte: usize,
        slice_qp: u8,
        cabac_init_idc: u32,
        is_i: bool,
        is_p: bool,
        first_mb: usize,
    ) -> Result<usize, MbError> {
        let eligible = edc_on() && rowdb_on() && !is_i && (is_p || self.is_b);
        let threaded = eligible && edc_spawn_worker(self.mb_w, self.mb_h, self.bits_per_mb, true);
        if !threaded {
            let r = self.decode_slice_cabac_inner(
                rbsp,
                start_byte,
                slice_qp,
                cabac_init_idc,
                is_i,
                is_p,
                first_mb,
            );
            self.note_slice_density(rbsp.len().saturating_sub(start_byte), first_mb, &r);
            return r;
        }
        {
            let ctx = self.edc_take_ctx();
            let (tx, rx) = crate::h264::decoder::sync::mpsc::sync_channel::<EdcMsg>(edc_bound());
            let (ctx_tx, ctx_rx) = crate::h264::decoder::sync::mpsc::channel::<PixelCtx>();
            let (back_tx, back_rx) = crate::h264::decoder::sync::mpsc::channel::<PixelCtx>();
            let (res, ctx, panicked) = std::thread::scope(|sc| {
                let h = sc.spawn(move || edc_worker(ctx, rx, ctx_tx, back_rx));
                self.edc_tx = Some(tx);
                self.edc_ctx_rx = Some(ctx_rx);
                self.edc_back_tx = Some(back_tx);
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.decode_slice_cabac_inner(
                        rbsp,
                        start_byte,
                        slice_qp,
                        cabac_init_idc,
                        is_i,
                        is_p,
                        first_mb,
                    )
                }));
                self.edc_flush_batch();
                self.edc_giveback();
                self.edc_tx = None;
                self.edc_ctx_rx = None;
                self.edc_back_tx = None;
                match (r, h.join()) {
                    (Ok(res), Ok(ctx)) => (res, Some(ctx), None),
                    (Err(p), Ok(ctx)) => (Err(MbError::Truncated), Some(ctx), Some(p)),
                    (Ok(_), Err(p)) | (Err(_), Err(p)) => (Err(MbError::Truncated), None, Some(p)),
                }
            });
            if let Some(ctx) = ctx {
                self.edc_restore_ctx(ctx);
            }
            if let Some(p) = panicked {
                std::panic::resume_unwind(p);
            }
            self.note_slice_density(rbsp.len().saturating_sub(start_byte), first_mb, &res);
            return res;
        }
    }
    fn note_slice_density(&mut self, bytes: usize, first_mb: usize, r: &Result<usize, MbError>) {
        let Ok(end) = r else { return };
        let mbs = end.saturating_sub(first_mb);
        if mbs == 0 {
            return;
        }
        let bpm = (bytes * 8) as f64 / mbs as f64;
        self.bits_per_mb = if self.bits_per_mb == 0.0 {
            bpm
        } else {
            0.75 * self.bits_per_mb + 0.25 * bpm
        };
    }
    fn decode_slice_cabac_inner(
        &mut self,
        rbsp: &[u8],
        start_byte: usize,
        slice_qp: u8,
        cabac_init_idc: u32,
        is_i: bool,
        is_p: bool,
        first_mb: usize,
    ) -> Result<usize, MbError> {
        self.edc_active = edc_on();
        let mut cab = crate::h264::decoder::cabac::Cabac::new(
            rbsp,
            start_byte,
            slice_qp as i32,
            cabac_init_idc,
            is_i,
        );
        let (range, _offset) = cab.dbg_state();
        let trace = false;
        debug_assert_eq!(range, 510, "CABAC init range must be 510");
        let mbw = self.mb_w;
        let total = self.mb_w * self.mb_h;
        let _alloc_g =
            crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecSliceAlloc);
        let mut cat = refill(core::mem::take(&mut self.sc_cat), total, 255u8);
        let mut mb_cbp = refill(core::mem::take(&mut self.sc_cbp), total, 0u8);
        let mut cmode = refill(core::mem::take(&mut self.sc_cmode), total, -1i32);
        let mut mb_nzc = refill(core::mem::take(&mut self.sc_nzc), total, [0u8; 24]);
        let mut cbf_dc = refill(core::mem::take(&mut self.sc_cbfdc), total, 0u16);
        let mut mb_skip = refill(core::mem::take(&mut self.sc_skip), total, false);
        let mut mb_ref = refill(core::mem::take(&mut self.sc_ref), total, [-1i8; 16]);
        let mut mb_mvd = refill(core::mem::take(&mut self.sc_mvd), total, [[0i16; 2]; 16]);
        let want_b_grids = self.is_b || fat_slice_on();
        let mut mb_ref1 = if want_b_grids {
            refill(core::mem::take(&mut self.sc_ref1), total, [-1i8; 16])
        } else {
            Vec::new()
        };
        let mut mb_mvd1 = if want_b_grids {
            refill(core::mem::take(&mut self.sc_mvd1), total, [[0i16; 2]; 16])
        } else {
            Vec::new()
        };
        let mut mb_direct = if want_b_grids {
            refill(core::mem::take(&mut self.sc_direct), total, false)
        } else {
            Vec::new()
        };
        drop(_alloc_g);
        self.slice_first_mb = first_mb;
        self.slice_bounds.push((first_mb, self.cur_idc2));
        self.any_idc2 |= self.cur_idc2;
        let mut last_delta_qp = 0i32;
        let mut addr = first_mb;
        let _mbloop_g =
            crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecMbLoop);
        self.span_flush();
        let mbw_nz = mbw.max(1);
        let (mut mbx, mut mby) = (addr % mbw_nz, addr / mbw_nz);
        if self.is_b {
            self.edc_flush();
        }
        let b_records = self.edc_tx.is_some();
        let b_refs_ok = !self.refs.is_empty() && !self.refs1.is_empty();
        let top_lim = first_mb + mbw;
        let mut pred_y = [0u8; 256];
        let mut c_pred = [[0u8; 64]; 2];
        if addr >= total {
            return Err(MbError::Truncated);
        }
        {
            let cat = &mut cat[..total];
            let mb_cbp = &mut mb_cbp[..total];
            let cmode = &mut cmode[..total];
            let mb_nzc = &mut mb_nzc[..total];
            let cbf_dc = &mut cbf_dc[..total];
            let mb_skip = &mut mb_skip[..total];
            let mb_ref = &mut mb_ref[..total];
            let mb_mvd = &mut mb_mvd[..total];
            loop {
                if addr >= total {
                    break;
                }
                if mbx == mbw {
                    mbx = 0;
                    mby += 1;
                }
                self.row_hook_at(addr, mby);
                self.wait_refs_for_mb(mby);
                let left = (mbx > 0 && addr > first_mb).then(|| addr - 1);
                let top = (mby > 0 && addr >= top_lim).then(|| addr - mbw);
                let mb_type;
                if is_p {
                    let sctx = 11
                        + (left.is_some() && !mb_skip.get(addr - 1).copied().unwrap_or(true))
                            as usize
                        + (top.is_some()
                            && !mb_skip.get(addr.wrapping_sub(mbw)).copied().unwrap_or(true))
                            as usize;
                    let (data, mut e, ctx) = cab.view();
                    if e.decode_decision(data, ctx, sctx) != 0 {
                        if let Some(p) = mb_skip.get_mut(addr) {
                            *p = true;
                        }
                        last_delta_qp = 0;
                        self.decode_p_skip(mbx, mby)?;
                        if let Some(p) = self.mb_qp.get_mut(addr) {
                            *p = self.cur_qp;
                        }
                        let eos = e.decode_terminate(data);
                        cab.commit(e);
                        addr += 1;
                        mbx += 1;
                        if eos || addr >= total {
                            break;
                        }
                        continue;
                    }
                    let mbt = mb_type_p_eng(&mut e, data, ctx);
                    cab.commit(e);
                    self.span_flush();
                    if mbt <= 3 {
                        let _gb = crate::h264::common::prof::scope(
                            crate::h264::common::prof::Stage::DecMbP,
                        );
                        let mut allow8 = true;
                        let mut mvdc = [[0i16; 2]; 30];
                        let mut refc = [-1i8; 30];
                        if let Some(l) = left {
                            let (Some(lr), Some(lm)) = (mb_ref.get(l), mb_mvd.get(l)) else {
                                return Err(MbError::Truncated);
                            };
                            for (ci, bi) in [(6usize, 3usize), (12, 7), (18, 11), (24, 15)] {
                                refc[ci] = lr[bi];
                                mvdc[ci] = lm[bi];
                            }
                        }
                        if let Some(t) = top {
                            let (Some(tr), Some(tm)) = (mb_ref.get(t), mb_mvd.get(t)) else {
                                return Err(MbError::Truncated);
                            };
                            for (ci, bi) in [(1usize, 12usize), (2, 13), (3, 14), (4, 15)] {
                                refc[ci] = tr[bi];
                                mvdc[ci] = tm[bi];
                            }
                        }
                        if mbx > 0 && mby > 0 {
                            let a = addr - mbw - 1;
                            if let (Some(r), Some(m)) = (mb_ref.get(a), mb_mvd.get(a)) {
                                (refc[0], mvdc[0]) = (r[15], m[15]);
                            }
                        }
                        if mby > 0 && mbx + 1 < mbw {
                            let a = addr - mbw + 1;
                            if let (Some(r), Some(m)) = (mb_ref.get(a), mb_mvd.get(a)) {
                                (refc[5], mvdc[5]) = (r[12], m[12]);
                            }
                        }
                        let mut mmvd = [[0i16; 2]; 16];
                        let mut mref = [0i8; 16];
                        macro_rules! refidx {
                            ($ pi : expr , $ zb : expr) => {{
                                if self.num_ref_active > 1 {
                                    let s = CACHE30[$pi & 15].clamp(6, 29);
                                    let c0 =
                                        (refc[s - 1] > 0) as usize + 2 * (refc[s - 6] > 0) as usize;
                                    let r = parse_ref_idx_cabac(&mut cab, c0);
                                    for &zb in $zb.iter() {
                                        refc[CACHE30[zb & 15]] = r;
                                    }
                                    r
                                } else {
                                    0i8
                                }
                            }};
                        }
                        macro_rules! part {
                            ($ pi : expr , $ zb : expr , $ pred : expr , $ rx : expr , $ ry : expr , $ rw : expr , $ rh : expr , $ refi : expr) => {{
                                let (mvx, mvy) = parse_mvd_partition(
                                    &mut cab, $pi, $zb, &mut mvdc, &mut refc, &mut mmvd, &mut mref,
                                    $refi,
                                );
                                let [na, nb, nc] = self.mv_neighbors_block(
                                    (mbx * 4 + $rx / 4) as isize,
                                    (mby * 4 + $ry / 4) as isize,
                                    ($rw / 4) as isize,
                                );
                                let pmv = $pred(na, nb, nc);
                                self.commit_inter_grid(
                                    mbx,
                                    mby,
                                    $rx,
                                    $ry,
                                    $rw,
                                    $rh,
                                    (pmv.0 + mvx, pmv.1 + mvy),
                                    $refi,
                                );
                            }};
                        }
                        match mbt {
                            0 => {
                                if let Some(k) = self.mb_kind.get_mut(mby * self.mb_w + mbx) {
                                    *k = crate::h264::common::deblock::MB_KIND_INTER_UNIFORM;
                                }
                                let r0 = refidx!(
                                    0,
                                    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
                                );
                                part!(
                                    0,
                                    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
                                    |a, b, c| predict_partition_mv(0, 0, a, b, c, r0 as i32),
                                    0,
                                    0,
                                    16,
                                    16,
                                    r0
                                );
                            }
                            1 => {
                                let r0 = refidx!(0, &[0, 1, 2, 3, 4, 5, 6, 7]);
                                let r1 = refidx!(8, &[8, 9, 10, 11, 12, 13, 14, 15]);
                                part!(
                                    0,
                                    &[0, 1, 2, 3, 4, 5, 6, 7],
                                    |a, b, c| predict_partition_mv(1, 0, a, b, c, r0 as i32),
                                    0,
                                    0,
                                    16,
                                    8,
                                    r0
                                );
                                part!(
                                    8,
                                    &[8, 9, 10, 11, 12, 13, 14, 15],
                                    |a, b, c| predict_partition_mv(1, 1, a, b, c, r1 as i32),
                                    0,
                                    8,
                                    16,
                                    8,
                                    r1
                                );
                            }
                            2 => {
                                let r0 = refidx!(0, &[0, 1, 2, 3, 8, 9, 10, 11]);
                                let r1 = refidx!(4, &[4, 5, 6, 7, 12, 13, 14, 15]);
                                part!(
                                    0,
                                    &[0, 1, 2, 3, 8, 9, 10, 11],
                                    |a, b, c| predict_partition_mv(2, 0, a, b, c, r0 as i32),
                                    0,
                                    0,
                                    8,
                                    16,
                                    r0
                                );
                                part!(
                                    4,
                                    &[4, 5, 6, 7, 12, 13, 14, 15],
                                    |a, b, c| predict_partition_mv(2, 1, a, b, c, r1 as i32),
                                    8,
                                    0,
                                    8,
                                    16,
                                    r1
                                );
                            }
                            _ => {
                                let mut subt = [0u32; 4];
                                for st in &mut subt {
                                    *st = parse_sub_mb_type_p_cabac(&mut cab);
                                }
                                allow8 = subt.iter().all(|&t| t == 0);
                                let mut pr = [0i8; 4];
                                for (i, r) in pr.iter_mut().enumerate() {
                                    let b = i * 4;
                                    *r = refidx!(b, &[b, b + 1, b + 2, b + 3]);
                                }
                                for i in 0..4usize {
                                    let b = i * 4;
                                    let (ox, oy) = ((i % 2) * 8, (i / 2) * 8);
                                    let ri = pr[i];
                                    match subt[i] {
                                        0 => part!(
                                            b,
                                            &[b, b + 1, b + 2, b + 3],
                                            |a, b, c| predict_mv(a, b, c, ri as i32),
                                            ox,
                                            oy,
                                            8,
                                            8,
                                            ri
                                        ),
                                        1 => {
                                            part!(
                                                b,
                                                &[b, b + 1],
                                                |a, b, c| predict_mv(a, b, c, ri as i32),
                                                ox,
                                                oy,
                                                8,
                                                4,
                                                ri
                                            );
                                            part!(
                                                b + 2,
                                                &[b + 2, b + 3],
                                                |a, b, c| predict_mv(a, b, c, ri as i32),
                                                ox,
                                                oy + 4,
                                                8,
                                                4,
                                                ri
                                            );
                                        }
                                        2 => {
                                            part!(
                                                b,
                                                &[b, b + 2],
                                                |a, b, c| predict_mv(a, b, c, ri as i32),
                                                ox,
                                                oy,
                                                4,
                                                8,
                                                ri
                                            );
                                            part!(
                                                b + 1,
                                                &[b + 1, b + 3],
                                                |a, b, c| predict_mv(a, b, c, ri as i32),
                                                ox + 4,
                                                oy,
                                                4,
                                                8,
                                                ri
                                            );
                                        }
                                        _ => {
                                            for j in 0..4usize {
                                                let (sx, sy) = ((j % 2) * 4, (j / 2) * 4);
                                                part!(
                                                    b + j,
                                                    &[b + j],
                                                    |a, b, c| predict_mv(a, b, c, ri as i32),
                                                    ox + sx,
                                                    oy + sy,
                                                    4,
                                                    4,
                                                    ri
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(p) = mb_ref.get_mut(addr) {
                            *p = mref;
                        }
                        if let Some(p) = mb_mvd.get_mut(addr) {
                            *p = mmvd;
                        }
                        let cbp = parse_cbp_cabac(
                            &mut cab,
                            top.and_then(|a| mb_cbp.get(a).copied()),
                            left.and_then(|a| mb_cbp.get(a).copied()),
                        );
                        if let Some(p) = mb_cbp.get_mut(addr) {
                            *p = cbp as u8;
                        }
                        let t8 = self.transform_8x8_mode && (cbp & 15) != 0 && allow8 && {
                            let a = left
                                .map_or(0, |x| self.mb_t8x8.get(x).is_some_and(|&f| f) as usize);
                            let b =
                                top.map_or(0, |x| self.mb_t8x8.get(x).is_some_and(|&f| f) as usize);
                            cab.decode_decision(399 + a + b) != 0
                        };
                        if let Some(p) = self.mb_t8x8.get_mut(addr) {
                            *p = t8;
                        }
                        self.any_t8 |= t8;
                        if cbp == 0 && nores_on() {
                            last_delta_qp = 0;
                            if let Some(p) = self.mb_qp.get_mut(addr) {
                                *p = self.cur_qp;
                            }
                            if let Some(p) = cbf_dc.get_mut(addr) {
                                *p = 0;
                            }
                            if let Some(p) = mb_nzc.get_mut(addr) {
                                *p = [0u8; 24];
                            }
                            if self.refs.is_empty() {
                                return Err(MbError::Unsupported("inter without reference"));
                            }
                            let (mut jgmv, mut jgref) = ([(0i32, 0i32); 16], [0u8; 16]);
                            {
                                let w4r = self.mb_w * 4;
                                for by in 0..4usize {
                                    let row = (mby * 4 + by) * w4r + mbx * 4;
                                    jgmv[by * 4..by * 4 + 4]
                                        .copy_from_slice(&self.mv_y[row..row + 4]);
                                    let ridx = &self.ref_idx_y[row..row + 4];
                                    for bx in 0..4usize {
                                        jgref[by * 4 + bx] = ridx[bx].clamp(0, 15) as u8;
                                    }
                                }
                            }
                            let pj = PInterNoResJob {
                                mbx,
                                mby,
                                t8,
                                gmv: jgmv,
                                gref: jgref,
                            };
                            if self.edc_tx.is_some() {
                                self.edc_giveback();
                                self.edc_commit_nnz(mbx, mby, t8, &[0u8; 24], 0);

                                self.edc_send_job(EdcJob::InterNoRes(Box::new(pj)));
                            } else if self.edc_active {
                                self.edc_jobs.push(EdcJob::InterNoRes(Box::new(pj)));
                            } else {
                                self.recon_p_inter_nores(&pj);
                                if double_recon() {
                                    self.recon_p_inter_nores(&pj);
                                }
                            }
                            let eos = cab.decode_terminate();
                            addr += 1;
                            mbx += 1;
                            if eos || addr >= total {
                                break;
                            }
                            continue;
                        }
                        let mut job = self.take_pinter_job();
                        job.t8 = t8;
                        let (cbp_luma, cbp_chroma) = (cbp & 15, cbp >> 4);
                        let mut nzc = [0xffu8; 48];
                        if let Some(t) = top {
                            let tnz = mb_nzc.get(t).unwrap_or(&ZERO_NZC);
                            nzc[1..5].copy_from_slice(&tnz[12..16]);
                            (nzc[0], nzc[5], nzc[29]) = (0, 0, 0);
                            (nzc[6], nzc[7], nzc[30], nzc[31]) =
                                (tnz[20], tnz[21], tnz[22], tnz[23]);
                        }
                        if let Some(l) = left {
                            let lnz = mb_nzc.get(l).unwrap_or(&ZERO_NZC);
                            (nzc[8], nzc[16], nzc[24], nzc[32]) =
                                (lnz[3], lnz[7], lnz[11], lnz[15]);
                            (nzc[13], nzc[21], nzc[37], nzc[45]) =
                                (lnz[17], lnz[21], lnz[19], lnz[23]);
                        }
                        let mut cbfdc = 0u16;
                        job.nnzs = [0u8; 24];
                        if cbp == 0 {
                            last_delta_qp = 0;
                        }
                        if cbp != 0 {
                            let ndc = (
                                top.and_then(|a| cbf_dc.get(a).copied()),
                                left.and_then(|a| cbf_dc.get(a).copied()),
                            );
                            let qpd = parse_mb_qp_delta_cabac(&mut cab, &mut last_delta_qp);
                            self.step_qp(qpd)?;
                            parse_mb_residual_cabac::<false>(
                                &mut cab,
                                &mut nzc,
                                &mut cbfdc,
                                ndc,
                                cbp_luma,
                                cbp_chroma,
                                t8,
                                ResidualOut {
                                    luma: &mut job.luma_scan,
                                    luma8: &mut job.luma8,
                                    cdc: &mut job.cdc,
                                    cac: &mut job.cac,
                                    nnzs: &mut job.nnzs,
                                },
                            );
                        }
                        if let Some(p) = self.mb_qp.get_mut(addr) {
                            *p = self.cur_qp;
                        }
                        if let Some(p) = cbf_dc.get_mut(addr) {
                            *p = cbfdc;
                        }
                        let _sc = crate::h264::common::prof::scope(
                            crate::h264::common::prof::Stage::DecStateCache,
                        );
                        let mn = nnz_raster_from_z(&job.nnzs);
                        if let Some(p) = mb_nzc.get_mut(addr) {
                            *p = mn;
                        }
                        drop(_sc);
                        if self.refs.is_empty() {
                            return Err(MbError::Unsupported("inter without reference"));
                        }
                        let (mut jgmv, mut jgref) = ([(0i32, 0i32); 16], [0u8; 16]);
                        {
                            let w4r = self.mb_w * 4;
                            for by in 0..4usize {
                                let row = (mby * 4 + by) * w4r + mbx * 4;
                                jgmv[by * 4..by * 4 + 4].copy_from_slice(&self.mv_y[row..row + 4]);
                                let ridx = &self.ref_idx_y[row..row + 4];
                                for bx in 0..4usize {
                                    jgref[by * 4 + bx] = ridx[bx].clamp(0, 15) as u8;
                                }
                            }
                        }
                        let nores = cbp == 0 && nores_on();
                        if nores {
                            let pj = PInterNoResJob {
                                mbx,
                                mby,
                                t8,
                                gmv: jgmv,
                                gref: jgref,
                            };
                            if self.edc_tx.is_some() {
                                self.edc_giveback();
                                self.edc_commit_nnz(mbx, mby, t8, &[0u8; 24], 0);

                                let b = self.take_nores_job(pj);
                                self.edc_send_job(EdcJob::InterNoRes(b));
                            } else if self.edc_active {
                                let b = self.take_nores_job(pj);
                                self.edc_jobs.push(EdcJob::InterNoRes(b));
                            } else {
                                self.recon_p_inter_nores(&pj);
                                if double_recon() {
                                    self.recon_p_inter_nores(&pj);
                                }
                            }
                            let eos = cab.decode_terminate();
                            addr += 1;
                            mbx += 1;
                            if eos || addr >= total {
                                break;
                            }
                            continue;
                        }
                        job.mbx = mbx;
                        job.mby = mby;
                        job.qp = self.cur_qp;
                        job.cbp_chroma = cbp_chroma;
                        job.gmv = jgmv;
                        job.gref = jgref;
                        if self.edc_tx.is_some() {
                            self.edc_giveback();
                            self.edc_commit_nnz(mbx, mby, t8, &job.nnzs, cbp_chroma);

                            self.edc_send_job(EdcJob::Inter(job));
                        } else if self.edc_active {
                            self.edc_jobs.push(EdcJob::Inter(job));
                        } else {
                            self.recon_p_inter(&job);
                            if double_recon() {
                                self.recon_p_inter(&job);
                            }
                            self.edc_job_pool.push(job);
                        }
                        let eos = cab.decode_terminate();
                        addr += 1;
                        mbx += 1;
                        if eos || addr >= total {
                            break;
                        }
                        continue;
                    }
                    mb_type = mbt - 5;
                } else if self.is_b {
                    if b_records {
                        self.edc_regions = Some(Vec::with_capacity(8));
                    }
                    let _gb =
                        crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecMbB);
                    let mut allow8 = true;
                    let (hl, ht) = (left.is_some(), top.is_some());
                    let sctx = 24
                        + (hl && !mb_skip.get(addr - 1).copied().unwrap_or(true)) as usize
                        + (ht && !mb_skip.get(addr.wrapping_sub(mbw)).copied().unwrap_or(true))
                            as usize;
                    let (data, mut e, ctx) = cab.view();
                    if e.decode_decision(data, ctx, sctx) != 0 {
                        if let (Some(sk), Some(di)) =
                            (mb_skip.get_mut(addr), mb_direct.get_mut(addr))
                        {
                            (*sk, *di) = (true, true);
                        }
                        last_delta_qp = 0;
                        if !self.b_skip_hot(mbx, mby) {
                            self.decode_b_skip(mbx, mby)?;
                        }
                        if let Some(p) = self.mb_qp.get_mut(addr) {
                            *p = self.cur_qp;
                        }
                        let eos = e.decode_terminate(data);
                        cab.commit(e);
                        addr += 1;
                        mbx += 1;
                        if eos || addr >= total {
                            break;
                        }
                        continue;
                    }
                    self.span_flush();
                    let bci = (hl && !mb_direct.get(addr - 1).copied().unwrap_or(true)) as usize
                        + (ht
                            && !mb_direct
                                .get(addr.wrapping_sub(mbw))
                                .copied()
                                .unwrap_or(true)) as usize;
                    let bmt = {
                        let _s = crate::h264::common::prof::scope(
                            crate::h264::common::prof::Stage::BTypeParse,
                        );
                        mb_type_b_eng(&mut e, data, ctx, 27, bci)
                    };
                    cab.commit(e);
                    if bmt < 23 {
                        let mut mvdc0 = [[0i16; 2]; 30];
                        let mut refc0 = [-1i8; 30];
                        let mut mvdc1 = [[0i16; 2]; 30];
                        let mut refc1 = [-1i8; 30];
                        let tl = (mbx > 0 && mby > 0).then(|| addr - mbw - 1);
                        let tr = (mby > 0 && mbx + 1 < mbw).then(|| addr - mbw + 1);
                        macro_rules! fill {
                            ($ mrf : expr , $ mmv : expr , $ rc : expr , $ mc : expr) => {{
                                if let Some(l) = left {
                                    if let (Some(rr), Some(mm)) = ($mrf.get(l), $mmv.get(l)) {
                                        for (ci, bi) in
                                            [(6usize, 3usize), (12, 7), (18, 11), (24, 15)]
                                        {
                                            $rc[ci] = rr[bi];
                                            $mc[ci] = mm[bi];
                                        }
                                    }
                                }
                                if let Some(t) = top {
                                    if let (Some(rr), Some(mm)) = ($mrf.get(t), $mmv.get(t)) {
                                        for (ci, bi) in
                                            [(1usize, 12usize), (2, 13), (3, 14), (4, 15)]
                                        {
                                            $rc[ci] = rr[bi];
                                            $mc[ci] = mm[bi];
                                        }
                                    }
                                }
                                if let Some(a) = tl {
                                    if let (Some(rr), Some(mm)) = ($mrf.get(a), $mmv.get(a)) {
                                        ($rc[0], $mc[0]) = (rr[15], mm[15]);
                                    }
                                }
                                if let Some(a) = tr {
                                    if let (Some(rr), Some(mm)) = ($mrf.get(a), $mmv.get(a)) {
                                        ($rc[5], $mc[5]) = (rr[12], mm[12]);
                                    }
                                }
                            }};
                        }
                        {
                            let _s = crate::h264::common::prof::scope(
                                crate::h264::common::prof::Stage::BFillCache,
                            );
                            fill!(mb_ref, mb_mvd, refc0, mvdc0);
                            fill!(mb_ref1, mb_mvd1, refc1, mvdc1);
                        }
                        let mut _smv = Some(crate::h264::common::prof::scope(
                            crate::h264::common::prof::Stage::BMvdParse,
                        ));
                        let mut mmvd0 = [[0i16; 2]; 16];
                        let mut mref0 = [-1i8; 16];
                        let mut mmvd1 = [[0i16; 2]; 16];
                        let mut mref1 = [-1i8; 16];
                        if !b_refs_ok {
                            return Err(MbError::Unsupported("B without references"));
                        }
                        let rec_mode = self.edc_regions.is_some();
                        if bmt == 0 {
                            if let Some(p) = mb_direct.get_mut(addr) {
                                *p = true;
                            }
                            allow8 = self.direct_8x8_inference;
                            (mref0, mref1) = ([0i8; 16], [0i8; 16]);
                            let up = addr.wrapping_sub(mbw);
                            let forced = !b_records
                                && mbx > 0
                                && mby > 0
                                && mbx + 1 < mbw
                                && up >= self.slice_first_mb
                                && match self.bzero.get(..=addr) {
                                    Some(bz) => {
                                        bz.get(addr - 1).copied().unwrap_or(false)
                                            && bz.get(up).copied().unwrap_or(false)
                                            && bz.get(up + 1).copied().unwrap_or(false)
                                    }
                                    None => false,
                                };
                            if forced {
                                self.b_direct_region(
                                    mbx,
                                    mby,
                                    0,
                                    0,
                                    16,
                                    16,
                                    &mut pred_y,
                                    &mut c_pred,
                                    (0, 0, (0, 0), (0, 0), false),
                                );
                                if let Some(b) = self.bzero.get_mut(addr) {
                                    *b = true;
                                }
                            } else {
                                self.decode_b_direct(
                                    mbx,
                                    mby,
                                    0,
                                    0,
                                    16,
                                    16,
                                    &mut pred_y,
                                    &mut c_pred,
                                );
                            }
                        } else if bmt == 22 {
                            let mut subt = [0u32; 4];
                            for s in &mut subt {
                                *s = parse_sub_mb_type_b_cabac(&mut cab);
                            }
                            let d8i = self.direct_8x8_inference;
                            allow8 = subt
                                .iter()
                                .all(|&t| if t == 0 { d8i } else { (1..=3).contains(&t) });
                            let su = [
                                [b_sub_uses(subt[0], 0), b_sub_uses(subt[0], 1)],
                                [b_sub_uses(subt[1], 0), b_sub_uses(subt[1], 1)],
                                [b_sub_uses(subt[2], 0), b_sub_uses(subt[2], 1)],
                                [b_sub_uses(subt[3], 0), b_sub_uses(subt[3], 1)],
                            ];
                            for i in 0..4usize {
                                if subt[i] == 0 {
                                    let b = i * 4;
                                    for zb in b..b + 4 {
                                        let (g, c) = (G_SCAN4[zb & 15], CACHE30[zb & 15]);
                                        (mref0[g], mref1[g]) = (0, 0);
                                        (refc0[c], refc1[c]) = (0, 0);
                                    }
                                }
                            }
                            let mut sref = [[0i8; 2]; 4];
                            for list in 0..2usize {
                                if (if list == 0 {
                                    self.num_ref_active
                                } else {
                                    self.num_ref_active1
                                }) <= 1
                                {
                                    continue;
                                }
                                let rc = if list == 0 { &mut refc0 } else { &mut refc1 };
                                for i in 0..4usize {
                                    let st = subt[i];
                                    if st == 0 || !su[i][list] {
                                        continue;
                                    }
                                    let b = i * 4;
                                    let s = CACHE30[b & 15].clamp(6, 29);
                                    let c0 =
                                        (rc[s - 1] > 0) as usize + 2 * (rc[s - 6] > 0) as usize;
                                    let r = parse_ref_idx_cabac(&mut cab, c0);
                                    for &zb in &[b, b + 1, b + 2, b + 3] {
                                        rc[CACHE30[zb]] = r;
                                    }
                                    sref[i][list] = r;
                                }
                            }
                            for list in 0..2usize {
                                let (mmv, mrf, mc, rc) = if list == 0 {
                                    (&mut mmvd0, &mut mref0, &mut mvdc0, &mut refc0)
                                } else {
                                    (&mut mmvd1, &mut mref1, &mut mvdc1, &mut refc1)
                                };
                                for i in 0..4usize {
                                    let st = subt[i];
                                    if st == 0 || !su[i][list] {
                                        continue;
                                    }
                                    let b = i * 4;
                                    for &(sx, sy, sw, sh) in b_sub_parts(st) {
                                        let (w4b, h4b) = (sw / 4, sh / 4);
                                        let base = b + (sy / 4) * 2 + sx / 4;
                                        let step = if w4b == 2 { 1 } else { 2 };
                                        let zb = [base, base + step, base + 2, base + 3];
                                        let n = w4b * h4b;
                                        parse_mvd_partition(
                                            &mut cab,
                                            zb[0],
                                            &zb[..n],
                                            mc,
                                            rc,
                                            mmv,
                                            mrf,
                                            sref[i][list],
                                        );
                                    }
                                }
                            }
                            let hoisted = if self.direct_spatial
                                && direct_memo_on()
                                && subt.iter().any(|&t| t == 0)
                            {
                                let (n0, n1) = self.b_direct_nbrs(mbx, mby);
                                Some(Self::b_direct_refs_mvs(&n0, &n1))
                            } else {
                                None
                            };
                            for (p, &st) in subt.iter().enumerate() {
                                let (b8x, b8y) = ((p % 2) * 8, (p / 2) * 8);
                                if st == 0 {
                                    match hoisted {
                                        Some(derived) => self.b_direct_region(
                                            mbx,
                                            mby,
                                            b8x,
                                            b8y,
                                            8,
                                            8,
                                            &mut pred_y,
                                            &mut c_pred,
                                            derived,
                                        ),
                                        None => self.decode_b_direct(
                                            mbx,
                                            mby,
                                            b8x,
                                            b8y,
                                            8,
                                            8,
                                            &mut pred_y,
                                            &mut c_pred,
                                        ),
                                    }
                                    continue;
                                }
                                let (u0, u1) = (su[p & 3][0], su[p & 3][1]);
                                for &(sx, sy, sw, sh) in b_sub_parts(st) {
                                    let (px, py) = (b8x + sx, b8y + sy);
                                    let mut mv = [(0i32, 0i32); 2];
                                    let (bx4, by4) =
                                        ((mbx * 4 + px / 4) as isize, (mby * 4 + py / 4) as isize);
                                    let didx = (py / 4) * 4 + px / 4;
                                    if u0 && u1 {
                                        let (n0, n1) =
                                            self.mv_neighbors_both(bx4, by4, (sw / 4) as isize);
                                        for (list, nb) in [(0usize, &n0), (1, &n1)] {
                                            let d = (if list == 0 { &mmvd0 } else { &mmvd1 })
                                                [didx & 15];
                                            let pmv = predict_mv(
                                                nb[0],
                                                nb[1],
                                                nb[2],
                                                sref[p & 3][list & 1] as i32,
                                            );
                                            mv[list & 1] =
                                                (pmv.0 + d[0] as i32, pmv.1 + d[1] as i32);
                                        }
                                    } else {
                                        for list in 0..2usize {
                                            if if list == 0 { u0 } else { u1 } {
                                                let d = (if list == 0 { &mmvd0 } else { &mmvd1 })
                                                    [didx & 15];
                                                let nb = self.mv_neighbors_list(
                                                    bx4,
                                                    by4,
                                                    (sw / 4) as isize,
                                                    list,
                                                );
                                                let pmv = predict_mv(
                                                    nb[0],
                                                    nb[1],
                                                    nb[2],
                                                    sref[p & 3][list & 1] as i32,
                                                );
                                                mv[list & 1] =
                                                    (pmv.0 + d[0] as i32, pmv.1 + d[1] as i32);
                                            }
                                        }
                                    }
                                    let refi0 = if u0 { sref[p & 3][0] as i32 } else { -1 };
                                    let refi1 = if u1 { sref[p & 3][1] as i32 } else { -1 };
                                    self.b_set_motion(
                                        mbx, mby, px, py, sw, sh, refi0, mv[0], refi1, mv[1],
                                    );
                                    if rec_mode {
                                        self.b_mc_or_record(
                                            mbx,
                                            mby,
                                            px,
                                            py,
                                            sw,
                                            sh,
                                            refi0,
                                            mv[0],
                                            refi1,
                                            mv[1],
                                            &mut pred_y,
                                            &mut c_pred,
                                        );
                                    } else {
                                        self.b_mc(
                                            mbx,
                                            mby,
                                            px,
                                            py,
                                            sw,
                                            sh,
                                            refi0,
                                            mv[0],
                                            refi1,
                                            mv[1],
                                            &mut pred_y,
                                            &mut c_pred,
                                        );
                                    }
                                }
                            }
                        } else {
                            let (layout, mvmode, preds) = b_inter_layout(bmt);
                            let parts: &[(usize, &[usize])] = match mvmode {
                                0 => {
                                    &[(0, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15])]
                                }
                                1 => &[
                                    (0, &[0, 1, 2, 3, 4, 5, 6, 7]),
                                    (8, &[8, 9, 10, 11, 12, 13, 14, 15]),
                                ],
                                _ => &[
                                    (0, &[0, 1, 2, 3, 8, 9, 10, 11]),
                                    (4, &[4, 5, 6, 7, 12, 13, 14, 15]),
                                ],
                            };
                            let mut pref = [[0i8; 2]; 2];
                            for list in 0..2usize {
                                let active = if list == 0 {
                                    self.num_ref_active
                                } else {
                                    self.num_ref_active1
                                };
                                if active <= 1 {
                                    continue;
                                }
                                let rc = if list == 0 { &mut refc0 } else { &mut refc1 };
                                for (p, &(pidx, zb)) in parts.iter().enumerate() {
                                    if !preds[p & 1].uses(list) {
                                        continue;
                                    }
                                    let s = CACHE30[pidx & 15].clamp(6, 29);
                                    let c0 =
                                        (rc[s - 1] > 0) as usize + 2 * (rc[s - 6] > 0) as usize;
                                    let r = parse_ref_idx_cabac(&mut cab, c0);
                                    for &zbi in zb.iter() {
                                        rc[CACHE30[zbi & 15]] = r;
                                    }
                                    pref[p & 1][list & 1] = r;
                                }
                            }
                            for list in 0..2usize {
                                let (mmv, mrf, mc, rc) = if list == 0 {
                                    (&mut mmvd0, &mut mref0, &mut mvdc0, &mut refc0)
                                } else {
                                    (&mut mmvd1, &mut mref1, &mut mvdc1, &mut refc1)
                                };
                                for (p, &(pidx, zb)) in parts.iter().enumerate() {
                                    if preds[p & 1].uses(list) {
                                        parse_mvd_partition(
                                            &mut cab,
                                            pidx,
                                            zb,
                                            mc,
                                            rc,
                                            mmv,
                                            mrf,
                                            pref[p & 1][list & 1],
                                        );
                                    }
                                }
                            }
                            let (mbb_x, mbb_y) = (mbx * 4, mby * 4);
                            for (p, &(rx, ry, rw, rh)) in layout.iter().enumerate() {
                                let mut mv = [(0i32, 0i32); 2];
                                let (u0, u1) = (preds[p & 1].uses(0), preds[p & 1].uses(1));
                                let (nx, ny) =
                                    ((mbb_x + rx / 4) as isize, (mbb_y + ry / 4) as isize);
                                let (nw, didx) = ((rw / 4) as isize, (ry / 4) * 4 + rx / 4);
                                if u0 && u1 {
                                    let (n0, n1) = self.mv_neighbors_both(nx, ny, nw);
                                    for (list, n) in [(0usize, &n0), (1, &n1)] {
                                        let d =
                                            (if list == 0 { &mmvd0 } else { &mmvd1 })[didx & 15];
                                        let pmv = predict_partition_mv(
                                            mvmode,
                                            p,
                                            n[0],
                                            n[1],
                                            n[2],
                                            pref[p & 1][list & 1] as i32,
                                        );
                                        mv[list & 1] = (pmv.0 + d[0] as i32, pmv.1 + d[1] as i32);
                                    }
                                } else {
                                    for list in 0..2usize {
                                        if if list == 0 { u0 } else { u1 } {
                                            let d = (if list == 0 { &mmvd0 } else { &mmvd1 })
                                                [didx & 15];
                                            let n = self.mv_neighbors_list(nx, ny, nw, list);
                                            let pmv = predict_partition_mv(
                                                mvmode,
                                                p,
                                                n[0],
                                                n[1],
                                                n[2],
                                                pref[p & 1][list & 1] as i32,
                                            );
                                            mv[list & 1] =
                                                (pmv.0 + d[0] as i32, pmv.1 + d[1] as i32);
                                        }
                                    }
                                }
                                let refi0 = if u0 { pref[p & 1][0] as i32 } else { -1 };
                                let refi1 = if u1 { pref[p & 1][1] as i32 } else { -1 };
                                self.b_set_motion(
                                    mbx, mby, rx, ry, rw, rh, refi0, mv[0], refi1, mv[1],
                                );
                                if rec_mode {
                                    self.b_mc_or_record(
                                        mbx,
                                        mby,
                                        rx,
                                        ry,
                                        rw,
                                        rh,
                                        refi0,
                                        mv[0],
                                        refi1,
                                        mv[1],
                                        &mut pred_y,
                                        &mut c_pred,
                                    );
                                } else {
                                    self.b_mc(
                                        mbx,
                                        mby,
                                        rx,
                                        ry,
                                        rw,
                                        rh,
                                        refi0,
                                        mv[0],
                                        refi1,
                                        mv[1],
                                        &mut pred_y,
                                        &mut c_pred,
                                    );
                                }
                            }
                        }
                        if let (Some(r0), Some(d0)) = (mb_ref.get_mut(addr), mb_mvd.get_mut(addr)) {
                            (*r0, *d0) = (mref0, mmvd0);
                        }
                        if let (Some(r1), Some(d1)) = (mb_ref1.get_mut(addr), mb_mvd1.get_mut(addr))
                        {
                            (*r1, *d1) = (mref1, mmvd1);
                        }
                        _smv = None;
                        let _sres = crate::h264::common::prof::scope(
                            crate::h264::common::prof::Stage::BResid,
                        );
                        let cbp = parse_cbp_cabac(
                            &mut cab,
                            top.and_then(|a| mb_cbp.get(a).copied()),
                            left.and_then(|a| mb_cbp.get(a).copied()),
                        );
                        if let Some(p) = mb_cbp.get_mut(addr) {
                            *p = cbp as u8;
                        }
                        let t8 = self.transform_8x8_mode && (cbp & 15) != 0 && allow8 && {
                            let a = left
                                .map_or(0, |x| self.mb_t8x8.get(x).is_some_and(|&f| f) as usize);
                            let b =
                                top.map_or(0, |x| self.mb_t8x8.get(x).is_some_and(|&f| f) as usize);
                            cab.decode_decision(399 + a + b) != 0
                        };
                        if let Some(p) = self.mb_t8x8.get_mut(addr) {
                            *p = t8;
                        }
                        self.any_t8 |= t8;
                        if cbp == 0 && nores_on() {
                            last_delta_qp = 0;
                            if let Some(p) = self.mb_qp.get_mut(addr) {
                                *p = self.cur_qp;
                            }
                            if let Some(p) = cbf_dc.get_mut(addr) {
                                *p = 0;
                            }
                            if let Some(p) = mb_nzc.get_mut(addr) {
                                *p = [0u8; 24];
                            }
                            if let Some(regions) = self.edc_regions.take() {
                                self.edc_giveback();
                                self.edc_commit_nnz(mbx, mby, false, &[0u8; 24], 0);
                                self.edc_send_job(EdcJob::BSkip { mbx, mby, regions });
                            } else {
                                for dy in 0..16 {
                                    let d = (mby * 16 + dy) * self.cw + mbx * 16;
                                    self.rec_y[d..d + 16]
                                        .copy_from_slice(&pred_y[dy * 16..dy * 16 + 16]);
                                }
                                for c in 0..2 {
                                    let plane = if c == 0 {
                                        &mut self.rec_u
                                    } else {
                                        &mut self.rec_v
                                    };
                                    for dy in 0..8 {
                                        let d = (mby * 8 + dy) * self.ccw + mbx * 8;
                                        plane[d..d + 8]
                                            .copy_from_slice(&c_pred[c][dy * 8..dy * 8 + 8]);
                                    }
                                }
                                let w4 = self.mb_w * 4;
                                for dy in 0..4 {
                                    self.nnz_y[(mby * 4 + dy) * w4 + mbx * 4..][..4].fill(0);
                                }
                            }
                            let eos = cab.decode_terminate();
                            addr += 1;
                            mbx += 1;
                            if eos || addr >= total {
                                break;
                            }
                            continue;
                        }
                        let mut luma8 = self
                            .scratch_luma8
                            .take()
                            .unwrap_or_else(|| Box::new([[0i32; 64]; 4]));
                        let (cbp_luma, cbp_chroma) = (cbp & 15, cbp >> 4);
                        let mut nzc = [0xffu8; 48];
                        if let Some(t) = top {
                            let tnz = mb_nzc.get(t).unwrap_or(&ZERO_NZC);
                            nzc[1..5].copy_from_slice(&tnz[12..16]);
                            (nzc[0], nzc[5], nzc[29]) = (0, 0, 0);
                            (nzc[6], nzc[7], nzc[30], nzc[31]) =
                                (tnz[20], tnz[21], tnz[22], tnz[23]);
                        }
                        if let Some(l) = left {
                            let lnz = mb_nzc.get(l).unwrap_or(&ZERO_NZC);
                            (nzc[8], nzc[16], nzc[24], nzc[32]) =
                                (lnz[3], lnz[7], lnz[11], lnz[15]);
                            (nzc[13], nzc[21], nzc[37], nzc[45]) =
                                (lnz[17], lnz[21], lnz[19], lnz[23]);
                        }
                        let mut cbfdc = 0u16;
                        let mut nnzs = [0u8; 24];
                        let mut luma_scan = self
                            .scratch_luma
                            .take()
                            .unwrap_or_else(|| Box::new([[0i32; 16]; 16]));
                        let mut cdc = [[0i32; 4]; 2];
                        let mut cac = self
                            .scratch_cac
                            .take()
                            .unwrap_or_else(|| Box::new([[[0i32; 16]; 4]; 2]));
                        if cbp == 0 {
                            last_delta_qp = 0;
                        }
                        if cbp != 0 {
                            let ndc = (
                                top.and_then(|a| cbf_dc.get(a).copied()),
                                left.and_then(|a| cbf_dc.get(a).copied()),
                            );
                            let qpd = parse_mb_qp_delta_cabac(&mut cab, &mut last_delta_qp);
                            self.step_qp(qpd)?;
                            parse_mb_residual_cabac::<false>(
                                &mut cab,
                                &mut nzc,
                                &mut cbfdc,
                                ndc,
                                cbp_luma,
                                cbp_chroma,
                                t8,
                                ResidualOut {
                                    luma: &mut luma_scan,
                                    luma8: &mut luma8,
                                    cdc: &mut cdc,
                                    cac: &mut cac,
                                    nnzs: &mut nnzs,
                                },
                            );
                        }
                        if let Some(p) = self.mb_qp.get_mut(addr) {
                            *p = self.cur_qp;
                        }
                        if let Some(p) = cbf_dc.get_mut(addr) {
                            *p = cbfdc;
                        }
                        let _sc = crate::h264::common::prof::scope(
                            crate::h264::common::prof::Stage::DecStateCache,
                        );
                        let mn = nnz_raster_from_z(&nnzs);
                        if let Some(p) = mb_nzc.get_mut(addr) {
                            *p = mn;
                        }
                        drop(_sc);
                        if let Some(regions) = self.edc_regions.take() {
                            self.edc_giveback();
                            self.edc_commit_nnz(mbx, mby, t8, &nnzs, cbp_chroma);
                            let job = BJob {
                                mbx,
                                mby,
                                qp: self.cur_qp,
                                cbp_chroma,
                                skip: false,
                                regions,
                                luma_scan: (!t8 && cbp_luma != 0).then(|| *luma_scan),
                                luma8: t8.then(|| *luma8),
                                cdc,
                                cac: (cbp_chroma == 2).then(|| *cac),
                                nnzs,
                            };
                            self.edc_send_job(EdcJob::B(Box::new(job)));
                        } else {
                            self.add_inter_residual(
                                mbx,
                                mby,
                                &pred_y,
                                &c_pred,
                                Some(&*luma_scan),
                                t8.then_some(&*luma8),
                                &cdc,
                                (cbp_chroma == 2).then_some(&*cac),
                                cbp_chroma,
                                &nnzs,
                            );
                        }
                        self.scratch_luma = Some(luma_scan);
                        self.scratch_luma8 = Some(luma8);
                        self.scratch_cac = Some(cac);
                        let eos = cab.decode_terminate();
                        addr += 1;
                        mbx += 1;
                        if eos || addr >= total {
                            break;
                        }
                        continue;
                    }
                    mb_type = bmt - 23;
                } else {
                    let li = left.map_or(0, |a| cat.get(a).is_some_and(|&c| c >= 2) as usize);
                    let ti = top.map_or(0, |a| cat.get(a).is_some_and(|&c| c >= 2) as usize);
                    mb_type = parse_mb_type_i_cabac(&mut cab, li + ti);
                }
                self.edc_intra_sync();
                self.span_flush();
                let _gi =
                    crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecMbI);
                if let Some(p) = self.mb_kind.get_mut(mby * self.mb_w + mbx) {
                    *p = crate::h264::common::deblock::MB_KIND_INTRA;
                }
                if mb_type == 25 {
                    let pcm = cab.pcm_start_byte();
                    let end = pcm + 384;
                    if end > rbsp.len() {
                        return Err(MbError::Truncated);
                    }
                    let mut pr = BitReader::new(&rbsp[pcm..end]);
                    self.decode_ipcm(&mut pr, mbx, mby)?;
                    cab.reinit_at(end);
                    if let Some(p) = self.mb_qp.get_mut(addr) {
                        *p = self.cur_qp;
                    }
                    if let Some(p) = cat.get_mut(addr) {
                        *p = 25;
                    }
                    if let Some(p) = cmode.get_mut(addr) {
                        *p = 0;
                    }
                    if let Some(p) = mb_cbp.get_mut(addr) {
                        *p = 0x2f;
                    }
                    if let Some(p) = cbf_dc.get_mut(addr) {
                        *p = (1 << RP_I16_DC) | (1 << RP_CHROMA_DC) | (1 << (RP_CHROMA_DC + 1));
                    }
                    if let Some(p) = mb_nzc.get_mut(addr) {
                        *p = [16u8; 24];
                    }
                    last_delta_qp = 0;
                    let eos = cab.decode_terminate();
                    addr += 1;
                    mbx += 1;
                    if eos || addr >= total {
                        break;
                    }
                    continue;
                }
                let cci = left.map_or(0, |a| {
                    cmode.get(a).is_some_and(|c| (1..=3).contains(c)) as usize
                }) + top.map_or(0, |a| {
                    cmode.get(a).is_some_and(|c| (1..=3).contains(c)) as usize
                });
                if mb_type != 0 {
                    let mt = mb_type - 1;
                    let pred_mode = I16Mode::from_id(mt % 4);
                    let cbp_chroma = (mt % 12) / 4;
                    let cbp_luma_15 = mt / 12 == 1;
                    let chroma_mode = parse_intra_chroma_pred_mode_cabac(&mut cab, cci) as u8;
                    if let Some(p) = cmode.get_mut(addr) {
                        *p = chroma_mode as i32;
                    }
                    if let Some(p) = cat.get_mut(addr) {
                        *p = 2;
                    }
                    if let Some(p) = mb_cbp.get_mut(addr) {
                        *p = ((cbp_chroma as u8) << 4) | if cbp_luma_15 { 15 } else { 0 };
                    }
                    let w4 = self.mb_w * 4;
                    let mut nzc = [0xffu8; 48];
                    if let Some(t) = top {
                        let tn = mb_nzc.get(t).unwrap_or(&ZERO_NZC);
                        nzc[1..5].copy_from_slice(&tn[12..16]);
                        (nzc[0], nzc[5], nzc[29]) = (0, 0, 0);
                        (nzc[6], nzc[7]) = (tn[20], tn[21]);
                        (nzc[30], nzc[31]) = (tn[22], tn[23]);
                    }
                    if let Some(l) = left {
                        let ln = mb_nzc.get(l).unwrap_or(&ZERO_NZC);
                        (nzc[8], nzc[16], nzc[24], nzc[32]) = (ln[3], ln[7], ln[11], ln[15]);
                        (nzc[13], nzc[21], nzc[37], nzc[45]) = (ln[17], ln[21], ln[19], ln[23]);
                    }
                    let ndc = (
                        top.and_then(|a| cbf_dc.get(a).copied()),
                        left.and_then(|a| cbf_dc.get(a).copied()),
                    );
                    let qpd = parse_mb_qp_delta_cabac(&mut cab, &mut last_delta_qp);
                    self.step_qp(qpd)?;
                    let qp = self.cur_qp;
                    let mut cbfdc = 0u16;
                    let nd = resolve_ndc(ndc, true);
                    let (data, mut e, ctx) = cab.view();
                    let mut dc_scan = [0i32; 16];
                    residual_block_eng::<RP_I16_DC, 16>(
                        &mut e,
                        data,
                        ctx,
                        &mut nzc,
                        &mut cbfdc,
                        0,
                        0,
                        true,
                        nd,
                        &mut dc_scan,
                    );
                    let recon_dc = inverse_quant_luma_dc_scan(
                        &dc_scan,
                        qp,
                        self.scaling.as_ref().map(|s| s[0][0]),
                    );
                    let mut q_blocks: Option<[[i32; 16]; 16]> = None;
                    let mut coded16 = 0u16;
                    for (iz, &(lbx, lby)) in LUMA_4X4_SCAN_XY.iter().enumerate() {
                        let total = if cbp_luma_15 {
                            let mut ac = [0i32; 16];
                            let t = residual_block_eng::<RP_I16_AC, 16>(
                                &mut e, data, ctx, &mut nzc, &mut cbfdc, iz, 0, true, nd, &mut ac,
                            );
                            q_blocks.get_or_insert_with(|| [[0i32; 16]; 16])
                                [(lby & 3) * 4 + (lbx & 3)] = ac;
                            t as u8
                        } else {
                            nzc[NZC_CACHE[iz.min(23)].min(47)] = 0;
                            0
                        };
                        coded16 |= ((total != 0) as u16) << ((lby & 3) * 4 + (lbx & 3));
                        if let Some(p) = self.nnz_y.get_mut((mby * 4 + lby) * w4 + (mbx * 4 + lbx))
                        {
                            *p = total;
                        }
                    }
                    let mut cdc = [[0i32; 4]; 2];
                    let mut cac: Option<[[[i32; 16]; 4]; 2]> = None;
                    let mut cnnz = [0u8; 8];
                    if cbp_chroma >= 1 {
                        for i in 0..2usize {
                            residual_block_eng::<RP_CHROMA_DC, 4>(
                                &mut e,
                                data,
                                ctx,
                                &mut nzc,
                                &mut cbfdc,
                                16 + i * 4,
                                i,
                                true,
                                nd,
                                &mut cdc[i],
                            );
                        }
                    }
                    if cbp_chroma == 2 {
                        let cacm = cac.get_or_insert_with(|| [[[0i32; 16]; 4]; 2]);
                        for i in 0..2usize {
                            for id4 in 0..4usize {
                                cnnz[(i * 4 + id4) & 7] = residual_block_eng::<RP_CHROMA_AC, 16>(
                                    &mut e,
                                    data,
                                    ctx,
                                    &mut nzc,
                                    &mut cbfdc,
                                    16 + i * 4 + id4,
                                    i,
                                    true,
                                    nd,
                                    &mut cacm[i][id4],
                                ) as u8;
                            }
                        }
                    }
                    cab.commit(e);
                    let top_ok = mby > 0
                        && self.nbr_in_slice(mbx, mby - 1)
                        && self.intra_nbr_ok(mbx * 4, mby * 4 - 1);
                    let left_ok = mbx > 0
                        && self.nbr_in_slice(mbx - 1, mby)
                        && self.intra_nbr_ok(mbx * 4 - 1, mby * 4);
                    self.recon_i16_luma(
                        mbx,
                        mby,
                        pred_mode,
                        top_ok,
                        left_ok,
                        q_blocks.as_ref(),
                        coded16,
                        &recon_dc,
                        qp,
                    );
                    self.recon_chroma_cabac(
                        mbx,
                        mby,
                        chroma_mode,
                        &cdc,
                        cac.as_ref(),
                        &cnnz,
                        cbp_chroma,
                        top_ok,
                        left_ok,
                    );
                    if let Some(p) = self.mb_qp.get_mut(addr) {
                        *p = self.cur_qp;
                    }
                    if let Some(p) = cbf_dc.get_mut(addr) {
                        *p = cbfdc;
                    }
                    let mut mn = [0u8; 24];
                    for k in 0..4 {
                        mn[k] = nzc[9 + k];
                        mn[4 + k] = nzc[17 + k];
                        mn[8 + k] = nzc[25 + k];
                        mn[12 + k] = nzc[33 + k];
                    }
                    (mn[16], mn[17], mn[20], mn[21]) = (nzc[14], nzc[15], nzc[22], nzc[23]);
                    (mn[18], mn[19], mn[22], mn[23]) = (nzc[38], nzc[39], nzc[46], nzc[47]);
                    for v in mn.iter_mut() {
                        if *v == 0xff {
                            *v = 0;
                        }
                    }
                    if let Some(p) = mb_nzc.get_mut(addr) {
                        *p = mn;
                    }
                    let eos = cab.decode_terminate();
                    addr += 1;
                    mbx += 1;
                    if eos || addr >= total {
                        break;
                    }
                    continue;
                }
                if let Some(p) = cat.get_mut(addr) {
                    *p = 0;
                }
                let w4 = self.mb_w * 4;
                let t8 = self.transform_8x8_mode && {
                    let a = left.map_or(0, |x| self.mb_t8x8.get(x).is_some_and(|&f| f) as usize);
                    let b = top.map_or(0, |x| self.mb_t8x8.get(x).is_some_and(|&f| f) as usize);
                    cab.decode_decision(399 + a + b) != 0
                };
                if let Some(p) = self.mb_t8x8.get_mut(addr) {
                    *p = t8;
                }
                self.any_t8 |= t8;
                let top_ok = mby > 0
                    && self.nbr_in_slice(mbx, mby - 1)
                    && self.intra_nbr_ok(mbx * 4, mby * 4 - 1);
                let left_ok = mbx > 0
                    && self.nbr_in_slice(mbx - 1, mby)
                    && self.intra_nbr_ok(mbx * 4 - 1, mby * 4);
                let mut modes = [2u8; 16];
                let mut modes8 = [2u8; 4];
                let (data, mut e, ctx) = cab.view();
                if t8 {
                    for b8 in 0..4usize {
                        let (b8x, b8y) = (b8 % 2, b8 / 2);
                        let (bx, by) = (mbx * 4 + b8x * 2, mby * 4 + b8y * 2);
                        let predicted =
                            self.predict_i4_mode_fast(bx, by, b8x * 2, b8y * 2, top_ok, left_ok);
                        let rr = intra4x4_pred_mode_eng(&mut e, data, ctx);
                        let actual = if rr < 0 {
                            predicted
                        } else {
                            let rem = rr as u8;
                            if rem < predicted { rem } else { rem + 1 }
                        };
                        modes8[b8] = actual;
                        for dy in 0..2 {
                            for dx in 0..2 {
                                if let Some(p) = self.modes_y.get_mut((by + dy) * w4 + (bx + dx)) {
                                    *p = actual;
                                }
                                modes[(b8y * 2 + dy) * 4 + (b8x * 2 + dx)] = actual;
                            }
                        }
                    }
                } else {
                    for &(lbx, lby) in &LUMA_4X4_SCAN_XY {
                        let (bx, by) = (mbx * 4 + lbx, mby * 4 + lby);
                        let predicted =
                            self.predict_i4_mode_fast(bx, by, lbx, lby, top_ok, left_ok);
                        let rr = intra4x4_pred_mode_eng(&mut e, data, ctx);
                        let actual = if rr < 0 {
                            predicted
                        } else {
                            let rem = rr as u8;
                            if rem < predicted { rem } else { rem + 1 }
                        };
                        if let Some(m) = self.modes_y.get_mut(by * w4 + bx) {
                            *m = actual;
                        }
                        modes[(lby & 3) * 4 + (lbx & 3)] = actual;
                    }
                }
                cab.commit(e);
                let chroma_mode = parse_intra_chroma_pred_mode_cabac(&mut cab, cci) as u8;
                if let Some(p) = cmode.get_mut(addr) {
                    *p = chroma_mode as i32;
                }
                let cbp = parse_cbp_cabac(
                    &mut cab,
                    top.and_then(|a| mb_cbp.get(a).copied()),
                    left.and_then(|a| mb_cbp.get(a).copied()),
                );
                if let Some(p) = mb_cbp.get_mut(addr) {
                    *p = cbp as u8;
                }
                let (cbp_luma, cbp_chroma) = (cbp & 15, cbp >> 4);
                let mut nzc = [0xffu8; 48];
                if let Some(t) = top {
                    let tn = mb_nzc.get(t).unwrap_or(&ZERO_NZC);
                    nzc[1..5].copy_from_slice(&tn[12..16]);
                    (nzc[0], nzc[5], nzc[29]) = (0, 0, 0);
                    (nzc[6], nzc[7]) = (tn[20], tn[21]);
                    (nzc[30], nzc[31]) = (tn[22], tn[23]);
                }
                if let Some(l) = left {
                    let ln = mb_nzc.get(l).unwrap_or(&ZERO_NZC);
                    (nzc[8], nzc[16], nzc[24], nzc[32]) = (ln[3], ln[7], ln[11], ln[15]);
                    (nzc[13], nzc[21], nzc[37], nzc[45]) = (ln[17], ln[21], ln[19], ln[23]);
                }
                let mut cbfdc = 0u16;
                let mut luma_scan = self
                    .scratch_luma
                    .take()
                    .unwrap_or_else(|| Box::new([[0i32; 16]; 16]));
                let mut luma8 = self
                    .scratch_luma8
                    .take()
                    .unwrap_or_else(|| Box::new([[0i32; 64]; 4]));
                let mut cdc = [[0i32; 4]; 2];
                let mut cac = self
                    .scratch_cac
                    .take()
                    .unwrap_or_else(|| Box::new([[[0i32; 16]; 4]; 2]));
                let mut nnzs = [0u8; 24];
                if cbp == 0 {
                    last_delta_qp = 0;
                }
                if cbp != 0 {
                    let ndc = (
                        top.and_then(|a| cbf_dc.get(a).copied()),
                        left.and_then(|a| cbf_dc.get(a).copied()),
                    );
                    let qpd = parse_mb_qp_delta_cabac(&mut cab, &mut last_delta_qp);
                    self.step_qp(qpd)?;
                    parse_mb_residual_cabac::<true>(
                        &mut cab,
                        &mut nzc,
                        &mut cbfdc,
                        ndc,
                        cbp_luma,
                        cbp_chroma,
                        t8,
                        ResidualOut {
                            luma: &mut luma_scan,
                            luma8: &mut luma8,
                            cdc: &mut cdc,
                            cac: &mut cac,
                            nnzs: &mut nnzs,
                        },
                    );
                    if t8 {
                        for id8 in 0..4usize {
                            let n = nnzs[id8 * 4];
                            let (b8x, b8y) = (id8 % 2, id8 / 2);
                            for sy in 0..2 {
                                for sx in 0..2 {
                                    if let Some(p) = self.nnz_y.get_mut(
                                        (mby * 4 + b8y * 2 + sy) * w4 + (mbx * 4 + b8x * 2 + sx),
                                    ) {
                                        *p = n;
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(p) = self.mb_qp.get_mut(addr) {
                    *p = self.cur_qp;
                }
                if let Some(p) = cbf_dc.get_mut(addr) {
                    *p = cbfdc;
                }
                let mn = nnz_raster_from_z(&nnzs);
                if let Some(p) = mb_nzc.get_mut(addr) {
                    *p = mn;
                }
                let qp = self.cur_qp;
                if t8 {
                    for b8 in 0..4usize {
                        let (b8x, b8y) = (b8 % 2, b8 / 2);
                        let (bx, by) = (mbx * 4 + b8x * 2, mby * 4 + b8y * 2);
                        let coded = cbp_luma & (1 << b8) != 0;
                        let avail_top = b8y > 0 || top_ok;
                        let avail_left = b8x > 0 || left_ok;
                        self.recon_i8_block(
                            bx,
                            by,
                            modes8[b8],
                            avail_top,
                            avail_left,
                            coded.then_some(&luma8[b8]),
                            qp,
                        );
                        for sy in 0..2 {
                            self.coded_y[(by + sy) * w4 + bx..][..2].fill(true);
                        }
                    }
                }
                if !t8 {
                    let scans = &*luma_scan;
                    let lnnz: [u8; 16] = core::array::from_fn(|i| nnzs[i]);
                    let kinds = self.i4_prepare(scans, &lnnz, qp);
                    let dq0 = self.dq_const(qp, 0);
                    for (blk, &(lbx, lby)) in LUMA_4X4_SCAN_XY.iter().enumerate() {
                        let (bx, by) = (mbx * 4 + lbx, mby * 4 + lby);
                        let at = lby > 0 || top_ok;
                        let al = lbx > 0 || left_ok;
                        self.recon_i4_block_res(
                            bx,
                            by,
                            modes[(lby & 3) * 4 + (lbx & 3)],
                            at,
                            al,
                            kinds[blk & 15],
                            scans,
                            &dq0,
                        );
                    }
                    for ry in 0..4usize {
                        let a = (mby * 4 + ry) * w4 + mbx * 4;
                        self.nnz_y[a..a + 4].copy_from_slice(&mn[ry * 4..ry * 4 + 4]);
                        self.coded_y[a..a + 4].fill(true);
                    }
                }
                self.recon_chroma_cabac(
                    mbx,
                    mby,
                    chroma_mode,
                    &cdc,
                    (cbp_chroma == 2).then_some(&*cac),
                    nnzs[16..24].try_into().expect("8 chroma counts"),
                    cbp_chroma,
                    top_ok,
                    left_ok,
                );
                self.scratch_luma = Some(luma_scan);
                self.scratch_luma8 = Some(luma8);
                self.scratch_cac = Some(cac);
                let eos = cab.decode_terminate();
                addr += 1;
                mbx += 1;
                if eos || addr >= total {
                    break;
                }
            }
        }
        if trace {
            eprintln!("# CABAC decoded {} MBs (of {total})", addr - first_mb);
        }
        self.edc_flush();
        self.sc_cat = cat;
        self.sc_cbp = mb_cbp;
        self.sc_cmode = cmode;
        self.sc_nzc = mb_nzc;
        self.sc_cbfdc = cbf_dc;
        self.sc_skip = mb_skip;
        self.sc_ref = mb_ref;
        self.sc_mvd = mb_mvd;
        if want_b_grids {
            self.sc_ref1 = mb_ref1;
            self.sc_mvd1 = mb_mvd1;
            self.sc_direct = mb_direct;
        }
        Ok(addr)
    }
    fn add_inter_residual(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        pred_y: &[u8; 256],
        c_pred: &[[u8; 64]; 2],
        luma_scan: Option<&[[i32; 16]; 16]>,
        luma8: Option<&[[i32; 64]; 4]>,
        cdc: &[[i32; 4]; 2],
        cac: Option<&[[[i32; 16]; 4]; 2]>,
        cbp_chroma: u32,
        nnzs: &[u8; 24],
    ) {
        let luma_scan = luma_scan.unwrap_or(&ZERO_LUMA_SCAN);
        let cac = cac.unwrap_or(&ZERO_CAC);
        let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecResidAdd);
        let qp = self.cur_qp;
        let qpc = self.chroma_qp_for(qp);
        let (w4r, w2r) = (self.mb_w * 4, self.mb_w * 2);
        if let Some(l8) = luma8 {
            for b8 in 0..4usize {
                let (b8x, b8y) = (b8 % 2, b8 / 2);
                let nnz: u32 = (0..4).map(|k| nnzs[b8 * 4 + k] as u32).sum();
                for sy in 0..2 {
                    let base = (mb_y * 4 + b8y * 2 + sy) * w4r + mb_x * 4 + b8x * 2;
                    self.nnz_y[base..][..2].copy_from_slice(&nnzs[b8 * 4 + sy * 2..][..2]);
                }
                for sy in 0..2 {
                    let base = (mb_y * 4 + b8y * 2 + sy) * w4r + mb_x * 4 + b8x * 2;
                    self.coded_y[base..][..2].fill(true);
                }
                let (px, py) = (mb_x * 16 + b8x * 8, mb_y * 16 + b8y * 8);
                if nnz == 0 {
                    for dy in 0..8 {
                        let d = (py + dy) * self.cw + px;
                        let po = (b8y * 8 + dy) * 16 + b8x * 8;
                        self.rec_y[d..d + 8].copy_from_slice(&pred_y[po..po + 8]);
                    }
                } else {
                    let raster = un_scan_8x8(&l8[b8]);
                    let res8 = self.inv_quant8(&raster, qp, 1);
                    let predb: [i32; 64] = core::array::from_fn(|i| {
                        pred_y[(b8y * 8 + i / 8) * 16 + (b8x * 8 + i % 8)] as i32
                    });
                    let recon = add_residual_8x8(&res8, &predb);
                    for dy in 0..8 {
                        let d = (py + dy) * self.cw + px;
                        self.rec_y[d..][..8].copy_from_slice(&recon[dy * 8..][..8]);
                    }
                }
            }
        }
        let dq3 = self.dq_const(qp, 3);
        for (blk, &(lbx, lby)) in LUMA_4X4_SCAN_XY.iter().enumerate() {
            if luma8.is_some() {
                break;
            }
            let nnz = nnzs[blk];
            if let Some(p) = self
                .nnz_y
                .get_mut((mb_y * 4 + lby) * w4r + (mb_x * 4 + lbx))
            {
                *p = nnz;
            }
            let cw = self.cw;
            let p_off = (lby * 4) * 16 + lbx * 4;
            let r_off = (mb_y * 4 + lby) * 4 * cw + (mb_x * 4 + lbx) * 4;
            if nnz == 0 {
                for r in 0..4 {
                    self.rec_y[r_off + r * cw..r_off + r * cw + 4]
                        .copy_from_slice(&pred_y[p_off + r * 16..p_off + r * 16 + 4]);
                }
                continue;
            }
            if nnz == 1 && luma_scan[blk][0] != 0 {
                let f = self.dequant_dc4(luma_scan[blk][0], qp, 3);
                reconstruct_4x4_dc_into(
                    (f + 32) >> 6,
                    pred_y,
                    p_off,
                    16,
                    &mut self.rec_y,
                    r_off,
                    cw,
                );
            } else {
                reconstruct_4x4_scan_into::<false>(
                    &luma_scan[blk],
                    &dq3,
                    0,
                    pred_y,
                    p_off,
                    16,
                    &mut self.rec_y,
                    r_off,
                    cw,
                );
            }
        }
        let mut c_dc = [[0i32; 4]; 2];
        if cbp_chroma != 0 {
            for c in 0..2 {
                c_dc[c] = self.dequant_chroma_dc(&cdc[c], qpc, 4 + c);
            }
        }
        let dqc = [self.dq_const(qpc, 4), self.dq_const(qpc, 5)];
        let ccw = self.ccw;
        for c in 0..2 {
            for &(bx, by) in &CHROMA_4X4_SCAN_XY {
                let mut ac_nz = false;
                if cbp_chroma == 2 {
                    let n = nnzs[(16 + c * 4 + by * 2 + bx).min(23)];
                    if let Some(p) =
                        self.nnz_c[c & 1].get_mut((mb_y * 2 + by) * w2r + (mb_x * 2 + bx))
                    {
                        *p = n;
                    }
                    ac_nz = n != 0;
                }
                let dc = c_dc[c & 1][(by * 2 + bx) & 3];
                let p_off = (by * 4) * 8 + bx * 4;
                let r_off = (mb_y * 2 + by) * 4 * ccw + (mb_x * 2 + bx) * 4;
                let plane = if c == 0 {
                    &mut self.rec_u
                } else {
                    &mut self.rec_v
                };
                if dc == 0 && !ac_nz {
                    for r in 0..4 {
                        plane[r_off + r * ccw..r_off + r * ccw + 4]
                            .copy_from_slice(&c_pred[c][p_off + r * 8..p_off + r * 8 + 4]);
                    }
                    continue;
                }
                if !ac_nz {
                    reconstruct_4x4_dc_into(
                        (dc + 32) >> 6,
                        &c_pred[c],
                        p_off,
                        8,
                        plane,
                        r_off,
                        ccw,
                    );
                    continue;
                }
                reconstruct_4x4_scan_into::<true>(
                    &cac[c & 1][(by * 2 + bx) & 3],
                    &dqc[c & 1],
                    dc,
                    &c_pred[c],
                    p_off,
                    8,
                    plane,
                    r_off,
                    ccw,
                );
            }
        }
    }
    fn i4_prepare(&self, scans: &[[i32; 16]; 16], nnzs: &[u8; 16], qp: u8) -> [I4Res; 16] {
        let mut kinds = [I4Res::Zero; 16];
        for blk in 0..16 {
            let (nnz, scan) = (nnzs[blk], &scans[blk]);
            kinds[blk] = if nnz == 0 {
                I4Res::Zero
            } else if nnz == 1 && scan[0] != 0 {
                let f = self.dequant_dc4(scan[0], qp, 0);
                I4Res::Flat((f + 32) >> 6)
            } else {
                I4Res::Idx(blk as u8)
            };
        }
        kinds
    }
    fn recon_i4_block_res(
        &mut self,
        bx: usize,
        by: usize,
        mode: u8,
        at: bool,
        al: bool,
        kind: I4Res,
        scans: &[[i32; 16]; 16],
        dq: &DequantQp,
    ) {
        let (px, py) = (bx * 4, by * 4);
        let (t, l, corner) = self.gather_i4(px, py, at, al, bx, by);
        let pred = intra4x4_pred(mode, at, al, &t, &l, corner);
        let r_off = py * self.cw + px;
        let cw = self.cw;
        match kind {
            I4Res::Zero => {
                let win = &mut self.rec_y[r_off..r_off + 3 * cw + 4];
                for r in 0..4 {
                    win[r * cw..r * cw + 4].copy_from_slice(&pred[r * 4..r * 4 + 4]);
                }
            }
            I4Res::Flat(v) => reconstruct_4x4_dc_into(v, &pred, 0, 4, &mut self.rec_y, r_off, cw),
            I4Res::Idx(i) => reconstruct_4x4_scan_into::<false>(
                &scans[i as usize & 15],
                dq,
                0,
                &pred,
                0,
                4,
                &mut self.rec_y,
                r_off,
                cw,
            ),
        }
    }
    fn recon_i8_block(
        &mut self,
        bx: usize,
        by: usize,
        mode: u8,
        avail_top: bool,
        avail_left: bool,
        coeffs: Option<&[i32; 64]>,
        qp: u8,
    ) {
        let (px, py) = (bx * 4, by * 4);
        let (t, l, corner, avail_corner) = self.gather_i8(px, py, avail_top, avail_left, bx, by);
        let pred = intra8x8_pred(mode, avail_top, avail_left, avail_corner, &t, &l, corner);
        match coeffs {
            None => {
                let (cw, base) = (self.cw, py * self.cw + px);
                let win = &mut self.rec_y[base..base + 7 * cw + 8];
                for dy in 0..8 {
                    win[dy * cw..dy * cw + 8].copy_from_slice(&pred[dy * 8..dy * 8 + 8]);
                }
            }
            Some(scan8) => {
                let raster = un_scan_8x8(scan8);
                let res8 = self.inv_quant8(&raster, qp, 0);
                let predb: [i32; 64] = core::array::from_fn(|i| pred[i] as i32);
                let recon = add_residual_8x8(&res8, &predb);
                let (cw, base) = (self.cw, py * self.cw + px);
                let win = &mut self.rec_y[base..base + 7 * cw + 8];
                for dy in 0..8 {
                    win[dy * cw..dy * cw + 8].copy_from_slice(&recon[dy * 8..dy * 8 + 8]);
                }
            }
        }
    }
    fn recon_i16_luma(
        &mut self,
        mbx: usize,
        mby: usize,
        pred_mode: crate::h264::common::predict::I16Mode,
        top_ok: bool,
        left_ok: bool,
        q_blocks: Option<&[[i32; 16]; 16]>,
        coded: u16,
        recon_dc: &[i32; 16],
        qp: u8,
    ) {
        let q_blocks = q_blocks.unwrap_or(&ZERO_LUMA_SCAN);
        let w4 = self.mb_w * 4;
        let (lx, ly) = (mbx * 16, mby * 16);
        let mut t16 = [0u8; 16];
        let mut l16 = [0u8; 16];
        if top_ok {
            t16.copy_from_slice(self.top_y_row(ly, lx, 16));
        }
        if left_ok {
            let (cw, base) = (self.cw, ly * self.cw + lx - 1);
            for (s, &v) in l16.iter_mut().zip(self.rec_y[base..].iter().step_by(cw)) {
                *s = v;
            }
        }
        let corner = if top_ok && left_ok {
            self.top_y_px(ly, lx - 1)
        } else {
            0
        };
        let pred_l = luma16x16_pred(pred_mode, top_ok, left_ok, &t16, &l16, corner);
        let dq0 = self.dq_const(qp, 0);
        for by in 0..4 {
            for bx in 0..4 {
                let p_off = (by * 4) * 16 + bx * 4;
                let r_off = (ly + by * 4) * self.cw + lx + bx * 4;
                if coded & (1u16 << ((by & 3) * 4 + (bx & 3))) == 0 {
                    reconstruct_4x4_dc_into(
                        (recon_dc[by * 4 + bx] + 32) >> 6,
                        &pred_l,
                        p_off,
                        16,
                        &mut self.rec_y,
                        r_off,
                        self.cw,
                    );
                } else {
                    reconstruct_4x4_scan_into::<true>(
                        &q_blocks[(by & 3) * 4 + (bx & 3)],
                        &dq0,
                        recon_dc[by * 4 + bx],
                        &pred_l,
                        p_off,
                        16,
                        &mut self.rec_y,
                        r_off,
                        self.cw,
                    );
                }
            }
        }
        for by in 0..4 {
            let r = (mby * 4 + by) * w4 + mbx * 4;
            self.modes_y[r..r + 4].fill(2);
            self.coded_y[r..r + 4].fill(true);
        }
    }
    fn recon_chroma_blocks(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        chroma_mode: u8,
        avail_top: bool,
        avail_left: bool,
        qac: &[[[i32; 16]; 4]; 2],
        coded: [u8; 2],
        dc: &[[i32; 4]; 2],
        qpc: u8,
    ) {
        let (cx, cy) = (mb_x * 8, mb_y * 8);
        let dqc = [self.dq_const(qpc, 1), self.dq_const(qpc, 2)];
        for c in 0..2 {
            let mut ctop = [0u8; 8];
            let mut cleft = [0u8; 8];
            let mut ccorner = 0u8;
            {
                let rec_c = if c == 0 { &self.rec_u } else { &self.rec_v };
                if avail_top {
                    ctop.copy_from_slice(self.top_c_row(c, cy, cx, 8));
                }
                if avail_left {
                    let cbase = cy * self.ccw + cx - 1;
                    for (o, &v) in cleft
                        .iter_mut()
                        .zip(rec_c[cbase..].iter().step_by(self.ccw))
                    {
                        *o = v;
                    }
                }
                if avail_top && avail_left {
                    ccorner = self.top_c_px(c, cy, cx - 1);
                }
            }
            let pred8 = chroma8x8_pred(chroma_mode, avail_top, avail_left, &ctop, &cleft, ccorner);
            for &(bx, by) in &CHROMA_4X4_SCAN_XY {
                let p_off = (by * 4) * 8 + bx * 4;
                let ccw = self.ccw;
                let r_off = (cy + by * 4) * ccw + cx + bx * 4;
                if coded[c & 1] & (1u8 << ((by * 2 + bx) & 3)) == 0 {
                    let plane = if c == 0 {
                        &mut self.rec_u
                    } else {
                        &mut self.rec_v
                    };
                    reconstruct_4x4_dc_into(
                        (dc[c & 1][(by * 2 + bx) & 3] + 32) >> 6,
                        &pred8,
                        p_off,
                        8,
                        plane,
                        r_off,
                        ccw,
                    );
                } else {
                    let plane = if c == 0 {
                        &mut self.rec_u
                    } else {
                        &mut self.rec_v
                    };
                    reconstruct_4x4_scan_into::<true>(
                        &qac[c & 1][(by * 2 + bx) & 3],
                        &dqc[c & 1],
                        dc[c & 1][(by * 2 + bx) & 3],
                        &pred8,
                        p_off,
                        8,
                        plane,
                        r_off,
                        ccw,
                    );
                }
            }
        }
    }
    fn recon_chroma_cabac(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        chroma_mode: u8,
        cdc: &[[i32; 4]; 2],
        cac: Option<&[[[i32; 16]; 4]; 2]>,
        cnnz: &[u8; 8],
        cbp_chroma: u32,
        avail_top: bool,
        avail_left: bool,
    ) {
        let cac = cac.unwrap_or(&ZERO_CAC);
        let qpc = self.chroma_qp_for(self.cur_qp);
        let mut c_dc = [[0i32; 4]; 2];
        if cbp_chroma != 0 {
            for c in 0..2 {
                c_dc[c] = self.dequant_chroma_dc(&cdc[c], qpc, 1 + c);
            }
        }
        let w2 = self.mb_w * 2;
        let mut ccoded = [0u8; 2];
        if cbp_chroma == 2 {
            for c in 0..2 {
                for &(bx, by) in &CHROMA_4X4_SCAN_XY {
                    let cnt = cnnz[(c * 4 + by * 2 + bx) & 7];
                    ccoded[c & 1] |= ((cnt != 0) as u8) << ((by * 2 + bx) & 3);
                    if let Some(p) =
                        self.nnz_c[c & 1].get_mut((mb_y * 2 + by) * w2 + (mb_x * 2 + bx))
                    {
                        *p = cnt;
                    }
                }
            }
        }
        self.recon_chroma_blocks(
            mb_x,
            mb_y,
            chroma_mode,
            avail_top,
            avail_left,
            cac,
            ccoded,
            &c_dc,
            qpc,
        );
    }
    pub fn decode_slice_data(
        &mut self,
        r: &mut BitReader,
        is_p: bool,
        first_mb: usize,
    ) -> Result<usize, MbError> {
        self.span_flush();
        let eligible = edc_on() && rowdb_on() && (is_p || self.is_b);
        let threaded = eligible && edc_spawn_worker(self.mb_w, self.mb_h, self.bits_per_mb, false);
        if !threaded {
            return self.decode_slice_cavlc_inner(r, is_p, first_mb);
        }
        {
            let ctx = self.edc_take_ctx();
            let (tx, rx) = crate::h264::decoder::sync::mpsc::sync_channel::<EdcMsg>(edc_bound());
            let (ctx_tx, ctx_rx) = crate::h264::decoder::sync::mpsc::channel::<PixelCtx>();
            let (back_tx, back_rx) = crate::h264::decoder::sync::mpsc::channel::<PixelCtx>();
            let (res, ctx, panicked) = std::thread::scope(|sc| {
                let h = sc.spawn(move || edc_worker(ctx, rx, ctx_tx, back_rx));
                self.edc_tx = Some(tx);
                self.edc_ctx_rx = Some(ctx_rx);
                self.edc_back_tx = Some(back_tx);
                let r2 = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.decode_slice_cavlc_inner(r, is_p, first_mb)
                }));
                self.edc_flush_batch();
                self.edc_giveback();
                self.edc_tx = None;
                self.edc_ctx_rx = None;
                self.edc_back_tx = None;
                match (r2, h.join()) {
                    (Ok(res), Ok(ctx)) => (res, Some(ctx), None),
                    (Err(pn), Ok(ctx)) => (Err(MbError::Truncated), Some(ctx), Some(pn)),
                    (Ok(_), Err(pn)) | (Err(_), Err(pn)) => {
                        (Err(MbError::Truncated), None, Some(pn))
                    }
                }
            });
            if let Some(ctx) = ctx {
                self.edc_restore_ctx(ctx);
            }
            if let Some(pn) = panicked {
                std::panic::resume_unwind(pn);
            }
            return res;
        }
    }
    fn decode_slice_cavlc_inner(
        &mut self,
        r: &mut BitReader,
        is_p: bool,
        first_mb: usize,
    ) -> Result<usize, MbError> {
        let total = self.mb_w * self.mb_h;
        self.slice_first_mb = first_mb;
        self.slice_bounds.push((first_mb, self.cur_idc2));
        self.any_idc2 |= self.cur_idc2;
        self.edc_active = edc_on();
        let mut addr = first_mb;
        let mbw_nz = self.mb_w.max(1);
        let (mut mbx, mut mby) = (addr % mbw_nz, addr / mbw_nz);
        let mbw_c = self.mb_w;
        while addr < total {
            if mbx == mbw_c {
                mbx = 0;
                mby += 1;
            }
            self.row_hook_at(addr, mby);
            if is_p || self.is_b {
                let skip_run = {
                    let _g =
                        crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Syntax);
                    r.read_ue()?
                } as usize;
                if skip_run > total - addr {
                    return Err(MbError::Truncated);
                }
                let mut remaining = skip_run;
                while remaining > 0 {
                    debug_assert!(addr < total);
                    if mbx == mbw_c {
                        mbx = 0;
                        mby += 1;
                    }
                    if self.is_b {
                        if !self.b_skip_hot(mbx, mby) {
                            self.decode_b_skip(mbx, mby)?;
                        }
                        if let Some(p) = self.mb_qp.get_mut(addr) {
                            *p = self.cur_qp;
                        }
                        addr += 1;
                        mbx += 1;
                        remaining -= 1;
                        continue;
                    }
                    self.decode_p_skip(mbx, mby)?;
                    if let Some(p) = self.mb_qp.get_mut(addr) {
                        *p = self.cur_qp;
                    }
                    addr += 1;
                    mbx += 1;
                    remaining -= 1;
                    while remaining > 0
                        && self.skip_zero_next == addr
                        && !no_runmv()
                        && self.edc_tx.is_none()
                    {
                        if mbx == mbw_c {
                            mbx = 0;
                            mby += 1;
                        }
                        let seg = remaining.min(mbw_c - mbx);
                        let recon =
                            self.edc_tx.is_none() && (self.weights.is_none() || self.weights_id0);
                        for k in 0..seg {
                            self.pz_push(mbx + k, mby, recon);
                        }
                        self.route_skip_mbs += seg as u32;
                        self.mb_qp[addr..addr + seg].fill(self.cur_qp);
                        if !recon {
                            for k in 0..seg {
                                self.edc_jobs.push(EdcJob::Skip {
                                    mbx: mbx + k,
                                    mby,
                                    mv: (0, 0),
                                });
                            }
                        }
                        addr += seg;
                        mbx += seg;
                        remaining -= seg;
                        self.skip_zero_next = addr;
                    }
                }
                if addr >= total {
                    break;
                }
                if skip_run > 0 && !r.more_rbsp_data() {
                    break;
                }
            }
            if mbx == mbw_c {
                mbx = 0;
                mby += 1;
            }
            if self.is_b {
                self.edc_intra_sync();
                self.decode_b_mb(r, mbx, mby)?;
            } else {
                self.span_flush();
                self.decode_mb(r, mbx, mby, is_p)?;
            }
            if let Some(p) = self.mb_qp.get_mut(addr) {
                *p = self.cur_qp;
            }
            addr += 1;
            mbx += 1;
            if !r.more_rbsp_data() {
                break;
            }
        }
        self.edc_flush();
        Ok(addr)
    }
    fn decode_mb(
        &mut self,
        r: &mut BitReader,
        mb_x: usize,
        mb_y: usize,
        is_p: bool,
    ) -> Result<(), MbError> {
        self.wait_refs_for_mb(mb_y);
        let mut mb_type = {
            let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Syntax);
            r.read_ue()?
        };
        if is_p {
            if mb_type <= 2 {
                return self.decode_inter(r, mb_x, mb_y, mb_type as u8);
            }
            if mb_type == 3 || mb_type == 4 {
                return self.decode_p8x8(r, mb_x, mb_y, mb_type == 4);
            }
            mb_type -= 5;
        }
        self.edc_intra_sync();
        self.decode_intra_mb(r, mb_x, mb_y, mb_type)
    }
    fn decode_intra_mb(
        &mut self,
        r: &mut BitReader,
        mb_x: usize,
        mb_y: usize,
        mb_type: u32,
    ) -> Result<(), MbError> {
        let _gi = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecMbI);
        if let Some(p) = self.mb_kind.get_mut(mb_y * self.mb_w + mb_x) {
            *p = crate::h264::common::deblock::MB_KIND_INTRA;
        }
        if mb_type == 0 {
            if self.transform_8x8_mode && r.read_bit()? {
                self.decode_i8x8(r, mb_x, mb_y)?;
            } else {
                self.decode_i4x4(r, mb_x, mb_y)?;
            }
        } else if (1..=24).contains(&mb_type) {
            self.decode_i16(r, mb_x, mb_y, mb_type - 1)?;
        } else if mb_type == 25 {
            self.edc_intra_sync();
            self.decode_ipcm(r, mb_x, mb_y)?;
        } else {
            return Err(MbError::Unsupported(
                "only I_4x4 / I_16x16 / I_PCM macroblocks",
            ));
        }
        let w4 = self.mb_w * 4;
        for lby in 0..4usize {
            let a = (mb_y * 4 + lby) * w4 + mb_x * 4;
            self.coded_y[a..a + 4].fill(true);
        }
        Ok(())
    }
    fn decode_inter(
        &mut self,
        r: &mut BitReader,
        mb_x: usize,
        mb_y: usize,
        mode: u8,
    ) -> Result<(), MbError> {
        if self.refs.is_empty() {
            return Err(MbError::Unsupported("inter without reference"));
        }
        if mode == 0 {
            if let Some(k) = self.mb_kind.get_mut(mb_y * self.mb_w + mb_x) {
                *k = crate::h264::common::deblock::MB_KIND_INTER_UNIFORM;
            }
        }
        let w4 = self.mb_w * 4;
        let (ch, cch) = (self.mb_h * 16, self.mb_h * 8);
        let num_refs = self.refs.len();
        let layout = inter_partitions(mode);
        let nparts = layout.len();
        let mut ref_idxs = [0i32; 4];
        if self.num_ref_active > 1 {
            for ri in ref_idxs[..nparts].iter_mut() {
                *ri = read_ref_idx(r, self.num_ref_active)?;
                if *ri as usize >= num_refs {
                    return Err(MbError::Truncated);
                }
            }
        }
        let mut part_mv = [(0i32, (0i32, 0i32)); 4];
        {
            let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::MvGrid);
            for (part, &(rx, ry, rw, rh)) in layout.iter().enumerate() {
                let refi = ref_idxs[part];
                let (pbx, pby) = ((mb_x * 4 + rx / 4) as isize, (mb_y * 4 + ry / 4) as isize);
                let [a, b, c] = self.mv_neighbors_block(pbx, pby, (rw / 4) as isize);
                let pmv = predict_partition_mv(mode, part, a, b, c, refi);
                let mvd_x = read_mvd(r)?;
                let mvd_y = read_mvd(r)?;
                let mv = (pmv.0 + mvd_x, pmv.1 + mvd_y);
                part_mv[part] = (refi, mv);
                for by in ry / 4..ry / 4 + rh / 4 {
                    for bx in rx / 4..rx / 4 + rw / 4 {
                        let idx = (mb_y * 4 + by) * w4 + (mb_x * 4 + bx);
                        if let (Some(m), Some(it), Some(rf), Some(cd)) = (
                            self.mv_y.get_mut(idx),
                            self.inter_y.get_mut(idx),
                            self.ref_idx_y.get_mut(idx),
                            self.coded_y.get_mut(idx),
                        ) {
                            (*m, *it, *rf, *cd) = (mv, true, refi, true);
                        }
                    }
                }
            }
        }
        let mut pred_y = [0u8; 256];
        let mut c_pred = [[0u8; 64]; 2];
        let defer = self.edc_tx.is_some() || self.edc_active;
        let mc_passes = if defer {
            0
        } else if double_recon() {
            2
        } else {
            1
        };
        for _pass in 0..mc_passes {
            if _pass > 0 {}
            for (part, &(rx, ry, rw, rh)) in layout.iter().enumerate() {
                let (refi, mv) = part_mv[part & 3];
                let Some(reference) = self.refs.get(refi as usize) else {
                    continue;
                };
                let mut tmp = [0u8; 256];
                mc_luma_padded(
                    &*reference.luma_guard(reference.ch),
                    reference.lstride(),
                    crate::h264::decoder::LPAD,
                    self.cw,
                    ch,
                    mb_x * 16 + rx,
                    mb_y * 16 + ry,
                    rw,
                    rh,
                    mv.0,
                    mv.1,
                    &mut tmp,
                );
                {
                    let _g =
                        crate::h264::common::prof::scope(crate::h264::common::prof::Stage::PredBuf);
                    restride(&mut pred_y, 16, rx, ry, &tmp, rw, rh);
                }
                let (crx, cry, crw, crh) = (rx / 2, ry / 2, rw / 2, rh / 2);
                for cc in 0..2 {
                    let rc = if cc == 0 {
                        &*reference.chroma_guard(0, reference.ch)
                    } else {
                        &*reference.chroma_guard(1, reference.ch)
                    };
                    let mut tc = [0u8; 64];
                    mc_chroma_padded(
                        rc,
                        reference.cstride(),
                        crate::h264::decoder::CPAD,
                        self.ccw,
                        cch,
                        mb_x * 8 + crx,
                        mb_y * 8 + cry,
                        crw,
                        crh,
                        mv.0,
                        mv.1,
                        &mut tc,
                    );
                    {
                        let _g = crate::h264::common::prof::scope(
                            crate::h264::common::prof::Stage::PredBuf,
                        );
                        restride(&mut c_pred[cc], 8, crx, cry, &tc, crw, crh);
                    }
                }
                self.weight_partition(&mut pred_y, &mut c_pred, 0, refi as usize, rx, ry, rw, rh);
            }
        }
        self.inter_finish(r, mb_x, mb_y, &pred_y, &c_pred, true, defer)
    }
    fn inter_finish(
        &mut self,
        r: &mut BitReader,
        mb_x: usize,
        mb_y: usize,
        pred_y: &[u8; 256],
        c_pred: &[[u8; 64]; 2],
        allow_8x8: bool,
        defer: bool,
    ) -> Result<(), MbError> {
        let w4 = self.mb_w * 4;
        let cbp = {
            let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Syntax);
            read_cbp_inter(r)?
        };
        let cbp_luma = cbp & 15;
        let cbp_chroma = cbp >> 4;
        let t8x8 = cbp_luma > 0 && self.transform_8x8_mode && allow_8x8 && r.read_bit()?;
        if t8x8 {
            if let Some(f) = self.mb_t8x8.get_mut(mb_y * self.mb_w + mb_x) {
                *f = true;
            }
            self.any_t8 = true;
        }
        if cbp != 0 {
            self.step_qp(r.read_se()?)?;
        }
        let (qp, _qpc) = (self.cur_qp, self.chroma_qp_for(self.cur_qp));
        self.nnz_cache_load(mb_x, mb_y);
        let mut nnzs = [0u8; 24];
        let nores = cbp == 0 && nores_on();
        let mut job = (defer && !nores).then(|| self.take_pinter_job());
        let mut s_luma = if job.is_none() {
            self.scratch_luma
                .take()
                .or_else(|| Some(Box::new([[0i32; 16]; 16])))
        } else {
            None
        };
        let mut s_luma8 = if job.is_none() {
            self.scratch_luma8
                .take()
                .or_else(|| Some(Box::new([[0i32; 64]; 4])))
        } else {
            None
        };
        let mut s_cac = if job.is_none() {
            self.scratch_cac
                .take()
                .or_else(|| Some(Box::new([[[0i32; 16]; 4]; 2])))
        } else {
            None
        };
        let (luma_scan, luma8, cac): (
            &mut [[i32; 16]; 16],
            &mut [[i32; 64]; 4],
            &mut [[[i32; 16]; 4]; 2],
        ) = match job.as_deref_mut() {
            Some(j) => (&mut j.luma_scan, &mut j.luma8, &mut j.cac),
            None => (
                s_luma.as_deref_mut().expect("scratch"),
                s_luma8.as_deref_mut().expect("scratch"),
                s_cac.as_deref_mut().expect("scratch"),
            ),
        };
        let mut nnz_raster = [0u8; 16];
        if t8x8 {
            for b8 in 0..4 {
                let (b8x, b8y) = (b8 % 2, b8 / 2);
                let (bx, by) = (mb_x * 4 + b8x * 2, mb_y * 4 + b8y * 2);
                let _ = (bx, by);
                if cbp_luma & (1 << b8) != 0 {
                    luma8[b8 & 3] = [0i32; 64];
                    for sub in 0..4 {
                        let (sx, sy) = (sub % 2, sub / 2);
                        let (cx, cy) = (b8x * 2 + sx, b8y * 2 + sy);
                        let nc = self.nc_pred(cx, cy);
                        let mut blk = [0i32; 16];
                        let total = decode_residual_block_into::<16, 16>(r, nc, &mut blk)?;
                        self.nnz_cache_set(cx, cy, total);
                        nnz_raster[cy * 4 + cx] = total;
                        nnzs[(b8 * 4 + sub) & 15] = total;
                        for k in 0..16 {
                            luma8[b8 & 3][(4 * k + sub) & 63] = blk[k];
                        }
                    }
                } else {
                    for sub in 0..4 {
                        let (sx, sy) = (sub % 2, sub / 2);
                        self.nnz_cache_set(b8x * 2 + sx, b8y * 2 + sy, 0);
                    }
                }
            }
        } else {
            for (blk, &(lbx, lby)) in LUMA_4X4_SCAN_XY.iter().enumerate() {
                let (bx, by) = (mb_x * 4 + lbx, mb_y * 4 + lby);
                let total = if cbp_luma & (1 << (blk / 4)) != 0 {
                    let nc = self.nc_pred(lbx, lby);
                    luma_scan[blk & 15] = [0i32; 16];
                    decode_residual_block_into::<16, 16>(r, nc, &mut luma_scan[blk & 15])?
                } else {
                    0
                };
                self.nnz_cache_set(lbx, lby, total);
                let _ = (bx, by);
                nnz_raster[(lby & 3) * 4 + (lbx & 3)] = total;
                nnzs[blk & 15] = total;
            }
        }
        for by in 0..4usize {
            let a = (mb_y * 4 + by) * w4 + mb_x * 4;
            self.nnz_y[a..a + 4].copy_from_slice(&nnz_raster[by * 4..by * 4 + 4]);
        }
        let mut c_recon_dc = [[0i32; 4]; 2];
        if cbp_chroma != 0 {
            for slot in c_recon_dc.iter_mut() {
                decode_residual_block_into::<4, 4>(r, -1, slot)?;
            }
        }
        if cbp_chroma == 2 {
            self.chroma_cache_load(mb_x, mb_y);
            let w2 = self.mb_w * 2;
            let mut cnnz = [[0u8; 4]; 2];
            for c in 0..2 {
                for &(bx, by) in &CHROMA_4X4_SCAN_XY {
                    let nc = self.chroma_nc_pred(c, bx, by);
                    let slot = &mut cac[c & 1][(by * 2 + bx) & 3];
                    *slot = [0i32; 16];
                    let total = decode_residual_block_into::<15, 16>(r, nc, slot)?;
                    self.chroma_nnz_cache_set(c, bx, by, total);
                    cnnz[c & 1][(by * 2 + bx) & 3] = total;
                    nnzs[16 + (c & 1) * 4 + (by & 1) * 2 + (bx & 1)] = total;
                }
                for by in 0..2usize {
                    let a = (mb_y * 2 + by) * w2 + mb_x * 2;
                    self.nnz_c[c][a..a + 2].copy_from_slice(&cnnz[c][by * 2..by * 2 + 2]);
                }
            }
        }
        if defer {
            let (mut gmv, mut gref) = ([(0i32, 0i32); 16], [0u8; 16]);
            let w4r = self.mb_w * 4;
            for by in 0..4usize {
                let base = (mb_y * 4 + by) * w4r + mb_x * 4;
                let mvr = &self.mv_y[base..][..4];
                let rfr = &self.ref_idx_y[base..][..4];
                gmv[by * 4..][..4].copy_from_slice(mvr);
                for bx in 0..4usize {
                    gref[by * 4 + bx] = rfr[bx].clamp(0, 15) as u8;
                }
            }
            let ej = if nores {
                let b = self.take_nores_job(PInterNoResJob {
                    mbx: mb_x,
                    mby: mb_y,
                    t8: t8x8,
                    gmv,
                    gref,
                });
                EdcJob::InterNoRes(b)
            } else {
                let mut job = job.take().expect("pooled job taken above");
                job.mbx = mb_x;
                job.mby = mb_y;
                job.qp = qp;
                job.cbp_chroma = cbp_chroma;
                job.t8 = t8x8;
                job.gmv = gmv;
                job.gref = gref;
                job.cdc = c_recon_dc;
                job.nnzs = nnzs;
                EdcJob::Inter(job)
            };
            if self.edc_tx.is_some() {
                self.edc_giveback();
                self.edc_send_job(ej);
            } else {
                self.edc_jobs.push(ej);
            }
        } else {
            self.add_inter_residual(
                mb_x,
                mb_y,
                pred_y,
                c_pred,
                Some(&*luma_scan),
                t8x8.then_some(&*luma8),
                &c_recon_dc,
                Some(&*cac),
                cbp_chroma,
                &nnzs,
            );
            self.scratch_luma = s_luma;
            self.scratch_luma8 = s_luma8;
            self.scratch_cac = s_cac;
        }
        for lby in 0..4usize {
            let a = (mb_y * 4 + lby) * w4 + mb_x * 4;
            self.modes_y[a..a + 4].fill(2);
        }
        Ok(())
    }
    fn mv_neighbors_list(
        &self,
        pbx: isize,
        pby: isize,
        pwb: isize,
        list: usize,
    ) -> [MvNeighbor; 3] {
        self.mv_neighbors_block_grid(pbx, pby, pwb, list)
    }
    #[inline]
    fn b_direct_nbrs(&self, mb_x: usize, mb_y: usize) -> ([MvNeighbor; 3], [MvNeighbor; 3]) {
        self.mv_neighbors_both((mb_x * 4) as isize, (mb_y * 4) as isize, 4)
    }
    fn mv_neighbors_both(
        &self,
        pbx: isize,
        pby: isize,
        pwb: isize,
    ) -> ([MvNeighbor; 3], [MvNeighbor; 3]) {
        let (w4, h4) = ((self.mb_w * 4) as isize, (self.mb_h * 4) as isize);
        let get2 = |bx: isize, by: isize| -> (MvNeighbor, MvNeighbor) {
            if bx < 0
                || by < 0
                || bx >= w4
                || by >= h4
                || !self
                    .coded_y
                    .get((by * w4 + bx) as usize)
                    .copied()
                    .unwrap_or(false)
                || !self.nbr_in_slice(bx as usize / 4, by as usize / 4)
            {
                (MvNeighbor::NONE, MvNeighbor::NONE)
            } else {
                let idx = (by * w4 + bx) as usize;
                match (
                    self.mv_y.get(idx),
                    self.ref_idx_y.get(idx),
                    self.mv1.get(idx),
                    self.ref_idx1.get(idx),
                ) {
                    (Some(&m0), Some(&r0), Some(&m1), Some(&r1)) => (
                        MvNeighbor {
                            available: true,
                            mv: m0,
                            ref_idx: r0,
                        },
                        MvNeighbor {
                            available: true,
                            mv: m1,
                            ref_idx: r1,
                        },
                    ),
                    _ => (MvNeighbor::NONE, MvNeighbor::NONE),
                }
            }
        };
        let (a0, a1) = get2(pbx - 1, pby);
        let (b0, b1) = get2(pbx, pby - 1);
        let (mut c0, mut c1) = get2(pbx + pwb, pby - 1);
        if !c0.available {
            let t = get2(pbx - 1, pby - 1);
            c0 = t.0;
            c1 = t.1;
        }
        ([a0, b0, c0], [a1, b1, c1])
    }
    fn mv_neighbors_block_grid(
        &self,
        pbx: isize,
        pby: isize,
        pwb: isize,
        list: usize,
    ) -> [MvNeighbor; 3] {
        let (w4, h4) = ((self.mb_w * 4) as isize, (self.mb_h * 4) as isize);
        let (mvg, refg) = if list == 0 {
            (&self.mv_y, &self.ref_idx_y)
        } else {
            (&self.mv1, &self.ref_idx1)
        };
        let get = |bx: isize, by: isize| -> MvNeighbor {
            if bx < 0
                || by < 0
                || bx >= w4
                || by >= h4
                || !self
                    .coded_y
                    .get((by * w4 + bx) as usize)
                    .copied()
                    .unwrap_or(false)
                || !self.nbr_in_slice(bx as usize / 4, by as usize / 4)
            {
                MvNeighbor::NONE
            } else {
                let idx = (by * w4 + bx) as usize;
                match (mvg.get(idx), refg.get(idx)) {
                    (Some(&m), Some(&r)) => MvNeighbor {
                        available: true,
                        mv: m,
                        ref_idx: r,
                    },
                    _ => MvNeighbor::NONE,
                }
            }
        };
        let a = get(pbx - 1, pby);
        let b = get(pbx, pby - 1);
        let mut c = get(pbx + pwb, pby - 1);
        if !c.available {
            c = get(pbx - 1, pby - 1);
        }
        [a, b, c]
    }
    #[inline]
    fn col_block(&self, bx4: usize, by4: usize) -> (usize, usize) {
        if self.direct_8x8_inference {
            ((bx4 / 2) * 3, (by4 / 2) * 3)
        } else {
            (bx4, by4)
        }
    }
    fn col_zero(&self, bx: usize, by: usize) -> bool {
        if self.col_ok {
            let Some(col) = self.refs1.first() else {
                return false;
            };
            let idx = by * self.col_w4 + bx;
            let (cref, cmv) = match col.ref_idx.get(idx) {
                Some(&r0) if r0 >= 0 => match col.mv.get(idx) {
                    Some(&m) => (r0, m),
                    None => return false,
                },
                Some(_) => match (col.ref_idx1.get(idx), col.mv1.get(idx)) {
                    (Some(&r1), Some(&m)) if r1 >= 0 => (r1, m),
                    _ => return false,
                },
                None => return false,
            };
            return cref == 0 && cmv.0.abs() <= 1 && cmv.1.abs() <= 1;
        }
        let Some(col) = self.refs1.first() else {
            return false;
        };
        if let Some(live) = col.live.as_ref() {
            col.wait_motion_ready();
            let meta = live.meta.read().unwrap();
            if meta.long_term || meta.w4 == 0 {
                return false;
            }
            let idx = by * meta.w4 + bx;
            let (cref, cmv) = match meta.ref_idx.get(idx) {
                Some(&r0) if r0 >= 0 => match meta.mv.get(idx) {
                    Some(&m) => (r0, m),
                    None => return false,
                },
                Some(_) => match (meta.ref_idx1.get(idx), meta.mv1.get(idx)) {
                    (Some(&r1), Some(&m)) if r1 >= 0 => (r1, m),
                    _ => return false,
                },
                None => return false,
            };
            return cref == 0 && cmv.0.abs() <= 1 && cmv.1.abs() <= 1;
        }
        if col.long_term || col.w4 == 0 {
            return false;
        }
        let idx = by * col.w4 + bx;
        if idx >= col.ref_idx.len() {
            return false;
        }
        let (cref, cmv) = match col.ref_idx.get(idx) {
            Some(&r0) if r0 >= 0 => match col.mv.get(idx) {
                Some(&m) => (r0, m),
                None => return false,
            },
            Some(_) => match (col.ref_idx1.get(idx), col.mv1.get(idx)) {
                (Some(&r1), Some(&m)) if r1 >= 0 => (r1, m),
                _ => return false,
            },
            None => return false,
        };
        cref == 0 && cmv.0.abs() <= 1 && cmv.1.abs() <= 1
    }
    fn implicit_weights(&self, refi0: i32, refi1: i32) -> Option<(i32, i32)> {
        if self.weighted_bipred_idc != 2 || refi0 < 0 || refi1 < 0 {
            return None;
        }
        let (Some(r0), Some(r1)) = (
            self.refs.get(refi0 as usize),
            self.refs1.get(refi1 as usize),
        ) else {
            return None;
        };
        let td = (r1.pic_poc() - r0.pic_poc()).clamp(-128, 127);
        let tb = (self.cur_poc - r0.pic_poc()).clamp(-128, 127);
        if td == 0 || r0.long_term || r1.long_term {
            return None;
        }
        let tx = tx_for_td(td);
        let dsf = ((tb * tx + 32) >> 6).clamp(-1024, 1023);
        let w1 = dsf >> 2;
        if !(-64..=128).contains(&w1) {
            return None;
        }
        Some((64 - w1, w1))
    }
    fn b_mc(
        &self,
        mb_x: usize,
        mb_y: usize,
        px: usize,
        py: usize,
        rw: usize,
        rh: usize,
        refi0: i32,
        mv0: (i32, i32),
        refi1: i32,
        mv1: (i32, i32),
        pred_y: &mut [u8; 256],
        c_pred: &mut [[u8; 64]; 2],
    ) {
        let _gb = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecBMc);
        let refi0 = if refi0 >= 0 {
            (refi0 as usize).min(self.refs.len().saturating_sub(1)) as i32
        } else {
            -1
        };
        let refi1 = if refi1 >= 0 {
            (refi1 as usize).min(self.refs1.len().saturating_sub(1)) as i32
        } else {
            -1
        };
        if (refi0 >= 0 && self.refs.is_empty()) || (refi1 >= 0 && self.refs1.is_empty()) {
            return;
        }
        let (ch, cch) = (self.mb_h * 16, self.mb_h * 8);
        let weights = {
            let _gw =
                crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecBWeights);
            self.implicit_weights(refi0, refi1)
        };
        let full = px == 0 && rw == 16;
        if refi0 >= 0
            && refi1 >= 0
            && mv0.0 % 4 == 0
            && mv0.1 % 4 == 0
            && mv1.0 % 4 == 0
            && mv1.1 % 4 == 0
        {}
        let _gl = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecBLuma);
        let chroma_done = crate::h264::common::inter::with_mc_scratch(|scr| {
            match (refi0 >= 0, refi1 >= 0, full) {
                (true, false, true) => {
                    let Some(rf) = self.refs.get(refi0 as usize) else {
                        return false;
                    };
                    crate::h264::common::inter::mc_luma_padded_pre(
                        scr,
                        &*rf.luma_guard(rf.ch),
                        rf.lstride(),
                        crate::h264::decoder::LPAD,
                        self.cw,
                        ch,
                        mb_x * 16,
                        mb_y * 16 + py,
                        rw,
                        rh,
                        mv0.0,
                        mv0.1,
                        &mut pred_y[py * 16..py * 16 + rw * rh],
                    );
                    false
                }
                (false, true, true) => {
                    let Some(rf) = self.refs1.get(refi1 as usize) else {
                        return false;
                    };
                    crate::h264::common::inter::mc_luma_padded_pre(
                        scr,
                        &*rf.luma_guard(rf.ch),
                        rf.lstride(),
                        crate::h264::decoder::LPAD,
                        self.cw,
                        ch,
                        mb_x * 16,
                        mb_y * 16 + py,
                        rw,
                        rh,
                        mv1.0,
                        mv1.1,
                        &mut pred_y[py * 16..py * 16 + rw * rh],
                    );
                    false
                }
                (true, true, true) => {
                    let Some(rf) = self.refs.get(refi0 as usize) else {
                        return false;
                    };
                    crate::h264::common::inter::mc_luma_padded_pre(
                        scr,
                        &*rf.luma_guard(rf.ch),
                        rf.lstride(),
                        crate::h264::decoder::LPAD,
                        self.cw,
                        ch,
                        mb_x * 16,
                        mb_y * 16 + py,
                        rw,
                        rh,
                        mv0.0,
                        mv0.1,
                        &mut pred_y[py * 16..py * 16 + rw * rh],
                    );
                    let mut b = [0u8; 256];
                    let Some(rf) = self.refs1.get(refi1 as usize) else {
                        return false;
                    };
                    crate::h264::common::inter::mc_luma_padded_pre(
                        scr,
                        &*rf.luma_guard(rf.ch),
                        rf.lstride(),
                        crate::h264::decoder::LPAD,
                        self.cw,
                        ch,
                        mb_x * 16,
                        mb_y * 16 + py,
                        rw,
                        rh,
                        mv1.0,
                        mv1.1,
                        &mut b[..rw * rh],
                    );
                    drop(_gl);
                    let _gbl = crate::h264::common::prof::scope(
                        crate::h264::common::prof::Stage::DecBBlend,
                    );
                    let dst = &mut pred_y[py * 16..py * 16 + rw * rh];
                    match weights {
                        None => {
                            for (d, s) in dst.iter_mut().zip(&b[..rw * rh]) {
                                *d = ((*d as u16 + *s as u16 + 1) >> 1) as u8;
                            }
                        }
                        Some((w0, w1)) => {
                            for (d, s) in dst.iter_mut().zip(&b[..rw * rh]) {
                                *d = ((*d as i32 * w0 + *s as i32 * w1 + 32) >> 6).clamp(0, 255)
                                    as u8;
                            }
                        }
                    }
                    let _gc = crate::h264::common::prof::scope(
                        crate::h264::common::prof::Stage::DecBChroma,
                    );
                    self.b_mc_chroma(
                        mb_x, mb_y, px, py, rw, rh, refi0, mv0, refi1, mv1, c_pred, weights, cch,
                    );
                    true
                }
                _ => {
                    let (mut a, mut b) = ([0u8; 256], [0u8; 256]);
                    if refi0 >= 0 {
                        let Some(rf) = self.refs.get(refi0 as usize) else {
                            return false;
                        };
                        crate::h264::common::inter::mc_luma_padded_pre(
                            scr,
                            &*rf.luma_guard(rf.ch),
                            rf.lstride(),
                            crate::h264::decoder::LPAD,
                            self.cw,
                            ch,
                            mb_x * 16 + px,
                            mb_y * 16 + py,
                            rw,
                            rh,
                            mv0.0,
                            mv0.1,
                            &mut a[..rw * rh],
                        );
                    }
                    if refi1 >= 0 {
                        let Some(rf) = self.refs1.get(refi1 as usize) else {
                            return false;
                        };
                        crate::h264::common::inter::mc_luma_padded_pre(
                            scr,
                            &*rf.luma_guard(rf.ch),
                            rf.lstride(),
                            crate::h264::decoder::LPAD,
                            self.cw,
                            ch,
                            mb_x * 16 + px,
                            mb_y * 16 + py,
                            rw,
                            rh,
                            mv1.0,
                            mv1.1,
                            &mut b[..rw * rh],
                        );
                    }
                    drop(_gl);
                    let _gbl = crate::h264::common::prof::scope(
                        crate::h264::common::prof::Stage::DecBBlend,
                    );
                    match (refi0 >= 0, refi1 >= 0) {
                        (true, true) => {
                            for dy in 0..rh {
                                let (ar, br) =
                                    (&a[dy * rw..dy * rw + rw], &b[dy * rw..dy * rw + rw]);
                                let base = (py + dy) * 16 + px;
                                let dst = &mut pred_y[base..base + rw];
                                match weights {
                                    None => {
                                        for ((d, p), q) in dst.iter_mut().zip(ar).zip(br) {
                                            *d = ((*p as u16 + *q as u16 + 1) >> 1) as u8;
                                        }
                                    }
                                    Some((w0, w1)) => {
                                        for ((d, p), q) in dst.iter_mut().zip(ar).zip(br) {
                                            *d = ((*p as i32 * w0 + *q as i32 * w1 + 32) >> 6)
                                                .clamp(0, 255)
                                                as u8;
                                        }
                                    }
                                }
                            }
                        }
                        (true, false) => {
                            for dy in 0..rh {
                                let d = (py + dy) * 16 + px;
                                pred_y[d..d + rw].copy_from_slice(&a[dy * rw..dy * rw + rw]);
                            }
                        }
                        _ => {
                            for dy in 0..rh {
                                let d = (py + dy) * 16 + px;
                                pred_y[d..d + rw].copy_from_slice(&b[dy * rw..dy * rw + rw]);
                            }
                        }
                    }
                    false
                }
            }
        });
        if chroma_done {
            return;
        }
        let _gc = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecBChroma);
        self.b_mc_chroma(
            mb_x, mb_y, px, py, rw, rh, refi0, mv0, refi1, mv1, c_pred, weights, cch,
        );
    }
    fn b_mc_chroma(
        &self,
        mb_x: usize,
        mb_y: usize,
        px: usize,
        py: usize,
        rw: usize,
        rh: usize,
        refi0: i32,
        mv0: (i32, i32),
        refi1: i32,
        mv1: (i32, i32),
        c_pred: &mut [[u8; 64]; 2],
        weights: Option<(i32, i32)>,
        cch: usize,
    ) {
        use crate::h264::common::inter::mc_chroma_padded_pair;
        let (crx, cry, crw, crh) = (px / 2, py / 2, rw / 2, rh / 2);
        let full = crx == 0 && crw == 8;
        let n = crw * crh;
        let rf0 = (refi0 >= 0)
            .then(|| self.refs.get(refi0 as usize))
            .flatten();
        let rf1 = (refi1 >= 0)
            .then(|| self.refs1.get(refi1 as usize))
            .flatten();
        let (a0, a1) = (rf0.is_some(), rf1.is_some());
        let [cu, cv] = c_pred;
        match (a0, a1, full) {
            (true, false, true) | (false, true, true) => {
                let (Some(rf), mv) = (if a0 { rf0 } else { rf1 }, if a0 { mv0 } else { mv1 })
                else {
                    return;
                };
                let (gu, gv) = (rf.chroma_guard(0, rf.ch), rf.chroma_guard(1, rf.ch));
                mc_chroma_padded_pair(
                    &gu,
                    &gv,
                    rf.cstride(),
                    crate::h264::decoder::CPAD,
                    self.ccw,
                    cch,
                    mb_x * 8,
                    mb_y * 8 + cry,
                    crw,
                    crh,
                    mv.0,
                    mv.1,
                    &mut cu[cry * 8..cry * 8 + n],
                    &mut cv[cry * 8..cry * 8 + n],
                );
            }
            (true, true, true) => {
                let (Some(rfa), Some(rfb)) = (rf0, rf1) else {
                    return;
                };
                {
                    let rf = rfa;
                    let (gu, gv) = (rf.chroma_guard(0, rf.ch), rf.chroma_guard(1, rf.ch));
                    mc_chroma_padded_pair(
                        &gu,
                        &gv,
                        rf.cstride(),
                        crate::h264::decoder::CPAD,
                        self.ccw,
                        cch,
                        mb_x * 8,
                        mb_y * 8 + cry,
                        crw,
                        crh,
                        mv0.0,
                        mv0.1,
                        &mut cu[cry * 8..cry * 8 + n],
                        &mut cv[cry * 8..cry * 8 + n],
                    );
                }
                let (mut bu, mut bv) = ([0u8; 64], [0u8; 64]);
                {
                    let rf = rfb;
                    let (gu, gv) = (rf.chroma_guard(0, rf.ch), rf.chroma_guard(1, rf.ch));
                    mc_chroma_padded_pair(
                        &gu,
                        &gv,
                        rf.cstride(),
                        crate::h264::decoder::CPAD,
                        self.ccw,
                        cch,
                        mb_x * 8,
                        mb_y * 8 + cry,
                        crw,
                        crh,
                        mv1.0,
                        mv1.1,
                        &mut bu[..n],
                        &mut bv[..n],
                    );
                }
                for (dst, stage) in [(&mut *cu, &bu), (&mut *cv, &bv)] {
                    let d = &mut dst[cry * 8..cry * 8 + n];
                    match weights {
                        None => {
                            for (o, q) in d.iter_mut().zip(&stage[..n]) {
                                *o = ((*o as u16 + *q as u16 + 1) >> 1) as u8;
                            }
                        }
                        Some((w0, w1)) => {
                            for (o, q) in d.iter_mut().zip(&stage[..n]) {
                                *o = ((*o as i32 * w0 + *q as i32 * w1 + 32) >> 6).clamp(0, 255)
                                    as u8;
                            }
                        }
                    }
                }
            }
            _ => {
                let (mut au, mut av, mut bu, mut bv) = ([0u8; 64], [0u8; 64], [0u8; 64], [0u8; 64]);
                if let Some(rf) = rf0 {
                    let (gu, gv) = (rf.chroma_guard(0, rf.ch), rf.chroma_guard(1, rf.ch));
                    mc_chroma_padded_pair(
                        &gu,
                        &gv,
                        rf.cstride(),
                        crate::h264::decoder::CPAD,
                        self.ccw,
                        cch,
                        mb_x * 8 + crx,
                        mb_y * 8 + cry,
                        crw,
                        crh,
                        mv0.0,
                        mv0.1,
                        &mut au[..n],
                        &mut av[..n],
                    );
                }
                if let Some(rf) = rf1 {
                    let (gu, gv) = (rf.chroma_guard(0, rf.ch), rf.chroma_guard(1, rf.ch));
                    mc_chroma_padded_pair(
                        &gu,
                        &gv,
                        rf.cstride(),
                        crate::h264::decoder::CPAD,
                        self.ccw,
                        cch,
                        mb_x * 8 + crx,
                        mb_y * 8 + cry,
                        crw,
                        crh,
                        mv1.0,
                        mv1.1,
                        &mut bu[..n],
                        &mut bv[..n],
                    );
                }
                for (dst, sa, sb) in [(&mut *cu, &au, &bu), (&mut *cv, &av, &bv)] {
                    for dy in 0..crh {
                        let base = (cry + dy) * 8 + crx;
                        let d = &mut dst[base..base + crw];
                        match (a0, a1) {
                            (true, true) => {
                                let (pr, qr) =
                                    (&sa[dy * crw..dy * crw + crw], &sb[dy * crw..dy * crw + crw]);
                                match weights {
                                    None => {
                                        for ((o, pp), q) in d.iter_mut().zip(pr).zip(qr) {
                                            *o = ((*pp as u16 + *q as u16 + 1) >> 1) as u8;
                                        }
                                    }
                                    Some((w0, w1)) => {
                                        for ((o, pp), q) in d.iter_mut().zip(pr).zip(qr) {
                                            *o = ((*pp as i32 * w0 + *q as i32 * w1 + 32) >> 6)
                                                .clamp(0, 255)
                                                as u8;
                                        }
                                    }
                                }
                            }
                            (true, false) => d.copy_from_slice(&sa[dy * crw..dy * crw + crw]),
                            _ => d.copy_from_slice(&sb[dy * crw..dy * crw + crw]),
                        }
                    }
                }
            }
        }
    }
    fn b_set_motion(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        px: usize,
        py: usize,
        rw: usize,
        rh: usize,
        refi0: i32,
        mv0: (i32, i32),
        refi1: i32,
        mv1: (i32, i32),
    ) {
        let _gs = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecBSet);
        let w4 = self.mb_w * 4;
        let mv0w = if refi0 >= 0 { mv0 } else { (0, 0) };
        let mv1w = if refi1 >= 0 { mv1 } else { (0, 0) };
        let by0 = mb_y * 4 + py / 4;
        let bx0 = mb_x * 4 + px / 4;
        let (bw, bh) = (rw / 4, rh / 4);
        for by in by0..by0 + bh {
            let row = by * w4 + bx0;
            let end = row + bw;
            let (Some(r0), Some(m0), Some(r1), Some(m1)) = (
                self.ref_idx_y.get_mut(row..end),
                self.mv_y.get_mut(row..end),
                self.ref_idx1.get_mut(row..end),
                self.mv1.get_mut(row..end),
            ) else {
                continue;
            };
            r0.fill(refi0);
            m0.fill(mv0w);
            r1.fill(refi1);
            m1.fill(mv1w);
            let (Some(it), Some(cd), Some(md)) = (
                self.inter_y.get_mut(row..end),
                self.coded_y.get_mut(row..end),
                self.modes_y.get_mut(row..end),
            ) else {
                continue;
            };
            it.fill(true);
            cd.fill(true);
            md.fill(2);
        }
    }
    fn coalesce_region<
        U: Fn(usize, usize, usize, usize) -> bool,
        E: FnMut(usize, usize, usize, usize),
    >(
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        uniform: &U,
        emit: &mut E,
    ) {
        if uniform(x, y, w, h) {
            emit(x, y, w, h);
            return;
        }
        if h > 1 && uniform(x, y, w, h / 2) && uniform(x, y + h / 2, w, h / 2) {
            emit(x, y, w, h / 2);
            emit(x, y + h / 2, w, h / 2);
            return;
        }
        if w > 1 && uniform(x, y, w / 2, h) && uniform(x + w / 2, y, w / 2, h) {
            emit(x, y, w / 2, h);
            emit(x + w / 2, y, w / 2, h);
            return;
        }
        match (w > 1, h > 1) {
            (true, true) => {
                for q in 0..4usize {
                    Self::coalesce_region(
                        x + (q % 2) * (w / 2),
                        y + (q / 2) * (h / 2),
                        w / 2,
                        h / 2,
                        uniform,
                        emit,
                    );
                }
            }
            (true, false) => {
                Self::coalesce_region(x, y, w / 2, h, uniform, emit);
                Self::coalesce_region(x + w / 2, y, w / 2, h, uniform, emit);
            }
            (false, true) => {
                Self::coalesce_region(x, y, w, h / 2, uniform, emit);
                Self::coalesce_region(x, y + h / 2, w, h / 2, uniform, emit);
            }
            (false, false) => emit(x, y, 1, 1),
        }
    }
    fn decode_b_direct(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        px: usize,
        py: usize,
        rw: usize,
        rh: usize,
        pred_y: &mut [u8; 256],
        c_pred: &mut [[u8; 64]; 2],
    ) {
        if !self.direct_spatial {
            return self.decode_b_direct_temporal(mb_x, mb_y, px, py, rw, rh, pred_y, c_pred);
        }
        let (n0, n1) = self.b_direct_nbrs(mb_x, mb_y);
        self.decode_b_direct_n(mb_x, mb_y, px, py, rw, rh, pred_y, c_pred, n0, n1);
    }
    #[inline]
    fn b_direct_refs_mvs(
        n0: &[MvNeighbor; 3],
        n1: &[MvNeighbor; 3],
    ) -> (i32, i32, (i32, i32), (i32, i32), bool) {
        let min_pos = |a: i32, b: i32| {
            if a < 0 {
                b
            } else if b < 0 {
                a
            } else {
                a.min(b)
            }
        };
        let rid = |n: &[MvNeighbor; 3]| min_pos(min_pos(n[0].ref_idx, n[1].ref_idx), n[2].ref_idx);
        let (mut refi0, mut refi1) = (rid(n0), rid(n1));
        let direct_zero = refi0 < 0 && refi1 < 0;
        if direct_zero {
            refi0 = 0;
            refi1 = 0;
        }
        let mv0 = if refi0 >= 0 && !direct_zero {
            predict_mv(n0[0], n0[1], n0[2], refi0)
        } else {
            (0, 0)
        };
        let mv1 = if refi1 >= 0 && !direct_zero {
            predict_mv(n1[0], n1[1], n1[2], refi1)
        } else {
            (0, 0)
        };
        (refi0, refi1, mv0, mv1, direct_zero)
    }
    fn decode_b_direct_n(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        px: usize,
        py: usize,
        rw: usize,
        rh: usize,
        pred_y: &mut [u8; 256],
        c_pred: &mut [[u8; 64]; 2],
        n0: [MvNeighbor; 3],
        n1: [MvNeighbor; 3],
    ) {
        let _gb = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecBDirect);
        let gd = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecBDeriv);
        let derived = Self::b_direct_refs_mvs(&n0, &n1);
        drop(gd);
        self.b_direct_region(mb_x, mb_y, px, py, rw, rh, pred_y, c_pred, derived);
    }
    fn b_direct_region(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        px: usize,
        py: usize,
        rw: usize,
        rh: usize,
        pred_y: &mut [u8; 256],
        c_pred: &mut [[u8; 64]; 2],
        derived: (i32, i32, (i32, i32), (i32, i32), bool),
    ) {
        let (refi0, refi1, mv0, mv1, direct_zero) = derived;
        let (bx0, by0, bw, bh) = (px / 4, py / 4, rw / 4, rh / 4);
        let mut czg = [[false; 4]; 4];
        let cz_matters = (refi0 == 0 && mv0 != (0, 0)) || (refi1 == 0 && mv1 != (0, 0));
        let mut cz_mixed = false;
        if cz_matters && !direct_zero && self.direct_8x8_inference {
            let mut oy = 0usize;
            while oy < bh {
                let h = (bh - oy).min(2);
                let mut ox = 0usize;
                while ox < bw {
                    let w = (bw - ox).min(2);
                    let (colx, coly) = self.col_block(bx0 + ox, by0 + oy);
                    let cz = self.col_zero(mb_x * 4 + colx, mb_y * 4 + coly);
                    cz_mixed |= cz != czg[0][0] && !(ox == 0 && oy == 0);
                    for dy in oy..oy + h {
                        for dx in ox..ox + w {
                            czg[dy & 3][dx & 3] = cz;
                        }
                    }
                    ox += w;
                }
                oy += h;
            }
        } else if cz_matters && !direct_zero {
            for dy in 0..bh {
                for dx in 0..bw {
                    let (colx, coly) = self.col_block(bx0 + dx, by0 + dy);
                    let cz = self.col_zero(mb_x * 4 + colx, mb_y * 4 + coly);
                    cz_mixed |= cz != czg[0][0] && !(dx == 0 && dy == 0);
                    czg[dy & 3][dx & 3] = cz;
                }
            }
        }
        let mut rects: [(usize, usize, usize, usize); 16] = [(0, 0, 0, 0); 16];
        let mut n = 0usize;
        if !cz_mixed {
            rects[0] = (0, 0, bw, bh);
            n = 1;
        } else {
            let uniform = |x: usize, y: usize, w: usize, h: usize| -> bool {
                let t = czg[y & 3][x & 3];
                (y..y + h).all(|dy| (x..x + w).all(|dx| czg[dy & 3][dx & 3] == t))
            };
            Self::coalesce_region(0, 0, bw, bh, &uniform, &mut |x, y, w, h| {
                rects[n & 15] = (x, y, w, h);
                n += 1;
            });
        }
        let recording = self.edc_regions.is_some();
        if rw == 16 && rh == 16 {
            if n == 1 {
                let cz = czg[0][0];
                let m0 = if refi0 == 0 && cz { (0, 0) } else { mv0 };
                let m1 = if refi1 == 0 && cz { (0, 0) } else { mv1 };
                let (a0, a1) = (refi0 >= 0, refi1 >= 0);
                if a0 && a1 && m0 == (0, 0) && m1 == (0, 0) {
                } else if (a0 != a1) && (if a0 { m0 } else { m1 }) == (0, 0) {
                } else if (!a0 || (m0.0 % 4 == 0 && m0.1 % 4 == 0))
                    && (!a1 || (m1.0 % 4 == 0 && m1.1 % 4 == 0))
                {
                }
                if self.bsk_last == Some((mb_x.wrapping_sub(1), mb_y, refi0, refi1, m0, m1)) {}
                self.bsk_last = Some((mb_x, mb_y, refi0, refi1, m0, m1));
            } else {
                self.bsk_last = None;
            }
        }
        for &(x, y, w, h) in &rects[..n] {
            let cz = czg[y & 3][x & 3];
            let m0 = if refi0 == 0 && cz { (0, 0) } else { mv0 };
            let m1 = if refi1 == 0 && cz { (0, 0) } else { mv1 };
            let (lx, ly, lw, lh) = ((bx0 + x) * 4, (by0 + y) * 4, w * 4, h * 4);
            if recording {
                self.b_mc_or_record(
                    mb_x, mb_y, lx, ly, lw, lh, refi0, m0, refi1, m1, pred_y, c_pred,
                );
            } else {
                self.b_mc(
                    mb_x, mb_y, lx, ly, lw, lh, refi0, m0, refi1, m1, pred_y, c_pred,
                );
            }
            self.b_set_motion(mb_x, mb_y, lx, ly, lw, lh, refi0, m0, refi1, m1);
        }
    }
    fn decode_b_direct_temporal(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        px: usize,
        py: usize,
        rw: usize,
        rh: usize,
        pred_y: &mut [u8; 256],
        c_pred: &mut [[u8; 64]; 2],
    ) {
        let poc1 = self.refs1.first().map_or(0, |f| f.pic_poc());
        let infer = self.direct_8x8_inference;
        let step = if infer { 8 } else { 4 };
        let mut sy = py;
        while sy < py + rh {
            let mut sx = px;
            while sx < px + rw {
                let (colx, coly) = self.col_block(sx / 4, sy / 4);
                let (mvcol, refpoc) = {
                    let Some(col) = self.refs1.first() else {
                        return;
                    };
                    if let Some(live) = col.live.as_ref() {
                        col.wait_motion_ready();
                        let meta = live.meta.read().unwrap();
                        let idx = (mb_y * 4 + coly) * meta.w4 + (mb_x * 4 + colx);
                        match (meta.mv.get(idx), meta.ref_poc.get(idx)) {
                            (Some(&m), Some(&pc)) if meta.w4 != 0 && pc != i32::MIN => (m, pc),
                            _ => ((0, 0), i32::MIN),
                        }
                    } else {
                        let idx = (mb_y * 4 + coly) * col.w4 + (mb_x * 4 + colx);
                        match (col.mv.get(idx), col.ref_poc.get(idx)) {
                            (Some(&m), Some(&pc)) if col.w4 != 0 && pc != i32::MIN => (m, pc),
                            _ => ((0, 0), i32::MIN),
                        }
                    }
                };
                let (refi0, mvc) = if refpoc == i32::MIN {
                    (0, (0, 0))
                } else {
                    let r = self
                        .refs
                        .iter()
                        .position(|f| f.pic_poc() == refpoc)
                        .unwrap_or(0) as i32;
                    (r, mvcol)
                };
                let Some(r0) = self.refs.get(refi0 as usize) else {
                    return;
                };
                let poc0 = r0.pic_poc();
                let td = (poc1 - poc0).clamp(-128, 127);
                let tb = (self.cur_poc - poc0).clamp(-128, 127);
                let (mv0, mv1) = if td == 0 || r0.long_term {
                    (mvc, (0, 0))
                } else {
                    let tx = tx_for_td(td);
                    let dsf = ((tb * tx + 32) >> 6).clamp(-1024, 1023);
                    let m0 = ((dsf * mvc.0 + 128) >> 8, (dsf * mvc.1 + 128) >> 8);
                    (m0, (m0.0 - mvc.0, m0.1 - mvc.1))
                };
                self.b_mc_or_record(
                    mb_x, mb_y, sx, sy, step, step, refi0, mv0, 0, mv1, pred_y, c_pred,
                );
                self.b_set_motion(mb_x, mb_y, sx, sy, step, step, refi0, mv0, 0, mv1);
                sx += step;
            }
            sy += step;
        }
    }
    fn read_b_ref(&self, r: &mut BitReader, list: usize) -> Result<i32, MbError> {
        let (active, avail) = if list == 0 {
            (self.num_ref_active, self.refs.len())
        } else {
            (self.num_ref_active1, self.refs1.len())
        };
        let v = if active > 1 {
            read_ref_idx(r, active)?
        } else {
            0
        };
        if v as usize >= avail {
            return Err(MbError::Truncated);
        }
        Ok(v)
    }
    #[inline]
    fn bz_push(&mut self, mb_x: usize, mb_y: usize, kind: BzKind) {
        match self.bzspan {
            Some((row, x0, ref mut n, k)) if row == mb_y && x0 + *n == mb_x && k == kind => *n += 1,
            _ => {
                self.bz_flush();
                self.bzspan = Some((mb_y, mb_x, 1, kind));
            }
        }
    }
    #[inline(always)]
    fn bz_flush(&mut self) {
        if self.bzspan.is_some() {
            self.bz_flush_slow();
        }
    }
    fn bz_flush_slow(&mut self) {
        let _sg = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::BSpanRecon);
        let Some((row, x0, n, kind)) = self.bzspan.take() else {
            return;
        };
        let (g_r0, g_r1, g_m0, g_m1): (i32, i32, (i32, i32), (i32, i32)) = match kind {
            BzKind::ZeroBi => (0, 0, (0, 0), (0, 0)),
            BzKind::ZeroUni(0, ri) => (ri as i32, -1, (0, 0), (0, 0)),
            BzKind::ZeroUni(_, ri) => (-1, ri as i32, (0, 0), (0, 0)),
            BzKind::Fp { r0, r1, m0, m1 } => {
                let mm0 = if r0 >= 0 {
                    (m0.0 as i32, m0.1 as i32)
                } else {
                    (0, 0)
                };
                let mm1 = if r1 >= 0 {
                    (m1.0 as i32, m1.1 as i32)
                } else {
                    (0, 0)
                };
                (r0 as i32, r1 as i32, mm0, mm1)
            }
        };
        let w4 = self.mb_w * 4;
        let (b0, len) = (x0 * 4, n * 4);
        for dy in 0..4 {
            let a = (row * 4 + dy) * w4 + b0;
            self.ref_idx_y[a..a + len].fill(g_r0);
            self.mv_y[a..a + len].fill(g_m0);
            self.ref_idx1[a..a + len].fill(g_r1);
            self.mv1[a..a + len].fill(g_m1);
            self.inter_y[a..a + len].fill(true);
            self.coded_y[a..a + len].fill(true);
            self.modes_y[a..a + len].fill(2);
            self.nnz_y[a..a + len].fill(0);
        }
        match kind {
            BzKind::ZeroBi => self.bz_recon_band_bi(row, x0, n),
            BzKind::ZeroUni(list, ri) => {
                self.bz_recon_band_copy(row, x0, n, list as usize, ri as usize, (0, 0))
            }
            BzKind::Fp { r0, r1, m0, m1 } => {
                let mv0 = (m0.0 as i32, m0.1 as i32);
                let mv1 = (m1.0 as i32, m1.1 as i32);
                match (r0 >= 0, r1 >= 0) {
                    (true, true) => {
                        self.bz_recon_band_fp_bi(row, x0, n, r0 as usize, r1 as usize, mv0, mv1)
                    }
                    (true, false) => self.bz_recon_band_copy(row, x0, n, 0, r0 as usize, mv0),
                    _ => self.bz_recon_band_copy(row, x0, n, 1, r1 as usize, mv1),
                }
            }
        }
    }
    fn bz_recon_band_bi(&mut self, row: usize, x0: usize, n: usize) {
        let w = n * 16;
        let Some(rf0) = self.refs.first() else { return };
        let Some(rf1) = self.refs1.first() else {
            return;
        };
        let (l0st, l1st) = (rf0.lstride(), rf1.lstride());
        {
            let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::SkipRecon);
            let (ly0, ly1) = (rf0.luma_guard(rf0.ch), rf1.luma_guard(rf1.ch));
            for dy in 0..16 {
                let y = row * 16 + dy;
                let s0 =
                    (y + crate::h264::decoder::LPAD) * l0st + crate::h264::decoder::LPAD + x0 * 16;
                let s1 =
                    (y + crate::h264::decoder::LPAD) * l1st + crate::h264::decoder::LPAD + x0 * 16;
                let d = y * self.cw + x0 * 16;
                crate::h264::common::inter::avg_row_into(
                    &ly0[s0..s0 + w],
                    &ly1[s1..s1 + w],
                    w,
                    &mut self.rec_y[d..d + w],
                );
            }
        }
        let (c0st, c1st) = (rf0.cstride(), rf1.cstride());
        let wc = n * 8;
        for c in 0..2 {
            let rc0 = rf0.chroma_guard(c, rf0.ch);
            let rc1 = rf1.chroma_guard(c, rf1.ch);
            let plane = if c == 0 {
                &mut self.rec_u
            } else {
                &mut self.rec_v
            };
            for dy in 0..8 {
                let y = row * 8 + dy;
                let s0 =
                    (y + crate::h264::decoder::CPAD) * c0st + crate::h264::decoder::CPAD + x0 * 8;
                let s1 =
                    (y + crate::h264::decoder::CPAD) * c1st + crate::h264::decoder::CPAD + x0 * 8;
                let d = y * self.ccw + x0 * 8;
                crate::h264::common::inter::avg_row_into(
                    &rc0[s0..s0 + wc],
                    &rc1[s1..s1 + wc],
                    wc,
                    &mut plane[d..d + wc],
                );
            }
        }
    }
    fn bz_recon_band_copy(
        &mut self,
        row: usize,
        x0: usize,
        n: usize,
        list: usize,
        ri: usize,
        mv: (i32, i32),
    ) {
        let Some(rf) = (if list == 0 {
            self.refs.get(ri)
        } else {
            self.refs1.get(ri)
        }) else {
            return;
        };
        let lst = rf.lstride();
        let (dx, dy) = ((mv.0 / 4) as isize, (mv.1 / 4) as isize);
        let w = n * 16;
        {
            let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::SkipRecon);
            let ly = rf.luma_guard(rf.ch);
            for r in 0..16 {
                let y = (row * 16 + r) as isize + dy;
                let src = ((y + crate::h264::decoder::LPAD as isize) * lst as isize
                    + crate::h264::decoder::LPAD as isize
                    + x0 as isize * 16
                    + dx) as usize;
                let d = (row * 16 + r) * self.cw + x0 * 16;
                self.rec_y[d..d + w].copy_from_slice(&ly[src..src + w]);
            }
        }
        let cst = rf.cstride();
        let (cdx, cdy) = ((mv.0 / 8) as isize, (mv.1 / 8) as isize);
        let wc = n * 8;
        for c in 0..2 {
            let rc = rf.chroma_guard(c, rf.ch);
            let plane = if c == 0 {
                &mut self.rec_u
            } else {
                &mut self.rec_v
            };
            for r in 0..8 {
                let y = (row * 8 + r) as isize + cdy;
                let src = ((y + crate::h264::decoder::CPAD as isize) * cst as isize
                    + crate::h264::decoder::CPAD as isize
                    + x0 as isize * 8
                    + cdx) as usize;
                let d = (row * 8 + r) * self.ccw + x0 * 8;
                plane[d..d + wc].copy_from_slice(&rc[src..src + wc]);
            }
        }
    }
    fn bz_recon_band_fp_bi(
        &mut self,
        row: usize,
        x0: usize,
        n: usize,
        r0: usize,
        r1: usize,
        mv0: (i32, i32),
        mv1: (i32, i32),
    ) {
        let (Some(rf0), Some(rf1)) = (self.refs.get(r0), self.refs1.get(r1)) else {
            return;
        };
        let (l0, l1) = (rf0.lstride(), rf1.lstride());
        let off = |st: usize, pad: isize, yy: isize, xx: isize| {
            (((yy + pad) * st as isize) + pad + xx) as usize
        };
        let w = n * 16;
        {
            let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::SkipRecon);
            let (ly0, ly1) = (rf0.luma_guard(rf0.ch), rf1.luma_guard(rf1.ch));
            for r in 0..16 {
                let y = (row * 16 + r) as isize;
                let s0 = off(
                    l0,
                    crate::h264::decoder::LPAD as isize,
                    y + (mv0.1 / 4) as isize,
                    x0 as isize * 16 + (mv0.0 / 4) as isize,
                );
                let s1 = off(
                    l1,
                    crate::h264::decoder::LPAD as isize,
                    y + (mv1.1 / 4) as isize,
                    x0 as isize * 16 + (mv1.0 / 4) as isize,
                );
                let d = (row * 16 + r) * self.cw + x0 * 16;
                crate::h264::common::inter::avg_row_into(
                    &ly0[s0..s0 + w],
                    &ly1[s1..s1 + w],
                    w,
                    &mut self.rec_y[d..d + w],
                );
            }
        }
        let (c0, c1) = (rf0.cstride(), rf1.cstride());
        let wc = n * 8;
        for c in 0..2 {
            let rc0 = rf0.chroma_guard(c, rf0.ch);
            let rc1 = rf1.chroma_guard(c, rf1.ch);
            let plane = if c == 0 {
                &mut self.rec_u
            } else {
                &mut self.rec_v
            };
            for r in 0..8 {
                let y = (row * 8 + r) as isize;
                let s0 = off(
                    c0,
                    crate::h264::decoder::CPAD as isize,
                    y + (mv0.1 / 8) as isize,
                    x0 as isize * 8 + (mv0.0 / 8) as isize,
                );
                let s1 = off(
                    c1,
                    crate::h264::decoder::CPAD as isize,
                    y + (mv1.1 / 8) as isize,
                    x0 as isize * 8 + (mv1.0 / 8) as isize,
                );
                let d = (row * 8 + r) * self.ccw + x0 * 8;
                crate::h264::common::inter::avg_row_into(
                    &rc0[s0..s0 + wc],
                    &rc1[s1..s1 + wc],
                    wc,
                    &mut plane[d..d + wc],
                );
            }
        }
    }
    fn bz_fp_valid(
        &self,
        mbx: usize,
        mby: usize,
        r0: Option<usize>,
        r1: Option<usize>,
        mv0: (i32, i32),
        mv1: (i32, i32),
    ) -> bool {
        let chk = |rf: &crate::h264::decoder::Ref, mv: (i32, i32)| -> bool {
            if mv.0 % 8 != 0 || mv.1 % 8 != 0 {
                return false;
            }
            let lst = rf.lstride();
            let lpad = crate::h264::decoder::LPAD as isize;
            let cx = mbx as isize * 16 + (mv.0 / 4) as isize + lpad;
            let cy = mby as isize * 16 + (mv.1 / 4) as isize + lpad;
            let lrows = rf.lrows() as isize;
            if cx < 0 || cx + 16 > lst as isize || cy < 0 || cy + 16 > lrows {
                return false;
            }
            let cst = rf.cstride();
            let cpad = crate::h264::decoder::CPAD as isize;
            let ccx = mbx as isize * 8 + (mv.0 / 8) as isize + cpad;
            let ccy = mby as isize * 8 + (mv.1 / 8) as isize + cpad;
            let crows = rf.crows() as isize;
            ccx >= 0 && ccx + 8 <= cst as isize && ccy >= 0 && ccy + 8 <= crows
        };
        r0.is_none_or(|i| self.refs.get(i).is_some_and(|r| chk(r, mv0)))
            && r1.is_none_or(|i| self.refs1.get(i).is_some_and(|r| chk(r, mv1)))
    }
    #[inline]
    fn pz_push(&mut self, mb_x: usize, mb_y: usize, recon: bool) {
        match self.pzspan {
            Some((row, x0, ref mut n, r)) if row == mb_y && x0 + *n == mb_x && r == recon => {
                *n += 1
            }
            _ => {
                self.pz_flush();
                self.pzspan = Some((mb_y, mb_x, 1, recon));
            }
        }
    }
    #[inline(always)]
    fn pz_flush(&mut self) {
        if self.pzspan.is_some() {
            self.pz_flush_slow();
        }
    }
    fn pz_flush_slow(&mut self) {
        let _sg = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::BSpanRecon);
        let Some((row, x0, n, recon)) = self.pzspan.take() else {
            return;
        };
        let w4 = self.mb_w * 4;
        let (b0, len) = (x0 * 4, n * 4);
        for dy in 0..4 {
            let a = (row * 4 + dy) * w4 + b0;
            self.mv_y[a..a + len].fill((0, 0));
            self.inter_y[a..a + len].fill(true);
            self.ref_idx_y[a..a + len].fill(0);
            self.coded_y[a..a + len].fill(true);
            self.modes_y[a..a + len].fill(2);
        }
        self.mb_kind[row * self.mb_w + x0..row * self.mb_w + x0 + n]
            .fill(crate::h264::common::deblock::MB_KIND_SKIP);
        if recon {
            self.recon_p_skip_band(x0, row, n);
        }
    }
    #[inline]
    fn span_flush(&mut self) {
        self.bz_flush();
        self.pz_flush();
    }
    #[inline(always)]
    fn b_skip_hot(&mut self, mb_x: usize, mb_y: usize) -> bool {
        if !self.direct_spatial || self.edc_tx.is_some() || no_bskipfast() {
            return false;
        }
        let mbw = self.mb_w;
        let addr = mb_y * mbw + mb_x;
        let up = addr.wrapping_sub(mbw);
        if !(mb_x > 0
            && mb_y > 0
            && mb_x + 1 < mbw
            && up >= self.slice_first_mb
            && self.bzero.get(addr - 1).copied().unwrap_or(false)
            && self.bzero.get(up).copied().unwrap_or(false)
            && self.bzero.get(up + 1).copied().unwrap_or(false))
        {
            return false;
        }
        let wgt = self.iw00();
        if !(wgt.is_none() || wgt == Some((32, 32))) {
            return false;
        }
        self.route_skip_mbs += 1;
        let _sg = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::BSkipHot);
        self.bz_push(mb_x, mb_y, BzKind::ZeroBi);
        if let Some(b) = self.bzero.get_mut(addr) {
            *b = true;
        }
        true
    }
    #[inline]
    fn clear_mb_nnz(&mut self, mb_x: usize, mb_y: usize, w4: usize) {
        let mut a = (mb_y * 4) * w4 + mb_x * 4;
        for _ in 0..4 {
            self.nnz_y[a..a + 4].fill(0);
            a += w4;
        }
    }
    fn b_skip_slow(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        nbrs: Option<([MvNeighbor; 3], [MvNeighbor; 3])>,
        w4: usize,
    ) -> Result<(), MbError> {
        let mut pred_y = [0u8; 256];
        let mut c_pred = [[0u8; 64]; 2];
        match nbrs {
            Some((n0, n1)) => {
                self.decode_b_direct_n(mb_x, mb_y, 0, 0, 16, 16, &mut pred_y, &mut c_pred, n0, n1)
            }
            None => self.decode_b_direct(mb_x, mb_y, 0, 0, 16, 16, &mut pred_y, &mut c_pred),
        }
        if let Some(regions) = self.edc_regions.take() {
            self.clear_mb_nnz(mb_x, mb_y, w4);
            self.edc_giveback();
            self.edc_send_job(EdcJob::BSkip {
                mbx: mb_x,
                mby: mb_y,
                regions,
            });
            return Ok(());
        }
        let mut d = (mb_y * 16) * self.cw + mb_x * 16;
        for dy in 0..16 {
            self.rec_y[d..d + 16].copy_from_slice(&pred_y[dy * 16..dy * 16 + 16]);
            d += self.cw;
        }
        let (ccw, cx0) = (self.ccw, mb_x * 8);
        for c in 0..2 {
            let plane = if c == 0 {
                &mut self.rec_u
            } else {
                &mut self.rec_v
            };
            let mut d = (mb_y * 8) * ccw + cx0;
            for dy in 0..8 {
                plane[d..d + 8].copy_from_slice(&c_pred[c][dy * 8..dy * 8 + 8]);
                d += ccw;
            }
        }
        self.clear_mb_nnz(mb_x, mb_y, w4);
        Ok(())
    }
    fn decode_b_skip(&mut self, mb_x: usize, mb_y: usize) -> Result<(), MbError> {
        let _sg = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::BSkipCold);
        self.route_skip_mbs += 1;
        self.wait_refs_for_mb(mb_y);
        let (nref0, nref1) = (self.refs.len(), self.refs1.len());
        if nref0 == 0 || nref1 == 0 {
            return Err(MbError::Unsupported("B without references"));
        }
        let (lref0, lref1) = (nref0 - 1, nref1 - 1);
        let mbw = self.mb_w;
        let addr = mb_y * mbw + mb_x;
        let w4 = mbw * 4;
        if self.direct_spatial && self.edc_tx.is_none() && !no_bskipfast() {
            let patch = match self.bzspan {
                Some((r, x0, n, k)) if r == mb_y && x0 + n == mb_x => Some(k),
                _ => None,
            };
            if patch.is_none() {
                self.bz_flush();
            }
            let (mut n0, mut n1) = self.b_direct_nbrs(mb_x, mb_y);
            if let Some(k) = patch {
                let (r0, r1, m0, m1): (i32, i32, (i32, i32), (i32, i32)) = match k {
                    BzKind::ZeroBi => (0, 0, (0, 0), (0, 0)),
                    BzKind::ZeroUni(0, ri) => (ri as i32, -1, (0, 0), (0, 0)),
                    BzKind::ZeroUni(_, ri) => (-1, ri as i32, (0, 0), (0, 0)),
                    BzKind::Fp { r0, r1, m0, m1 } => {
                        let mm0 = if r0 >= 0 {
                            (m0.0 as i32, m0.1 as i32)
                        } else {
                            (0, 0)
                        };
                        let mm1 = if r1 >= 0 {
                            (m1.0 as i32, m1.1 as i32)
                        } else {
                            (0, 0)
                        };
                        (r0 as i32, r1 as i32, mm0, mm1)
                    }
                };
                n0[0] = MvNeighbor {
                    available: true,
                    mv: m0,
                    ref_idx: r0,
                };
                n1[0] = MvNeighbor {
                    available: true,
                    mv: m1,
                    ref_idx: r1,
                };
            }
            let (refi0, refi1, mv0, mv1, _dz) = Self::b_direct_refs_mvs(&n0, &n1);
            let (a0, a1) = (refi0 >= 0, refi1 >= 0);
            let fast = if a0 && a1 && mv0 == (0, 0) && mv1 == (0, 0) {
                let r0 = (refi0 as usize).min(lref0);
                let r1 = (refi1 as usize).min(lref1);
                let wgt = if r0 == 0 && r1 == 0 {
                    self.iw00()
                } else {
                    self.implicit_weights(r0 as i32, r1 as i32)
                };
                if wgt.is_none() || wgt == Some((32, 32)) {
                    if refi0 == 0 && refi1 == 0 {
                        if let Some(b) = self.bzero.get_mut(addr) {
                            *b = true;
                        }
                        self.bz_push(mb_x, mb_y, BzKind::ZeroBi);
                        return Ok(());
                    }
                    self.recon_b_skip_zero_bi(mb_x, mb_y, r0, r1);
                    true
                } else {
                    false
                }
            } else if a0 != a1 && (if a0 { mv0 } else { mv1 }) == (0, 0) {
                let (list, ri) = if a0 {
                    (0u8, (refi0 as usize).min(lref0) as u8)
                } else {
                    (1u8, (refi1 as usize).min(lref1) as u8)
                };
                self.bz_push(mb_x, mb_y, BzKind::ZeroUni(list, ri));
                return Ok(());
            } else if (!a0 || (mv0.0 % 4 == 0 && mv0.1 % 4 == 0))
                && (!a1 || (mv1.0 % 4 == 0 && mv1.1 % 4 == 0))
                && self.direct_8x8_inference
            {
                let mut cz_ok = true;
                let mut czv = false;
                let cz_need = refi0 == 0 || refi1 == 0;
                if cz_need {
                    let (bx4, by4) = (mb_x * 4, mb_y * 4);
                    for (k, &(ox, oy)) in [(0usize, 0usize), (2, 0), (0, 2), (2, 2)]
                        .iter()
                        .enumerate()
                    {
                        let (colx, coly) = self.col_block(ox, oy);
                        let cz = self.col_zero(bx4 + colx, by4 + coly);
                        if k == 0 {
                            czv = cz;
                        } else if cz != czv {
                            cz_ok = false;
                            break;
                        }
                    }
                }
                if cz_ok {
                    let m0 = if refi0 == 0 && czv { (0, 0) } else { mv0 };
                    let m1 = if refi1 == 0 && czv { (0, 0) } else { mv1 };
                    let r0c = (refi0 as usize).min(lref0);
                    let r1c = (refi1 as usize).min(lref1);
                    let (or0, or1) = (a0.then_some(r0c), a1.then_some(r1c));
                    let wgt_ok = !(a0 && a1) || {
                        let wgt = self.implicit_weights(r0c as i32, r1c as i32);
                        wgt.is_none() || wgt == Some((32, 32))
                    };
                    if wgt_ok && self.bz_fp_valid(mb_x, mb_y, or0, or1, m0, m1) {
                        let kind = BzKind::Fp {
                            r0: if a0 { r0c as i8 } else { -1 },
                            r1: if a1 { r1c as i8 } else { -1 },
                            m0: (m0.0 as i16, m0.1 as i16),
                            m1: (m1.0 as i16, m1.1 as i16),
                        };
                        self.bz_push(mb_x, mb_y, kind);
                        return Ok(());
                    }
                    let ok = wgt_ok && self.recon_b_skip_fp(mb_x, mb_y, or0, or1, m0, m1);
                    if ok {
                        self.b_set_motion(mb_x, mb_y, 0, 0, 16, 16, refi0, m0, refi1, m1);
                        self.clear_mb_nnz(mb_x, mb_y, w4);
                        return Ok(());
                    }
                }
                false
            } else {
                false
            };
            if fast {
                self.b_set_motion(mb_x, mb_y, 0, 0, 16, 16, refi0, (0, 0), refi1, (0, 0));
                self.clear_mb_nnz(mb_x, mb_y, w4);
                return Ok(());
            }
            return self.b_skip_slow(mb_x, mb_y, Some((n0, n1)), w4);
        } else {
            if self.edc_tx.is_some() {
                self.edc_regions = Some(Vec::with_capacity(4));
            }
            return self.b_skip_slow(mb_x, mb_y, None, w4);
        }
    }
    fn decode_b_mb(&mut self, r: &mut BitReader, mb_x: usize, mb_y: usize) -> Result<(), MbError> {
        self.span_flush();
        self.wait_refs_for_mb(mb_y);
        let mb_type = r.read_ue()?;
        if mb_type >= 23 {
            return self.decode_intra_mb(r, mb_x, mb_y, mb_type - 23);
        }
        if self.refs.is_empty() || self.refs1.is_empty() {
            return Err(MbError::Unsupported("B without references"));
        }
        let mut pred_y = [0u8; 256];
        let mut c_pred = [[0u8; 64]; 2];
        if mb_type == 0 {
            let addr_f = mb_y * self.mb_w + mb_x;
            let forced = self.direct_spatial
                && self.edc_tx.is_none()
                && mb_x > 0
                && mb_y > 0
                && mb_x + 1 < self.mb_w
                && addr_f - self.mb_w >= self.slice_first_mb
                && self.bzero.get(addr_f - 1).copied().unwrap_or(false)
                && self
                    .bzero
                    .get(addr_f.wrapping_sub(self.mb_w))
                    .copied()
                    .unwrap_or(false)
                && self
                    .bzero
                    .get(addr_f.wrapping_sub(self.mb_w) + 1)
                    .copied()
                    .unwrap_or(false);
            if forced {
                self.b_direct_region(
                    mb_x,
                    mb_y,
                    0,
                    0,
                    16,
                    16,
                    &mut pred_y,
                    &mut c_pred,
                    (0, 0, (0, 0), (0, 0), false),
                );
                if let Some(b) = self.bzero.get_mut(addr_f) {
                    *b = true;
                }
            } else {
                self.decode_b_direct(mb_x, mb_y, 0, 0, 16, 16, &mut pred_y, &mut c_pred);
            }
            return self.inter_finish(
                r,
                mb_x,
                mb_y,
                &pred_y,
                &c_pred,
                self.direct_8x8_inference,
                false,
            );
        }
        if mb_type == 22 {
            return self.decode_b_8x8(r, mb_x, mb_y);
        }
        let (layout, mvmode, preds) = b_inter_layout(mb_type);
        let mut refi = [[-1i32; 2]; 2];
        for (p, &(_, _, _, _)) in layout.iter().enumerate() {
            if preds[p & 1].uses(0) {
                refi[p][0] = self.read_b_ref(r, 0)?;
            }
        }
        for (p, _) in layout.iter().enumerate() {
            if preds[p & 1].uses(1) {
                refi[p][1] = self.read_b_ref(r, 1)?;
            }
        }
        let mut mvd = [[(0i32, 0i32); 2]; 2];
        for (p, _) in layout.iter().enumerate() {
            if preds[p & 1].uses(0) {
                mvd[p][0] = (read_mvd(r)?, read_mvd(r)?);
            }
        }
        for (p, _) in layout.iter().enumerate() {
            if preds[p & 1].uses(1) {
                mvd[p][1] = (read_mvd(r)?, read_mvd(r)?);
            }
        }
        for (p, &(rx, ry, rw, rh)) in layout.iter().enumerate() {
            let (pbx, pby) = ((mb_x * 4 + rx / 4) as isize, (mb_y * 4 + ry / 4) as isize);
            let pwb = (rw / 4) as isize;
            let mut mv = [(0i32, 0i32); 2];
            for list in 0..2 {
                if refi[p][list] >= 0 {
                    let n = self.mv_neighbors_list(pbx, pby, pwb, list);
                    let pmv = predict_partition_mv(mvmode, p, n[0], n[1], n[2], refi[p][list]);
                    mv[list] = (pmv.0 + mvd[p][list].0, pmv.1 + mvd[p][list].1);
                }
            }
            self.b_set_motion(
                mb_x,
                mb_y,
                rx,
                ry,
                rw,
                rh,
                refi[p & 3][0],
                mv[0],
                refi[p & 3][1],
                mv[1],
            );
            self.b_mc_or_record(
                mb_x,
                mb_y,
                rx,
                ry,
                rw,
                rh,
                refi[p & 3][0],
                mv[0],
                refi[p & 3][1],
                mv[1],
                &mut pred_y,
                &mut c_pred,
            );
        }
        self.inter_finish(r, mb_x, mb_y, &pred_y, &c_pred, true, false)
    }
    fn decode_b_8x8(&mut self, r: &mut BitReader, mb_x: usize, mb_y: usize) -> Result<(), MbError> {
        let mut sub = [0u32; 4];
        for s in sub.iter_mut() {
            let v = r.read_ue()?;
            if v > 12 {
                return Err(MbError::Unsupported("invalid B sub_mb_type"));
            }
            *s = v;
        }
        let mut pred_y = [0u8; 256];
        let mut c_pred = [[0u8; 64]; 2];
        let mut refi = [[-1i32; 2]; 4];
        for (p, &st) in sub.iter().enumerate() {
            if st != 0 && b_sub_uses(st, 0) {
                refi[p][0] = self.read_b_ref(r, 0)?;
            }
        }
        for (p, &st) in sub.iter().enumerate() {
            if st != 0 && b_sub_uses(st, 1) {
                refi[p][1] = self.read_b_ref(r, 1)?;
            }
        }
        const MAX_MVD: usize = 16;
        let mut mvd0 = [(0i32, 0i32); MAX_MVD];
        let mut mvd1 = [(0i32, 0i32); MAX_MVD];
        let (mut n0, mut n1) = (0usize, 0usize);
        for &st in &sub {
            if st != 0 && b_sub_uses(st, 0) {
                for _ in b_sub_parts(st) {
                    mvd0[n0] = (read_mvd(r)?, read_mvd(r)?);
                    n0 += 1;
                }
            }
        }
        for &st in &sub {
            if st != 0 && b_sub_uses(st, 1) {
                for _ in b_sub_parts(st) {
                    mvd1[n1] = (read_mvd(r)?, read_mvd(r)?);
                    n1 += 1;
                }
            }
        }
        let hoisted = if self.direct_spatial && direct_memo_on() && sub.iter().any(|&t| t == 0) {
            Some(self.b_direct_nbrs(mb_x, mb_y))
        } else {
            None
        };
        let (mut i0, mut i1) = (0usize, 0usize);
        for (p, &st) in sub.iter().enumerate() {
            let (b8x, b8y) = ((p % 2) * 8, (p / 2) * 8);
            if st == 0 {
                match hoisted {
                    Some((n0, n1)) => self.decode_b_direct_n(
                        mb_x,
                        mb_y,
                        b8x,
                        b8y,
                        8,
                        8,
                        &mut pred_y,
                        &mut c_pred,
                        n0,
                        n1,
                    ),
                    None => {
                        self.decode_b_direct(mb_x, mb_y, b8x, b8y, 8, 8, &mut pred_y, &mut c_pred)
                    }
                }
                continue;
            }
            for &(sx, sy, sw, sh) in b_sub_parts(st) {
                let (px, py) = (b8x + sx, b8y + sy);
                let (pbx, pby) = ((mb_x * 4 + px / 4) as isize, (mb_y * 4 + py / 4) as isize);
                let pwb = (sw / 4) as isize;
                let mut mv = [(0i32, 0i32); 2];
                if b_sub_uses(st, 0) {
                    let n = self.mv_neighbors_list(pbx, pby, pwb, 0);
                    let pmv = predict_mv(n[0], n[1], n[2], refi[p & 3][0]);
                    let d = mvd0[i0 & 15];
                    i0 += 1;
                    mv[0] = (pmv.0 + d.0, pmv.1 + d.1);
                }
                if b_sub_uses(st, 1) {
                    let n = self.mv_neighbors_list(pbx, pby, pwb, 1);
                    let pmv = predict_mv(n[0], n[1], n[2], refi[p & 3][1]);
                    let d = mvd1[i1 & 15];
                    i1 += 1;
                    mv[1] = (pmv.0 + d.0, pmv.1 + d.1);
                }
                self.b_set_motion(
                    mb_x,
                    mb_y,
                    px,
                    py,
                    sw,
                    sh,
                    refi[p & 3][0],
                    mv[0],
                    refi[p & 3][1],
                    mv[1],
                );
                self.b_mc_or_record(
                    mb_x,
                    mb_y,
                    px,
                    py,
                    sw,
                    sh,
                    refi[p & 3][0],
                    mv[0],
                    refi[p & 3][1],
                    mv[1],
                    &mut pred_y,
                    &mut c_pred,
                );
            }
        }
        let allow_8x8 = sub.iter().all(|&st| {
            if st == 0 {
                self.direct_8x8_inference
            } else {
                st <= 3
            }
        });
        self.inter_finish(r, mb_x, mb_y, &pred_y, &c_pred, allow_8x8, false)
    }
    fn decode_p8x8(
        &mut self,
        r: &mut BitReader,
        mb_x: usize,
        mb_y: usize,
        ref0: bool,
    ) -> Result<(), MbError> {
        if self.refs.is_empty() {
            return Err(MbError::Unsupported("inter without reference"));
        }
        let w4 = self.mb_w * 4;
        let (ch, cch) = (self.mb_h * 16, self.mb_h * 8);
        let num_refs = self.refs.len();
        let mut sub_types = [0u32; 4];
        for st in sub_types.iter_mut() {
            let v = r.read_ue()?;
            if v > 3 {
                return Err(MbError::Unsupported("B-slice / invalid sub_mb_type"));
            }
            *st = v;
        }
        let mut ref_idxs = [0i32; 4];
        if self.num_ref_active > 1 && !ref0 {
            for ri in ref_idxs.iter_mut() {
                *ri = read_ref_idx(r, self.num_ref_active)?;
                if *ri as usize >= num_refs {
                    return Err(MbError::Truncated);
                }
            }
        }
        let defer = self.edc_tx.is_some() || self.edc_active;
        let mut regions: [(usize, usize, usize, usize, i32, (i32, i32)); 16] =
            [(0, 0, 0, 0, 0, (0, 0)); 16];
        let mut nreg = 0usize;
        for part in 0..4usize {
            let refi = ref_idxs[part];
            let (b8x, b8y) = ((part % 2) * 8, (part / 2) * 8);
            for &(srx, sry, srw, srh) in sub_mb_partitions(sub_types[part]) {
                let (px, py) = (b8x + srx, b8y + sry);
                let (pbx, pby) = ((mb_x * 4 + px / 4) as isize, (mb_y * 4 + py / 4) as isize);
                let [a, b, c] = self.mv_neighbors_block(pbx, pby, (srw / 4) as isize);
                let pmv = predict_mv(a, b, c, refi);
                let mvd_x = read_mvd(r)?;
                let mvd_y = read_mvd(r)?;
                let mv = (pmv.0 + mvd_x, pmv.1 + mvd_y);
                for by in py / 4..py / 4 + srh / 4 {
                    for bx in px / 4..px / 4 + srw / 4 {
                        let idx = (mb_y * 4 + by) * w4 + (mb_x * 4 + bx);
                        if let (Some(m), Some(it), Some(rf), Some(cd)) = (
                            self.mv_y.get_mut(idx),
                            self.inter_y.get_mut(idx),
                            self.ref_idx_y.get_mut(idx),
                            self.coded_y.get_mut(idx),
                        ) {
                            (*m, *it, *rf, *cd) = (mv, true, refi, true);
                        }
                    }
                }
                regions[nreg & 15] = (px, py, srw, srh, refi, mv);
                nreg += 1;
            }
        }
        let mut pred_y = [0u8; 256];
        let mut c_pred = [[0u8; 64]; 2];
        if !defer {
            for &(px, py, srw, srh, refi, mv) in &regions[..nreg] {
                let Some(reference) = self.refs.get(refi as usize) else {
                    continue;
                };
                let mut tmp = [0u8; 256];
                mc_luma_padded(
                    &*reference.luma_guard(reference.ch),
                    reference.lstride(),
                    crate::h264::decoder::LPAD,
                    self.cw,
                    ch,
                    mb_x * 16 + px,
                    mb_y * 16 + py,
                    srw,
                    srh,
                    mv.0,
                    mv.1,
                    &mut tmp,
                );
                restride(&mut pred_y, 16, px, py, &tmp, srw, srh);
                let (crx, cry, crw, crh) = (px / 2, py / 2, srw / 2, srh / 2);
                for cc in 0..2 {
                    let rc = if cc == 0 {
                        &*reference.chroma_guard(0, reference.ch)
                    } else {
                        &*reference.chroma_guard(1, reference.ch)
                    };
                    let mut tc = [0u8; 64];
                    mc_chroma_padded(
                        rc,
                        reference.cstride(),
                        crate::h264::decoder::CPAD,
                        self.ccw,
                        cch,
                        mb_x * 8 + crx,
                        mb_y * 8 + cry,
                        crw,
                        crh,
                        mv.0,
                        mv.1,
                        &mut tc,
                    );
                    restride(&mut c_pred[cc], 8, crx, cry, &tc, crw, crh);
                }
                self.weight_partition(&mut pred_y, &mut c_pred, 0, refi as usize, px, py, srw, srh);
            }
        }
        let allow_8x8 = sub_types.iter().all(|&t| t == 0);
        self.inter_finish(r, mb_x, mb_y, &pred_y, &c_pred, allow_8x8, defer)
    }
    fn b_mc_or_record(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        px: usize,
        py: usize,
        rw: usize,
        rh: usize,
        refi0: i32,
        mv0: (i32, i32),
        refi1: i32,
        mv1: (i32, i32),
        pred_y: &mut [u8; 256],
        c_pred: &mut [[u8; 64]; 2],
    ) {
        if self.edc_regions.is_some() {
            let cr0 = if refi0 >= 0 {
                (refi0 as usize).min(self.refs.len().saturating_sub(1)) as i32
            } else {
                -1
            };
            let cr1 = if refi1 >= 0 {
                (refi1 as usize).min(self.refs1.len().saturating_sub(1)) as i32
            } else {
                -1
            };
            let w = if (cr0 >= 0 && self.refs.is_empty()) || (cr1 >= 0 && self.refs1.is_empty()) {
                None
            } else {
                self.implicit_weights(cr0, cr1)
            };
            if let Some(regions) = self.edc_regions.as_mut() {
                regions.push(BRegion {
                    px,
                    py,
                    rw,
                    rh,
                    refi0,
                    refi1,
                    mv0,
                    mv1,
                    w,
                });
            }
            return;
        }
        self.b_mc(
            mb_x, mb_y, px, py, rw, rh, refi0, mv0, refi1, mv1, pred_y, c_pred,
        );
    }
    fn edc_take_ctx(&mut self) -> PixelCtx {
        PixelCtx {
            rec_y: core::mem::take(&mut self.rec_y),
            rec_u: core::mem::take(&mut self.rec_u),
            rec_v: core::mem::take(&mut self.rec_v),
            bak_y: core::mem::take(&mut self.bak_y),
            bak_u: core::mem::take(&mut self.bak_u),
            bak_v: core::mem::take(&mut self.bak_v),
            refs: self.refs.clone(),
            refs1: self.refs1.clone(),
            weights: self.weights.clone(),
            weights_l0id: self.weights_l0id,
            scaling: self.scaling,
            scaling8: self.scaling8,
            cw: self.cw,
            ccw: self.ccw,
            mb_w: self.mb_w,
            mb_h: self.mb_h,
            chroma_qp_offset: self.chroma_qp_offset,
            flt_rows: self.flt_rows,
            db_ena: self.db_ena,
            db_oa: self.db_oa,
            db_ob: self.db_ob,
            cur_qp: self.cur_qp,
            qp_grid: self.mb_qp.clone(),
            t8_grid: self.mb_t8x8.clone(),
            bs_store: self.bs_frame.clone(),
            progress: self.progress.clone(),
        }
    }
    fn edc_restore_ctx(&mut self, ctx: PixelCtx) {
        self.rec_y = ctx.rec_y;
        self.rec_u = ctx.rec_u;
        self.rec_v = ctx.rec_v;
        self.bak_y = ctx.bak_y;
        self.bak_u = ctx.bak_u;
        self.bak_v = ctx.bak_v;
        self.flt_rows = ctx.flt_rows;
    }
    #[inline]
    fn edc_send_job(&mut self, job: EdcJob) {
        if !batch_on() {
            self.edc_tx
                .as_ref()
                .unwrap()
                .send(EdcMsg::Job(job))
                .expect("worker alive");
            return;
        }
        self.edc_batch.push(job);
    }
    fn edc_flush_batch(&mut self) {
        if self.edc_batch.is_empty() {
            return;
        }
        let cap = self.edc_batch.capacity().max(self.mb_w);
        let jobs = core::mem::replace(&mut self.edc_batch, Vec::with_capacity(cap));
        self.edc_tx
            .as_ref()
            .unwrap()
            .send(EdcMsg::Batch(jobs))
            .expect("worker alive");
    }
    fn edc_intra_sync(&mut self) {
        if self.edc_tx.is_none() {
            self.edc_flush();
            return;
        }
        if self.edc_parked.is_some() {
            return;
        }
        self.edc_flush_batch();
        self.edc_tx
            .as_ref()
            .unwrap()
            .send(EdcMsg::NeedCtx)
            .expect("worker alive");
        let mut ctx = self
            .edc_ctx_rx
            .as_ref()
            .unwrap()
            .recv()
            .expect("worker ctx");
        self.rec_y = core::mem::take(&mut ctx.rec_y);
        self.rec_u = core::mem::take(&mut ctx.rec_u);
        self.rec_v = core::mem::take(&mut ctx.rec_v);
        self.bak_y = core::mem::take(&mut ctx.bak_y);
        self.bak_u = core::mem::take(&mut ctx.bak_u);
        self.bak_v = core::mem::take(&mut ctx.bak_v);
        self.flt_rows = ctx.flt_rows;
        self.edc_parked = Some(ctx);
    }
    fn edc_giveback(&mut self) {
        if let Some(mut parked) = self.edc_parked.take() {
            parked.rec_y = core::mem::take(&mut self.rec_y);
            parked.rec_u = core::mem::take(&mut self.rec_u);
            parked.rec_v = core::mem::take(&mut self.rec_v);
            parked.bak_y = core::mem::take(&mut self.bak_y);
            parked.bak_u = core::mem::take(&mut self.bak_u);
            parked.bak_v = core::mem::take(&mut self.bak_v);
            parked.flt_rows = self.flt_rows;
            let _ = self.edc_back_tx.as_ref().unwrap().send(parked);
        }
    }
    fn edc_commit_nnz(
        &mut self,
        mbx: usize,
        mby: usize,
        t8: bool,
        nnzs: &[u8; 24],
        cbp_chroma: u32,
    ) {
        let (w4r, w2r) = (self.mb_w * 4, self.mb_w * 2);
        let mut raster = [0u8; 16];
        if t8 {
            for b8 in 0..4usize {
                let (b8x, b8y) = (b8 % 2, b8 / 2);
                let n = nnzs[b8 * 4];
                for sy in 0..2 {
                    for sx in 0..2 {
                        raster[(b8y * 2 + sy) * 4 + b8x * 2 + sx] = n;
                    }
                }
            }
        } else {
            for (blk, &(lbx, lby)) in LUMA_4X4_SCAN_XY.iter().enumerate() {
                raster[lby * 4 + lbx] = nnzs[blk];
            }
        }
        for by in 0..4usize {
            let a = (mby * 4 + by) * w4r + mbx * 4;
            self.nnz_y[a..a + 4].copy_from_slice(&raster[by * 4..by * 4 + 4]);
        }
        if cbp_chroma == 2 {
            for c in 0..2usize {
                let mut cr = [0u8; 4];
                for &(bx, by) in &CHROMA_4X4_SCAN_XY {
                    cr[by * 2 + bx] = nnzs[(16 + c * 4 + by * 2 + bx).min(23)];
                }
                for by in 0..2usize {
                    let a = (mby * 2 + by) * w2r + mbx * 2;
                    self.nnz_c[c][a..a + 2].copy_from_slice(&cr[by * 2..by * 2 + 2]);
                }
            }
        }
    }
    #[inline]
    fn take_pinter_job(&mut self) -> Box<PInterJob> {
        self.edc_job_pool
            .pop()
            .unwrap_or_else(|| Box::new(PInterJob::blank()))
    }
    #[inline]
    fn take_nores_job(&mut self, pj: PInterNoResJob) -> Box<PInterNoResJob> {
        match self.edc_nores_pool.pop() {
            Some(mut b) => {
                *b = pj;
                b
            }
            None => Box::new(pj),
        }
    }
    #[inline(always)]
    fn edc_flush(&mut self) {
        if self.edc_jobs.is_empty() {
            return;
        }
        self.edc_flush_slow();
    }
    fn edc_flush_slow(&mut self) {
        let mut jobs = core::mem::take(&mut self.edc_jobs);
        let band_ok = self.weights.is_none() || self.weights_id0;
        let mut counted_until = 0usize;
        let mut i = 0usize;
        while i < jobs.len() {
            let j = &jobs[i];
            match j {
                EdcJob::Skip { mbx, mby, mv } => {
                    if *mv == (0, 0) && band_ok && !no_skipband() {
                        let (x0, y0) = (*mbx, *mby);
                        let mut n = 1usize;
                        while let Some(EdcJob::Skip {
                            mbx: nx,
                            mby: ny,
                            mv: nmv,
                        }) = jobs.get(i + n)
                        {
                            if *ny == y0 && *nx == x0 + n && *nmv == (0, 0) {
                                n += 1;
                            } else {
                                break;
                            }
                        }
                        self.recon_p_skip_band(x0, y0, n);
                        if double_recon() {
                            self.recon_p_skip_band(x0, y0, n);
                        }
                        i += n;
                        continue;
                    }
                    if *mv != (0, 0) && i >= counted_until && false {
                        let (x0, y0) = (*mbx, *mby);
                        let mut n2 = 1usize;
                        while let Some(EdcJob::Skip {
                            mbx: nx,
                            mby: ny,
                            mv: nmv,
                        }) = jobs.get(i + n2)
                        {
                            if *ny == y0 && *nx == x0 + n2 && nmv == mv {
                                n2 += 1;
                            } else {
                                break;
                            }
                        }
                        if n2 >= 2 {
                            let fp = mv.0 % 4 == 0 && mv.1 % 4 == 0;
                        }
                        counted_until = i + n2;
                    }
                    self.recon_p_skip(*mbx, *mby, *mv);
                    if double_recon() {
                        self.recon_p_skip(*mbx, *mby, *mv);
                    }
                }
                EdcJob::Inter(job) => {
                    self.recon_p_inter(job);
                    if double_recon() {
                        self.recon_p_inter(job);
                    }
                }
                EdcJob::InterNoRes(job) => {
                    self.recon_p_inter_nores(job);
                    if double_recon() {
                        self.recon_p_inter_nores(job);
                    }
                }
                EdcJob::B(_) | EdcJob::BSkip { .. } => unreachable!("B jobs are worker-only"),
            }
            i += 1;
        }
        for j in jobs.drain(..) {
            match j {
                EdcJob::Inter(b) => self.edc_job_pool.push(b),
                EdcJob::InterNoRes(b) => self.edc_nores_pool.push(b),
                _ => {}
            }
        }
        self.edc_jobs = jobs;
    }
    fn recon_p_inter_parts(
        &mut self,
        mbx: usize,
        mby: usize,
        qp: u8,
        cbp_chroma: u32,
        luma_scan: Option<&[[i32; 16]; 16]>,
        luma8: Option<&[[i32; 64]; 4]>,
        cdc: &[[i32; 4]; 2],
        cac: Option<&[[[i32; 16]; 4]; 2]>,
        nnzs: &[u8; 24],
    ) {
        let mbw = self.mb_w;
        let saved_qp = self.cur_qp;
        self.cur_qp = qp;
        let w4r = mbw * 4;
        let mut pred_y = [0u8; 256];
        let mut c_pred = [[0u8; 64]; 2];
        {
            let _ms =
                crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecMcStage);
            let (rh16, cch) = (self.mb_h * 16, self.mb_h * 8);
            let mut gmv = [(0i32, 0i32); 16];
            let mut gref = [0usize; 16];
            let _gg = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::MvGrid);
            let nrefs = self.refs.len() - 1;
            for by in 0..4usize {
                let row = (mby * 4 + by) * w4r + mbx * 4;
                gmv[by * 4..by * 4 + 4].copy_from_slice(&self.mv_y[row..row + 4]);
                let ridx = &self.ref_idx_y[row..row + 4];
                for bx in 0..4usize {
                    gref[by * 4 + bx] = (ridx[bx].max(0) as usize).min(nrefs);
                }
            }
            drop(_gg);
            let rect_eq = |x4: usize, y4: usize, w4: usize, h4: usize| -> bool {
                let t = y4 * 4 + x4;
                (0..h4).all(|dy| {
                    (0..w4).all(|dx| {
                        let b = ((y4 + dy) * 4 + (x4 + dx)) & 15;
                        gmv[b] == gmv[t] && gref[b] == gref[t]
                    })
                })
            };
            let refs = &self.refs;
            let (cw, ccw) = (self.cw, self.ccw);
            let mc_rect = |x4: usize,
                           y4: usize,
                           w4: usize,
                           h4: usize,
                           pred_y: &mut [u8; 256],
                           c_pred: &mut [[u8; 64]; 2]| {
                let b = y4 * 4 + x4;
                let (mv, gr) = (gmv[b & 15], gref[b & 15]);
                let Some(reference) = refs.get(gr) else {
                    return;
                };
                let (w, h) = (w4 * 4, h4 * 4);
                if w == 16 {
                    crate::h264::common::inter::with_mc_scratch(|scr| {
                        crate::h264::common::inter::mc_luma_padded_pre(
                            scr,
                            &*reference.luma_guard(reference.ch),
                            reference.lstride(),
                            crate::h264::decoder::LPAD,
                            cw,
                            rh16,
                            mbx * 16,
                            mby * 16 + y4 * 4,
                            w,
                            h,
                            mv.0,
                            mv.1,
                            &mut pred_y[y4 * 64..y4 * 64 + w * h],
                        )
                    });
                } else {
                    let mut t = [0u8; 256];
                    crate::h264::common::inter::with_mc_scratch(|scr| {
                        crate::h264::common::inter::mc_luma_padded_pre(
                            scr,
                            &*reference.luma_guard(reference.ch),
                            reference.lstride(),
                            crate::h264::decoder::LPAD,
                            cw,
                            rh16,
                            mbx * 16 + x4 * 4,
                            mby * 16 + y4 * 4,
                            w,
                            h,
                            mv.0,
                            mv.1,
                            &mut t[..w * h],
                        )
                    });
                    let _pb =
                        crate::h264::common::prof::scope(crate::h264::common::prof::Stage::PredBuf);
                    for dy in 0..h {
                        pred_y[(y4 * 4 + dy) * 16 + x4 * 4..][..w]
                            .copy_from_slice(&t[dy * w..dy * w + w]);
                    }
                }
                let (cw4, ch4) = (w4 * 2, h4 * 2);
                let nc = cw4 * ch4;
                let (gu, gv) = (
                    reference.chroma_guard(0, reference.ch),
                    reference.chroma_guard(1, reference.ch),
                );
                if cw4 == 8 {
                    let [cu, cv] = &mut *c_pred;
                    crate::h264::common::inter::mc_chroma_padded_pair(
                        &gu,
                        &gv,
                        reference.cstride(),
                        crate::h264::decoder::CPAD,
                        ccw,
                        cch,
                        mbx * 8,
                        mby * 8 + y4 * 2,
                        cw4,
                        ch4,
                        mv.0,
                        mv.1,
                        &mut cu[y4 * 16..y4 * 16 + nc],
                        &mut cv[y4 * 16..y4 * 16 + nc],
                    );
                } else {
                    let (mut tu, mut tv) = ([0u8; 64], [0u8; 64]);
                    crate::h264::common::inter::mc_chroma_padded_pair(
                        &gu,
                        &gv,
                        reference.cstride(),
                        crate::h264::decoder::CPAD,
                        ccw,
                        cch,
                        mbx * 8 + x4 * 2,
                        mby * 8 + y4 * 2,
                        cw4,
                        ch4,
                        mv.0,
                        mv.1,
                        &mut tu[..nc],
                        &mut tv[..nc],
                    );
                    let _pb =
                        crate::h264::common::prof::scope(crate::h264::common::prof::Stage::PredBuf);
                    for (cc, tc) in [(0usize, &tu), (1, &tv)] {
                        for dy in 0..ch4 {
                            c_pred[cc][(y4 * 2 + dy) * 8 + x4 * 2..][..cw4]
                                .copy_from_slice(&tc[dy * cw4..dy * cw4 + cw4]);
                        }
                    }
                }
            };
            if rect_eq(0, 0, 4, 4) {
                mc_rect(0, 0, 4, 4, &mut pred_y, &mut c_pred);
            } else if rect_eq(0, 0, 4, 2) && rect_eq(0, 2, 4, 2) {
                mc_rect(0, 0, 4, 2, &mut pred_y, &mut c_pred);
                mc_rect(0, 2, 4, 2, &mut pred_y, &mut c_pred);
            } else if rect_eq(0, 0, 2, 4) && rect_eq(2, 0, 2, 4) {
                mc_rect(0, 0, 2, 4, &mut pred_y, &mut c_pred);
                mc_rect(2, 0, 2, 4, &mut pred_y, &mut c_pred);
            } else {
                for q in 0..4usize {
                    let (qx, qy) = ((q % 2) * 2, (q / 2) * 2);
                    if rect_eq(qx, qy, 2, 2) {
                        mc_rect(qx, qy, 2, 2, &mut pred_y, &mut c_pred);
                    } else if rect_eq(qx, qy, 2, 1) && rect_eq(qx, qy + 1, 2, 1) {
                        mc_rect(qx, qy, 2, 1, &mut pred_y, &mut c_pred);
                        mc_rect(qx, qy + 1, 2, 1, &mut pred_y, &mut c_pred);
                    } else if rect_eq(qx, qy, 1, 2) && rect_eq(qx + 1, qy, 1, 2) {
                        mc_rect(qx, qy, 1, 2, &mut pred_y, &mut c_pred);
                        mc_rect(qx + 1, qy, 1, 2, &mut pred_y, &mut c_pred);
                    } else {
                        for j in 0..4usize {
                            mc_rect(qx + (j % 2), qy + (j / 2), 1, 1, &mut pred_y, &mut c_pred);
                        }
                    }
                }
            }
            if self.weights.is_some() {
                for by in 0..4usize {
                    for bx in 0..4usize {
                        let refi = gref[by * 4 + bx];
                        self.weight_partition(
                            &mut pred_y,
                            &mut c_pred,
                            0,
                            refi,
                            bx * 4,
                            by * 4,
                            4,
                            4,
                        );
                    }
                }
            }
        }
        self.add_inter_residual(
            mbx, mby, &pred_y, &c_pred, luma_scan, luma8, cdc, cac, cbp_chroma, nnzs,
        );
        self.cur_qp = saved_qp;
    }
    fn recon_p_inter(&mut self, j: &PInterJob) {
        self.recon_p_inter_parts(
            j.mbx,
            j.mby,
            j.qp,
            j.cbp_chroma,
            Some(&j.luma_scan),
            j.t8.then_some(&j.luma8),
            &j.cdc,
            Some(&j.cac),
            &j.nnzs,
        );
    }
    fn recon_p_inter_nores(&mut self, j: &PInterNoResJob) {
        let w4r = self.mb_w * 4;
        for by in 0..4usize {
            let r = (j.mby * 4 + by) * w4r + j.mbx * 4;
            self.nnz_y[r..r + 4].fill(0);
            if j.t8 {
                self.coded_y[r..r + 4].fill(true);
            }
        }
        let mut pred_y = [0u8; 256];
        let mut c_pred = [[0u8; 64]; 2];
        let nrefs = self.refs.len() - 1;
        let gref: [usize; 16] = core::array::from_fn(|k| (j.gref[k] as usize).min(nrefs));
        coalesce_p_inter_mc(
            &self.refs,
            self.cw,
            self.ccw,
            self.mb_h,
            j.mbx,
            j.mby,
            &j.gmv,
            &gref,
            &mut pred_y,
            &mut c_pred,
        );
        if self.weights.is_some() && !self.weights_l0id {
            for by in 0..4usize {
                for bx in 0..4usize {
                    let refi = gref[by * 4 + bx];
                    self.weight_partition(&mut pred_y, &mut c_pred, 0, refi, bx * 4, by * 4, 4, 4);
                }
            }
        }
        let (cw, ccw) = (self.cw, self.ccw);
        let ybase = (j.mby * 16) * cw + j.mbx * 16;
        let ywin = &mut self.rec_y[ybase..ybase + 15 * cw + 16];
        for dy in 0..16 {
            ywin[dy * cw..dy * cw + 16].copy_from_slice(&pred_y[dy * 16..dy * 16 + 16]);
        }
        let cbase = (j.mby * 8) * ccw + j.mbx * 8;
        for c in 0..2 {
            let plane = if c == 0 {
                &mut self.rec_u
            } else {
                &mut self.rec_v
            };
            let cwin = &mut plane[cbase..cbase + 7 * ccw + 8];
            for dy in 0..8 {
                cwin[dy * ccw..dy * ccw + 8].copy_from_slice(&c_pred[c][dy * 8..dy * 8 + 8]);
            }
        }
    }
    fn decode_p_skip(&mut self, mb_x: usize, mb_y: usize) -> Result<(), MbError> {
        self.route_skip_mbs += 1;
        self.wait_refs_for_mb(mb_y);
        if self.refs.is_empty() {
            return Err(MbError::Unsupported("P_Skip without reference"));
        }
        let addr = mb_y * self.mb_w + mb_x;
        let mv = if (mb_x == 0 || self.skip_zero_next == addr) && !no_runmv() {
            (0, 0)
        } else {
            self.span_flush();
            self.skip_mv(mb_x, mb_y)
        };
        self.skip_zero_next = if mv == (0, 0) { addr + 1 } else { usize::MAX };
        if mv == (0, 0) {
            let recon = self.edc_tx.is_none() && (self.weights.is_none() || self.weights_id0);
            self.pz_push(mb_x, mb_y, recon);
            if recon {
                return Ok(());
            }
        } else {
            let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::SkipRecon);
            if let Some(p) = self.mb_kind.get_mut(mb_y * self.mb_w + mb_x) {
                *p = crate::h264::common::deblock::MB_KIND_SKIP;
            }
            self.set_mb_mv(mb_x, mb_y, mv, true, 0);
            let w4 = self.mb_w * 4;
            for dy in 0..4 {
                let a = (mb_y * 4 + dy) * w4 + mb_x * 4;
                self.coded_y[a..a + 4].fill(true);
                self.modes_y[a..a + 4].fill(2);
            }
        }
        if self.edc_tx.is_some() {
            self.edc_giveback();
            self.edc_send_job(EdcJob::Skip {
                mbx: mb_x,
                mby: mb_y,
                mv,
            });
            return Ok(());
        }
        if self.edc_active {
            self.edc_jobs.push(EdcJob::Skip {
                mbx: mb_x,
                mby: mb_y,
                mv,
            });
            return Ok(());
        }
        self.recon_p_skip(mb_x, mb_y, mv);
        if double_recon() {
            self.recon_p_skip(mb_x, mb_y, mv);
        }
        Ok(())
    }
    fn recon_p_skip_band(&mut self, mbx0: usize, mby: usize, n: usize) {
        let w = n * 16;
        let Some(rf0) = self.refs.first() else { return };
        let lst = rf0.lstride();
        {
            let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::SkipRecon);
            let ly = rf0.luma_guard(rf0.ch);
            for dy in 0..16 {
                let y = mby * 16 + dy;
                let src =
                    (y + crate::h264::decoder::LPAD) * lst + crate::h264::decoder::LPAD + mbx0 * 16;
                let dst = y * self.cw + mbx0 * 16;
                let (Some(d), Some(sr)) = (self.rec_y.get_mut(dst..dst + w), ly.get(src..src + w))
                else {
                    return;
                };
                d.copy_from_slice(sr);
            }
        }
        let cst = rf0.cstride();
        let wc = n * 8;
        for c in 0..2 {
            let rc = rf0.chroma_guard(c, rf0.ch);
            let plane = if c == 0 {
                &mut self.rec_u
            } else {
                &mut self.rec_v
            };
            for dy in 0..8 {
                let y = mby * 8 + dy;
                let src =
                    (y + crate::h264::decoder::CPAD) * cst + crate::h264::decoder::CPAD + mbx0 * 8;
                let dst = y * self.ccw + mbx0 * 8;
                let (Some(d), Some(sr)) = (plane.get_mut(dst..dst + wc), rc.get(src..src + wc))
                else {
                    return;
                };
                d.copy_from_slice(sr);
            }
        }
    }
    fn recon_b_skip_zero_bi(&mut self, mbx: usize, mby: usize, r0i: usize, r1i: usize) {
        let (Some(rf0), Some(rf1)) = (self.refs.get(r0i), self.refs1.get(r1i)) else {
            return;
        };
        let (l0st, l1st) = (rf0.lstride(), rf1.lstride());
        {
            let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::SkipRecon);
            let (ly0, ly1) = (rf0.luma_guard(rf0.ch), rf1.luma_guard(rf1.ch));
            for dy in 0..16 {
                let y = mby * 16 + dy;
                let s0 =
                    (y + crate::h264::decoder::LPAD) * l0st + crate::h264::decoder::LPAD + mbx * 16;
                let s1 =
                    (y + crate::h264::decoder::LPAD) * l1st + crate::h264::decoder::LPAD + mbx * 16;
                let d = y * self.cw + mbx * 16;
                crate::h264::common::inter::avg_row_into(
                    &ly0[s0..s0 + 16],
                    &ly1[s1..s1 + 16],
                    16,
                    &mut self.rec_y[d..d + 16],
                );
            }
        }
        let (c0st, c1st) = (rf0.cstride(), rf1.cstride());
        for c in 0..2 {
            let rc0 = rf0.chroma_guard(c, rf0.ch);
            let rc1 = rf1.chroma_guard(c, rf1.ch);
            let plane = if c == 0 {
                &mut self.rec_u
            } else {
                &mut self.rec_v
            };
            for dy in 0..8 {
                let y = mby * 8 + dy;
                let s0 =
                    (y + crate::h264::decoder::CPAD) * c0st + crate::h264::decoder::CPAD + mbx * 8;
                let s1 =
                    (y + crate::h264::decoder::CPAD) * c1st + crate::h264::decoder::CPAD + mbx * 8;
                let d = y * self.ccw + mbx * 8;
                crate::h264::common::inter::avg_row_into(
                    &rc0[s0..s0 + 8],
                    &rc1[s1..s1 + 8],
                    8,
                    &mut plane[d..d + 8],
                );
            }
        }
    }
    fn recon_p_skip_fullpel(&mut self, mbx: usize, mby: usize, mv: (i32, i32)) -> bool {
        let (dx, dy) = ((mv.0 / 4) as isize, (mv.1 / 4) as isize);
        let Some(rf0) = self.refs.first() else {
            return false;
        };
        let lst = rf0.lstride();
        let lpad = crate::h264::decoder::LPAD as isize;
        let cx = mbx as isize * 16 + dx + lpad;
        let cy = mby as isize * 16 + dy + lpad;
        {
            let ly = rf0.luma_guard(rf0.ch);
            let lrows = rf0.lrows() as isize;
            if cx < 0 || cx + 16 > lst as isize || cy < 0 || cy + 16 > lrows {
                return false;
            }
            let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::SkipRecon);
            let (cw, sb) = (self.cw, cy as usize * lst + cx as usize);
            let db = (mby * 16) * cw + mbx * 16;
            let sw = &ly[sb..sb + 15 * lst + 16];
            let dw = &mut self.rec_y[db..db + 15 * cw + 16];
            for r in 0..16 {
                dw[r * cw..r * cw + 16].copy_from_slice(&sw[r * lst..r * lst + 16]);
            }
        }
        let cst = rf0.cstride();
        let cpad = crate::h264::decoder::CPAD as isize;
        if mv.0 % 8 == 0 && mv.1 % 8 == 0 {
            let ccx = mbx as isize * 8 + (mv.0 / 8) as isize + cpad;
            let ccy = mby as isize * 8 + (mv.1 / 8) as isize + cpad;
            let crows = rf0.crows() as isize;
            if ccx >= 0 && ccx + 8 <= cst as isize && ccy >= 0 && ccy + 8 <= crows {
                for c in 0..2 {
                    let rc = rf0.chroma_guard(c, rf0.ch);
                    let plane = if c == 0 {
                        &mut self.rec_u
                    } else {
                        &mut self.rec_v
                    };
                    let (ccw, sb) = (self.ccw, ccy as usize * cst + ccx as usize);
                    let db = (mby * 8) * ccw + mbx * 8;
                    let sw = &rc[sb..sb + 7 * cst + 8];
                    let dw = &mut plane[db..db + 7 * ccw + 8];
                    for r in 0..8 {
                        dw[r * ccw..r * ccw + 8].copy_from_slice(&sw[r * cst..r * cst + 8]);
                    }
                }
                return true;
            }
        }
        let cch = self.mb_h * 8;
        for c in 0..2 {
            let mut pc = [0u8; 64];
            let rc = if c == 0 {
                &*rf0.chroma_guard(0, rf0.ch)
            } else {
                &*rf0.chroma_guard(1, rf0.ch)
            };
            mc_chroma_padded(
                rc,
                cst,
                crate::h264::decoder::CPAD,
                self.ccw,
                cch,
                mbx * 8,
                mby * 8,
                8,
                8,
                mv.0,
                mv.1,
                &mut pc,
            );
            let plane = if c == 0 {
                &mut self.rec_u
            } else {
                &mut self.rec_v
            };
            for r in 0..8 {
                let d = (mby * 8 + r) * self.ccw + mbx * 8;
                plane[d..d + 8].copy_from_slice(&pc[r * 8..r * 8 + 8]);
            }
        }
        true
    }
    fn recon_b_skip_fp(
        &mut self,
        mbx: usize,
        mby: usize,
        r0: Option<usize>,
        r1: Option<usize>,
        mv0: (i32, i32),
        mv1: (i32, i32),
    ) -> bool {
        let lpad = crate::h264::decoder::LPAD as isize;
        let win = |rf: &crate::h264::decoder::RefFrame, mv: (i32, i32)| -> Option<(usize, usize)> {
            let lst = rf.lstride();
            let cx = mbx as isize * 16 + (mv.0 / 4) as isize + lpad;
            let cy = mby as isize * 16 + (mv.1 / 4) as isize + lpad;
            let lrows = rf.lrows() as isize;
            if cx < 0 || cx + 16 > lst as isize || cy < 0 || cy + 16 > lrows {
                None
            } else {
                Some((cy as usize * lst + cx as usize, lst))
            }
        };
        let w0 = match r0 {
            Some(i) => match self.refs.get(i).and_then(|rr| win(rr, mv0)) {
                Some(w) => Some(w),
                None => return false,
            },
            None => None,
        };
        let w1 = match r1 {
            Some(i) => match self.refs1.get(i).and_then(|rr| win(rr, mv1)) {
                Some(w) => Some(w),
                None => return false,
            },
            None => None,
        };
        {
            let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::SkipRecon);
            match (w0, w1) {
                (Some((s0, l0)), Some((s1, l1))) => {
                    let Some(rf0) = r0.and_then(|i| self.refs.get(i)) else {
                        return false;
                    };
                    let Some(rf1) = r1.and_then(|i| self.refs1.get(i)) else {
                        return false;
                    };
                    let (ly0, ly1) = (rf0.luma_guard(rf0.ch), rf1.luma_guard(rf1.ch));
                    for r in 0..16 {
                        let d = (mby * 16 + r) * self.cw + mbx * 16;
                        crate::h264::common::inter::avg_row_into(
                            &ly0[s0 + r * l0..],
                            &ly1[s1 + r * l1..],
                            16,
                            &mut self.rec_y[d..d + 16],
                        );
                    }
                }
                (Some((s0, l0)), None) => {
                    let Some(rf0) = r0.and_then(|i| self.refs.get(i)) else {
                        return false;
                    };
                    let ly = rf0.luma_guard(rf0.ch);
                    for r in 0..16 {
                        let d = (mby * 16 + r) * self.cw + mbx * 16;
                        self.rec_y[d..d + 16].copy_from_slice(&ly[s0 + r * l0..s0 + r * l0 + 16]);
                    }
                }
                (None, Some((s1, l1))) => {
                    let Some(rf1) = r1.and_then(|i| self.refs1.get(i)) else {
                        return false;
                    };
                    let ly = rf1.luma_guard(rf1.ch);
                    for r in 0..16 {
                        let d = (mby * 16 + r) * self.cw + mbx * 16;
                        self.rec_y[d..d + 16].copy_from_slice(&ly[s1 + r * l1..s1 + r * l1 + 16]);
                    }
                }
                (None, None) => unreachable!("caller guarantees an active list"),
            }
        }
        let cch = self.mb_h * 8;
        let mut c_pred = [[0u8; 64]; 2];
        let (ri0, ri1) = (r0.map_or(-1, |i| i as i32), r1.map_or(-1, |i| i as i32));
        self.b_mc_chroma(
            mbx,
            mby,
            0,
            0,
            16,
            16,
            ri0,
            mv0,
            ri1,
            mv1,
            &mut c_pred,
            None,
            cch,
        );
        for c in 0..2 {
            let plane = if c == 0 {
                &mut self.rec_u
            } else {
                &mut self.rec_v
            };
            for r in 0..8 {
                let d = (mby * 8 + r) * self.ccw + mbx * 8;
                plane[d..d + 8].copy_from_slice(&c_pred[c][r * 8..r * 8 + 8]);
            }
        }
        true
    }
    fn recon_p_skip(&mut self, mb_x: usize, mb_y: usize, mv: (i32, i32)) {
        if (self.weights.is_none() || self.weights_id0)
            && mv.0 % 4 == 0
            && mv.1 % 4 == 0
            && !no_skipfp()
            && self.recon_p_skip_fullpel(mb_x, mb_y, mv)
        {
            return;
        }
        let (ch, cch) = (self.mb_h * 16, self.mb_h * 8);
        let mut pred = [0u8; 256];
        let Some(rf0) = self.refs.first() else { return };
        mc_luma_padded(
            &*rf0.luma_guard(rf0.ch),
            rf0.lstride(),
            crate::h264::decoder::LPAD,
            self.cw,
            ch,
            mb_x * 16,
            mb_y * 16,
            16,
            16,
            mv.0,
            mv.1,
            &mut pred,
        );
        if let Some(wt) = &self.weights {
            if !self.weights_id0 || no_skipfp() {
                for p in pred.iter_mut() {
                    *p = wt.apply_luma(*p, 0, 0);
                }
            }
        }
        {
            let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::SkipRecon);
            let (cw, base) = (self.cw, (mb_y * 16) * self.cw + mb_x * 16);
            let win = &mut self.rec_y[base..base + 15 * cw + 16];
            for dy in 0..16 {
                win[dy * cw..dy * cw + 16].copy_from_slice(&pred[dy * 16..dy * 16 + 16]);
            }
        }
        let (mut pu, mut pv) = ([0u8; 64], [0u8; 64]);
        {
            let Some(rf0) = self.refs.first() else { return };
            let (gu, gv) = (rf0.chroma_guard(0, rf0.ch), rf0.chroma_guard(1, rf0.ch));
            crate::h264::common::inter::mc_chroma_padded_pair(
                &gu,
                &gv,
                rf0.cstride(),
                crate::h264::decoder::CPAD,
                self.ccw,
                cch,
                mb_x * 8,
                mb_y * 8,
                8,
                8,
                mv.0,
                mv.1,
                &mut pu,
                &mut pv,
            );
        }
        if let Some(wt) = &self.weights {
            if !self.weights_id0 || no_skipfp() {
                for p in pu.iter_mut() {
                    *p = wt.apply_chroma(*p, 0, 0, 0);
                }
                for p in pv.iter_mut() {
                    *p = wt.apply_chroma(*p, 0, 0, 1);
                }
            }
        }
        for (pc, plane) in [(&pu, &mut self.rec_u), (&pv, &mut self.rec_v)] {
            for dy in 0..8 {
                let d = (mb_y * 8 + dy) * self.ccw + mb_x * 8;
                plane[d..d + 8].copy_from_slice(&pc[dy * 8..dy * 8 + 8]);
            }
        }
    }
    #[inline]
    fn predict_i4_mode_fast(
        &self,
        bx: usize,
        by: usize,
        lbx: usize,
        lby: usize,
        top_ok: bool,
        left_ok: bool,
    ) -> u8 {
        if self.constrained_intra {
            return self.predict_i4_mode(bx, by);
        }
        if !((lbx > 0 || left_ok) && (lby > 0 || top_ok)) {
            return 2;
        }
        let w4 = self.mb_w * 4;
        let l = self.modes_y.get(by * w4 + (bx - 1)).copied().unwrap_or(2);
        let t = self.modes_y.get((by - 1) * w4 + bx).copied().unwrap_or(2);
        l.min(t)
    }
    fn predict_i4_mode(&self, bx: usize, by: usize) -> u8 {
        if bx == 0 || by == 0 {
            return 2;
        }
        if !self.nbr_in_slice((bx - 1) / 4, by / 4)
            || !self.nbr_in_slice(bx / 4, (by - 1) / 4)
            || !self.intra_nbr_ok(bx - 1, by)
            || !self.intra_nbr_ok(bx, by - 1)
        {
            return 2;
        }
        let w4 = self.mb_w * 4;
        let l = self.modes_y.get(by * w4 + (bx - 1)).copied().unwrap_or(2);
        let t = self.modes_y.get((by - 1) * w4 + bx).copied().unwrap_or(2);
        l.min(t)
    }
    fn gather_i4(
        &self,
        px: usize,
        py: usize,
        avail_top: bool,
        avail_left: bool,
        bx: usize,
        by: usize,
    ) -> ([u8; 8], [u8; 4], u8) {
        let (cw, w4) = (self.cw, self.mb_w * 4);
        let mut top = [0u8; 8];
        let mut left = [0u8; 4];
        let mut corner = 0;
        let (lbx, lby) = (bx & 3, by & 3);
        if avail_top {
            top[..4].copy_from_slice(self.top_y_row(py, px, 4));
            let tr_avail = if lby > 0 {
                lbx < 3 && (I4_TR_IN_MB >> I4_Z_OF_XY[lby][lbx]) & 1 == 1
            } else {
                bx + 1 < w4
                    && self
                        .coded_y
                        .get((by - 1) * w4 + (bx + 1))
                        .copied()
                        .unwrap_or(false)
                    && self.nbr_in_slice((bx + 1) / 4, (by - 1) / 4)
                    && self.intra_nbr_ok(bx + 1, by - 1)
            };
            if tr_avail {
                top[4..8].copy_from_slice(self.top_y_row(py, px + 4, 4));
            } else {
                let t3 = top[3];
                top[4..8].fill(t3);
            }
        }
        if avail_left {
            let base = py * cw + px - 1;
            for (s, &v) in left.iter_mut().zip(self.rec_y[base..].iter().step_by(cw)) {
                *s = v;
            }
        }
        if avail_top && avail_left && ((lbx > 0 && lby > 0) || self.intra_nbr_ok(bx - 1, by - 1)) {
            corner = self.top_y_px(py, px - 1);
        }
        (top, left, corner)
    }
    fn decode_ipcm(&mut self, r: &mut BitReader, mb_x: usize, mb_y: usize) -> Result<(), MbError> {
        r.align_to_byte()?;
        let (lx, ly) = (mb_x * 16, mb_y * 16);
        for dy in 0..16 {
            let row = &mut self.rec_y[(ly + dy) * self.cw + lx..][..16];
            for px in row.iter_mut() {
                *px = r.read_bits(8)? as u8;
            }
        }
        let (cx, cy) = (mb_x * 8, mb_y * 8);
        let ccw = self.ccw;
        for plane in [&mut self.rec_u, &mut self.rec_v] {
            for dy in 0..8 {
                let row = &mut plane[(cy + dy) * ccw + cx..][..8];
                for px in row.iter_mut() {
                    *px = r.read_bits(8)? as u8;
                }
            }
        }
        let (w4, w2) = (self.mb_w * 4, self.mb_w * 2);
        for ry in 0..4 {
            let base = (mb_y * 4 + ry) * w4 + mb_x * 4;
            self.nnz_y[base..][..4].fill(16);
            self.modes_y[base..][..4].fill(2);
            self.coded_y[base..][..4].fill(true);
            self.inter_y[base..][..4].fill(false);
            self.ref_idx_y[base..][..4].fill(-1);
            self.mv_y[base..][..4].fill((0, 0));
        }
        for c in 0..2 {
            for by in 0..2 {
                self.nnz_c[c][(mb_y * 2 + by) * w2 + mb_x * 2..][..2].fill(16);
            }
        }
        Ok(())
    }
    fn decode_i4x4(&mut self, r: &mut BitReader, mb_x: usize, mb_y: usize) -> Result<(), MbError> {
        let w4 = self.mb_w * 4;
        let mut modes = [2u8; 16];
        for &(lbx, lby) in &LUMA_4X4_SCAN_XY {
            let (bx, by) = (mb_x * 4 + lbx, mb_y * 4 + lby);
            let predicted = self.predict_i4_mode(bx, by);
            let actual = if r.read_bit()? {
                predicted
            } else {
                let rem = r.read_bits(3)? as u8;
                if rem < predicted { rem } else { rem + 1 }
            };
            if let Some(p) = self.modes_y.get_mut(by * w4 + bx) {
                *p = actual;
            }
            modes[(lby & 3) * 4 + (lbx & 3)] = actual;
        }
        let chroma_mode = r.read_ue()? as u8;
        let cbp = read_cbp_intra(r)?;
        let cbp_luma = cbp & 15;
        let cbp_chroma = cbp >> 4;
        if cbp != 0 {
            self.step_qp(r.read_se()?)?;
        }
        let qp = self.cur_qp;
        let top_mb_avail = mb_y > 0
            && self.nbr_in_slice(mb_x, mb_y - 1)
            && self.intra_nbr_ok(mb_x * 4, mb_y * 4 - 1);
        let left_mb_avail = mb_x > 0
            && self.nbr_in_slice(mb_x - 1, mb_y)
            && self.intra_nbr_ok(mb_x * 4 - 1, mb_y * 4);
        self.nnz_cache_load(mb_x, mb_y);
        let mut nnz_raster = [0u8; 16];
        let mut scans = [[0i32; 16]; 16];
        let mut totals = [0u8; 16];
        for (blk, &(lbx, lby)) in LUMA_4X4_SCAN_XY.iter().enumerate() {
            let total = if cbp_luma & (1 << (blk / 4)) != 0 {
                let nc = self.nc_pred(lbx, lby);
                decode_residual_block_into::<16, 16>(r, nc, &mut scans[blk & 15])?
            } else {
                0
            };
            self.nnz_cache_set(lbx, lby, total);
            nnz_raster[(lby & 3) * 4 + (lbx & 3)] = total;
            totals[blk & 15] = total;
        }
        let kinds = self.i4_prepare(&scans, &totals, qp);
        let dq0 = self.dq_const(qp, 0);
        for (blk, &(lbx, lby)) in LUMA_4X4_SCAN_XY.iter().enumerate() {
            let (bx, by) = (mb_x * 4 + lbx, mb_y * 4 + lby);
            let avail_top = lby > 0 || top_mb_avail;
            let avail_left = lbx > 0 || left_mb_avail;
            self.recon_i4_block_res(
                bx,
                by,
                modes[(lby & 3) * 4 + (lbx & 3)],
                avail_top,
                avail_left,
                kinds[blk & 15],
                &scans,
                &dq0,
            );
        }
        for ry in 0..4usize {
            self.coded_y[(mb_y * 4 + ry) * w4 + mb_x * 4..][..4].fill(true);
        }
        for by in 0..4usize {
            let a = (mb_y * 4 + by) * w4 + mb_x * 4;
            self.nnz_y[a..a + 4].copy_from_slice(&nnz_raster[by * 4..by * 4 + 4]);
        }
        self.decode_chroma(r, mb_x, mb_y, cbp_chroma, chroma_mode)
    }
    fn decode_i8x8(&mut self, r: &mut BitReader, mb_x: usize, mb_y: usize) -> Result<(), MbError> {
        let w4 = self.mb_w * 4;
        if let Some(p) = self.mb_t8x8.get_mut(mb_y * self.mb_w + mb_x) {
            *p = true;
        }
        self.any_t8 = true;
        let mut modes8 = [2u8; 4];
        for (b8, mode) in modes8.iter_mut().enumerate() {
            let (b8x, b8y) = (b8 % 2, b8 / 2);
            let (bx, by) = (mb_x * 4 + b8x * 2, mb_y * 4 + b8y * 2);
            let predicted = self.predict_i4_mode(bx, by);
            let actual = if r.read_bit()? {
                predicted
            } else {
                let rem = r.read_bits(3)? as u8;
                if rem < predicted { rem } else { rem + 1 }
            };
            *mode = actual;
            for sy in 0..2 {
                self.modes_y[(by + sy) * w4 + bx..][..2].fill(actual);
            }
        }
        let chroma_mode = r.read_ue()? as u8;
        let cbp = read_cbp_intra(r)?;
        let cbp_luma = cbp & 15;
        let cbp_chroma = cbp >> 4;
        if cbp != 0 {
            self.step_qp(r.read_se()?)?;
        }
        let qp = self.cur_qp;
        let top_mb_avail = mb_y > 0
            && self.nbr_in_slice(mb_x, mb_y - 1)
            && self.intra_nbr_ok(mb_x * 4, mb_y * 4 - 1);
        let left_mb_avail = mb_x > 0
            && self.nbr_in_slice(mb_x - 1, mb_y)
            && self.intra_nbr_ok(mb_x * 4 - 1, mb_y * 4);
        self.nnz_cache_load(mb_x, mb_y);
        let mut nnz_raster = [0u8; 16];
        for b8 in 0..4 {
            let (b8x, b8y) = (b8 % 2, b8 / 2);
            let (bx, by) = (mb_x * 4 + b8x * 2, mb_y * 4 + b8y * 2);
            let mut scan8 = [0i32; 64];
            let coded8 = cbp_luma & (1 << b8) != 0;
            if coded8 {
                for sub in 0..4 {
                    let (sx, sy) = (sub % 2, sub / 2);
                    let (cx, cy) = (b8x * 2 + sx, b8y * 2 + sy);
                    let nc = self.nc_pred(cx, cy);
                    let mut blk = [0i32; 16];
                    let total = decode_residual_block_into::<16, 16>(r, nc, &mut blk)?;
                    self.nnz_cache_set(cx, cy, total);
                    nnz_raster[(cy & 3) * 4 + (cx & 3)] = total;
                    for k in 0..16 {
                        scan8[4 * k + sub] = blk[k];
                    }
                }
            } else {
                for sub in 0..4 {
                    let (sx, sy) = (sub % 2, sub / 2);
                    self.nnz_cache_set(b8x * 2 + sx, b8y * 2 + sy, 0);
                }
            }
            let avail_top = b8y > 0 || top_mb_avail;
            let avail_left = b8x > 0 || left_mb_avail;
            self.recon_i8_block(
                bx,
                by,
                modes8[b8],
                avail_top,
                avail_left,
                coded8.then_some(&scan8),
                qp,
            );
            for sy in 0..2 {
                self.coded_y[(by + sy) * w4 + bx..][..2].fill(true);
            }
        }
        for ry in 0..4usize {
            let a = (mb_y * 4 + ry) * w4 + mb_x * 4;
            self.nnz_y[a..a + 4].copy_from_slice(&nnz_raster[ry * 4..ry * 4 + 4]);
        }
        self.decode_chroma(r, mb_x, mb_y, cbp_chroma, chroma_mode)
    }
    fn inv_quant8(&self, raster: &[i32; 64], qp: u8, list: usize) -> [i32; 64] {
        match &self.scaling8 {
            Some(s) => inverse_quant_8x8_dq(raster, &Dequant8Qp::build(qp, &s[list])),
            None => inverse_quant_8x8_dq(raster, &DQ8_FLAT[(qp as usize).min(51)]),
        }
    }
    fn gather_i8(
        &self,
        px: usize,
        py: usize,
        avail_top: bool,
        avail_left: bool,
        bx: usize,
        by: usize,
    ) -> ([u8; 16], [u8; 8], u8, bool) {
        let (cw, w4) = (self.cw, self.mb_w * 4);
        let mut top = [0u8; 16];
        let mut left = [0u8; 8];
        let mut corner = 0;
        if avail_top {
            top[..8].copy_from_slice(self.top_y_row(py, px, 8));
            let tr_avail = bx + 2 < w4
                && self
                    .coded_y
                    .get((by - 1) * w4 + (bx + 2))
                    .copied()
                    .unwrap_or(false)
                && self.nbr_in_slice((bx + 2) / 4, (by - 1) / 4)
                && self.intra_nbr_ok(bx + 2, by - 1);
            if tr_avail {
                top[8..16].copy_from_slice(self.top_y_row(py, px + 8, 8));
            } else {
                let t7 = top[7];
                top[8..16].fill(t7);
            }
        }
        if avail_left {
            let base = py * cw + px - 1;
            for (s, &v) in left.iter_mut().zip(self.rec_y[base..].iter().step_by(cw)) {
                *s = v;
            }
        }
        let avail_corner = avail_top && avail_left && self.intra_nbr_ok(bx - 1, by - 1);
        if avail_corner {
            corner = self.top_y_px(py, px - 1);
        }
        (top, left, corner, avail_corner)
    }
    fn decode_i16(
        &mut self,
        r: &mut BitReader,
        mb_x: usize,
        mb_y: usize,
        mt: u32,
    ) -> Result<(), MbError> {
        let pred_mode = I16Mode::from_id(mt % 4);
        let cbp_chroma = (mt % 12) / 4;
        let cbp_luma_15 = mt / 12 == 1;
        let chroma_mode = r.read_ue()? as u8;
        self.step_qp(r.read_se()?)?;
        let qp = self.cur_qp;
        let w4 = self.mb_w * 4;
        self.nnz_cache_load(mb_x, mb_y);
        let nc_dc = self.nc_pred(0, 0);
        let mut dc_scan = [0i32; 16];
        decode_residual_block_into::<16, 16>(r, nc_dc, &mut dc_scan)?;
        let recon_dc =
            inverse_quant_luma_dc_scan(&dc_scan, qp, self.scaling.as_ref().map(|s| s[0][0]));
        let mut nnz_raster = [0u8; 16];
        let mut q_blocks = [[0i32; 16]; 16];
        for &(bx, by) in &LUMA_4X4_SCAN_XY {
            let total = if cbp_luma_15 {
                let nc = self.nc_pred(bx, by);
                let mut ac = [0i32; 16];
                let t = decode_residual_block_into::<15, 16>(r, nc, &mut ac)?;
                if t != 0 {
                    q_blocks[(by & 3) * 4 + (bx & 3)] = ac;
                }
                t
            } else {
                0
            };
            self.nnz_cache_set(bx, by, total);
            nnz_raster[(by & 3) * 4 + (bx & 3)] = total;
        }
        let coded16 = nnz_raster
            .iter()
            .enumerate()
            .fold(0u16, |m, (i, &n)| m | (((n != 0) as u16) << i));
        for by in 0..4usize {
            let a = (mb_y * 4 + by) * w4 + mb_x * 4;
            self.nnz_y[a..a + 4].copy_from_slice(&nnz_raster[by * 4..by * 4 + 4]);
        }
        let avail_top = mb_y > 0
            && self.nbr_in_slice(mb_x, mb_y - 1)
            && self.intra_nbr_ok(mb_x * 4, mb_y * 4 - 1);
        let avail_left = mb_x > 0
            && self.nbr_in_slice(mb_x - 1, mb_y)
            && self.intra_nbr_ok(mb_x * 4 - 1, mb_y * 4);
        self.recon_i16_luma(
            mb_x,
            mb_y,
            pred_mode,
            avail_top,
            avail_left,
            Some(&q_blocks),
            coded16,
            &recon_dc,
            qp,
        );
        for lby in 0..4usize {
            let a = (mb_y * 4 + lby) * w4 + mb_x * 4;
            self.modes_y[a..a + 4].fill(2);
        }
        self.decode_chroma(r, mb_x, mb_y, cbp_chroma, chroma_mode)
    }
    fn decode_chroma(
        &mut self,
        r: &mut BitReader,
        mb_x: usize,
        mb_y: usize,
        cbp_chroma: u32,
        chroma_mode: u8,
    ) -> Result<(), MbError> {
        let qpc = self.chroma_qp_for(self.cur_qp);
        let avail_top = mb_y > 0
            && self.nbr_in_slice(mb_x, mb_y - 1)
            && self.intra_nbr_ok(mb_x * 4, mb_y * 4 - 1);
        let avail_left = mb_x > 0
            && self.nbr_in_slice(mb_x - 1, mb_y)
            && self.intra_nbr_ok(mb_x * 4 - 1, mb_y * 4);
        let mut c_recon_dc = [[0i32; 4]; 2];
        if cbp_chroma != 0 {
            for (c, slot) in c_recon_dc.iter_mut().enumerate() {
                let mut dc = [0i32; 4];
                decode_residual_block_into::<4, 4>(r, -1, &mut dc)?;
                *slot = self.dequant_chroma_dc(&dc, qpc, 1 + c);
            }
        }
        let mut c_q_blocks = [[[0i32; 16]; 4]; 2];
        let mut ccoded = [0u8; 2];
        if cbp_chroma == 2 {
            self.chroma_cache_load(mb_x, mb_y);
            let w2 = self.mb_w * 2;
            for c in 0..2 {
                for &(bx, by) in &CHROMA_4X4_SCAN_XY {
                    let nc = self.chroma_nc_pred(c, bx, by);
                    let mut ac = [0i32; 16];
                    let total = decode_residual_block_into::<15, 16>(r, nc, &mut ac)?;
                    self.chroma_nnz_cache_set(c, bx, by, total);
                    if let Some(n) =
                        self.nnz_c[c & 1].get_mut((mb_y * 2 + by) * w2 + (mb_x * 2 + bx))
                    {
                        *n = total;
                    }
                    if total != 0 {
                        ccoded[c & 1] |= 1u8 << ((by * 2 + bx) & 3);
                        c_q_blocks[c & 1][(by * 2 + bx) & 3] = ac;
                    }
                }
            }
        }
        self.recon_chroma_blocks(
            mb_x,
            mb_y,
            chroma_mode,
            avail_top,
            avail_left,
            &c_q_blocks,
            ccoded,
            &c_recon_dc,
            qpc,
        );
        Ok(())
    }
    fn dump_mb_map(&self) {
        if !dump_mb_on() {
            return;
        }
        let w4 = self.mb_w * 4;
        let mut hist = [0usize; 4];
        eprintln!("--- frame poc {} ---", self.cur_poc);
        for mb_y in 0..self.mb_h {
            let mut row = String::new();
            for mb_x in 0..self.mb_w {
                let b = (mb_y * 4) * w4 + mb_x * 4;
                let Some(&r) = self.ref_idx_y.get(b) else {
                    continue;
                };
                if r < 0 {
                    row.push('i');
                } else {
                    if (r as usize) < 4 {
                        hist[r as usize] += 1;
                    }
                    row.push((b'0' + (r as u8).min(9)) as char);
                }
            }
            eprintln!("{row}");
        }
        eprintln!(
            "ref histogram: {hist:?}   num_ref_active={} refs.len()={}   OUT-OF-RANGE={}",
            self.num_ref_active,
            self.refs.len(),
            hist.iter().skip(self.refs.len()).sum::<usize>()
        );
        let list: Vec<String> = self
            .refs
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let synth = if f.w4 == 0 { " SYNTH-GREY" } else { "" };
                format!("[{i}] poc={} fn={}{synth}", f.poc, f.frame_num)
            })
            .collect();
        eprintln!("  RefPicList0: {}", list.join("  "));
    }
    pub fn deblock(&mut self, offset_a: i32, offset_b: i32) {
        self.span_flush();
        self.edc_flush();
        self.dump_mb_map();
        if rowdb_on() {
            while self.bs_rows < self.mb_h {
                let r = self.bs_rows;
                self.derive_bs_row(r);
                self.bs_rows += 1;
            }
        }
        let rowdb = rowdb_on();
        let nnz_db_storage;
        let nnz_db: &[u8] = if rowdb {
            &[]
        } else if self.mb_t8x8.iter().any(|&t| t) {
            let mut n = self.nnz_y.clone();
            let w4 = self.mb_w * 4;
            for mb_y in 0..self.mb_h {
                for mb_x in 0..self.mb_w {
                    if !self
                        .mb_t8x8
                        .get(mb_y * self.mb_w + mb_x)
                        .copied()
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    for b8 in 0..4 {
                        let (bx, by) = (mb_x * 4 + (b8 % 2) * 2, mb_y * 4 + (b8 / 2) * 2);
                        let any = (0..2).any(|sy| {
                            self.nnz_y[(by + sy) * w4 + bx..][..2]
                                .iter()
                                .any(|&v| v > 0)
                        });
                        for sy in 0..2 {
                            n[(by + sy) * w4 + bx..][..2].fill(u8::from(any));
                        }
                    }
                }
            }
            nnz_db_storage = n;
            &nnz_db_storage
        } else {
            &self.nnz_y
        };
        let mut info = crate::h264::common::deblock::BlockInfo {
            inter: if rowdb { &[] } else { &self.inter_y },
            nnz: nnz_db,
            mv: if rowdb { &[] } else { &self.mv_y },
            ref_id: if rowdb { &[] } else { &self.ref_idx_y },
            mv1: if rowdb { &[] } else { &self.mv1 },
            ref_id1: if rowdb || self.ref_poc1.is_empty() {
                &[]
            } else {
                &self.ref_idx1
            },
            w4: self.mb_w * 4,
            t8x8: &self.mb_t8x8,
            bs: &[],
            poc0: if rowdb { &[] } else { &self.ref_poc0 },
            poc1: if rowdb { &[] } else { &self.ref_poc1 },
            kind: &self.mb_kind,
        };
        let bs_store;
        if rowdb {
            bs_store = core::mem::take(&mut self.bs_frame);
            info.bs = &bs_store;
        } else if bs_pre_on() {
            let mut buf = Vec::new();
            crate::h264::common::deblock::precompute_bs_frame(
                &info, self.mb_w, self.mb_h, &mut buf,
            );
            bs_store = buf;
            info.bs = &bs_store;
        } else {
            bs_store = Vec::new();
        }
        let first_row = if rowdb { self.flt_rows } else { 0 };
        crate::h264::common::deblock::filter_frame_rows_pre(
            &mut self.rec_y,
            &mut self.rec_u,
            &mut self.rec_v,
            self.mb_w,
            self.mb_h,
            first_row..self.mb_h,
            &self.mb_qp,
            self.chroma_qp_offset,
            offset_a,
            offset_b,
            &info,
        );
        drop(info);
        if rowdb {
            self.bs_frame = bs_store;
        }
    }
    pub fn into_frame_recycle(mut self, crop_r: usize, crop_b: usize) -> (YuvFrame, GridPool) {
        let [c0, c1] = core::mem::take(&mut self.nnz_c);
        let pool = GridPool {
            job_pool: core::mem::take(&mut self.edc_job_pool),
            nores_pool: core::mem::take(&mut self.edc_nores_pool),
            bits_per_mb: self.bits_per_mb,
            mb_qp: core::mem::take(&mut self.mb_qp),
            bs_frame: core::mem::take(&mut self.bs_frame),
            pk_prev: core::mem::take(&mut self.pk_prev),
            pk_cur: core::mem::take(&mut self.pk_cur),
            nnz_dbr: core::mem::take(&mut self.nnz_dbr),
            bak_y: core::mem::take(&mut self.bak_y),
            bak_u: core::mem::take(&mut self.bak_u),
            bak_v: core::mem::take(&mut self.bak_v),
            nnz_y: core::mem::take(&mut self.nnz_y),
            nnz_c0: c0,
            nnz_c1: c1,
            modes_y: core::mem::take(&mut self.modes_y),
            coded_y: core::mem::take(&mut self.coded_y),
            mv_y: core::mem::take(&mut self.mv_y),
            inter_y: core::mem::take(&mut self.inter_y),
            ref_idx_y: core::mem::take(&mut self.ref_idx_y),
            mv1: core::mem::take(&mut self.mv1),
            ref_idx1: core::mem::take(&mut self.ref_idx1),
            mb_t8x8: core::mem::take(&mut self.mb_t8x8),
            mb_kind: core::mem::take(&mut self.mb_kind),
            bzero: core::mem::take(&mut self.bzero),
            sc_cat: core::mem::take(&mut self.sc_cat),
            sc_cbp: core::mem::take(&mut self.sc_cbp),
            sc_cmode: core::mem::take(&mut self.sc_cmode),
            sc_nzc: core::mem::take(&mut self.sc_nzc),
            sc_cbfdc: core::mem::take(&mut self.sc_cbfdc),
            sc_skip: core::mem::take(&mut self.sc_skip),
            sc_ref: core::mem::take(&mut self.sc_ref),
            sc_mvd: core::mem::take(&mut self.sc_mvd),
            sc_ref1: core::mem::take(&mut self.sc_ref1),
            sc_mvd1: core::mem::take(&mut self.sc_mvd1),
            sc_direct: core::mem::take(&mut self.sc_direct),
            ref_poc0: core::mem::take(&mut self.ref_poc0),
            ref_poc1: core::mem::take(&mut self.ref_poc1),
        };
        (self.into_frame(crop_r, crop_b), pool)
    }
    pub fn into_frame(self, crop_r: usize, crop_b: usize) -> YuvFrame {
        if crop_r == 0 && crop_b == 0 {
            return YuvFrame {
                width: self.cw,
                height: self.ch,
                y: self.rec_y,
                u: self.rec_u,
                v: self.rec_v,
            };
        }
        let dw = self.cw - 2 * crop_r;
        let dh = self.ch - 2 * crop_b;
        let mut y = vec![0u8; dw * dh];
        for row in 0..dh {
            y[row * dw..row * dw + dw]
                .copy_from_slice(&self.rec_y[row * self.cw..row * self.cw + dw]);
        }
        let (cdw, cdh) = (dw / 2, dh / 2);
        let mut u = vec![0u8; cdw * cdh];
        let mut v = vec![0u8; cdw * cdh];
        for row in 0..cdh {
            u[row * cdw..row * cdw + cdw]
                .copy_from_slice(&self.rec_u[row * self.ccw..row * self.ccw + cdw]);
            v[row * cdw..row * cdw + cdw]
                .copy_from_slice(&self.rec_v[row * self.ccw..row * self.ccw + cdw]);
        }
        let _ = self.cch;
        YuvFrame {
            width: dw,
            height: dh,
            y,
            u,
            v,
        }
    }
}
fn read_mvd(r: &mut BitReader) -> Result<i32, MbError> {
    let v = r.read_se()?;
    if v.unsigned_abs() > (1 << 17) {
        return Err(MbError::Truncated);
    }
    Ok(v)
}
type Eng = crate::h264::decoder::cabac::Engine;
type Ctx = crate::h264::decoder::cabac::Ctx;
fn cabac_unary(e: &mut Eng, data: &[u8], ctx: &mut Ctx, c: usize, off: usize) -> u32 {
    if e.decode_decision(data, ctx, c) == 0 {
        return 0;
    }
    let mut sym = 0;
    loop {
        let bin = e.decode_decision(data, ctx, c + off);
        sym += 1;
        if bin == 0 || sym >= 512 {
            break;
        }
    }
    sym
}
#[inline(never)]
fn cabac_exp_bypass(mut e: Eng, data: &[u8], mut count: i32) -> (u32, Eng) {
    let mut sym = 0u32;
    loop {
        let c = e.decode_bypass(data);
        if c == 1 {
            sym += 1 << count;
            count += 1;
        }
        if c == 0 || count == 16 {
            break;
        }
    }
    let mut sym2 = 0u32;
    while count > 0 {
        count -= 1;
        if e.decode_bypass(data) != 0 {
            sym2 |= 1 << count;
        }
    }
    (sym + sym2, e)
}
#[inline(always)]
fn cabac_ueg_level(e: &mut Eng, data: &[u8], ctx: &mut Ctx, c: usize) -> u32 {
    if e.decode_decision(data, ctx, c) == 0 {
        return 0;
    }
    let mut code = 0u32;
    let mut tmp;
    loop {
        tmp = e.decode_decision(data, ctx, c);
        code += 1;
        if tmp == 0 || code == 12 {
            break;
        }
    }
    if tmp != 0 {
        let (v, e2) = cabac_exp_bypass(*e, data, 0);
        *e = e2;
        code += v + 1;
    }
    code
}
pub fn parse_mb_qp_delta_cabac(
    cab: &mut crate::h264::decoder::cabac::Cabac,
    last_delta_qp: &mut i32,
) -> i32 {
    const O: usize = 60;
    let ctx_inc = (*last_delta_qp != 0) as usize;
    let mut qp_delta = 0;
    let (data, mut e, ctx) = cab.view();
    if e.decode_decision(data, ctx, O + ctx_inc) != 0 {
        let code = cabac_unary(&mut e, data, ctx, O + 2, 1) + 1;
        qp_delta = ((code + 1) >> 1) as i32;
        if code & 1 == 0 {
            qp_delta = -qp_delta;
        }
    }
    cab.commit(e);
    *last_delta_qp = qp_delta;
    qp_delta
}
use crate::h264::common::cabac_tables::{
    NZC_CACHE, RES_CBF, RES_MAP, RES_MAXC2, RES_MAXPOS, RES_ONE,
};
const RP_I16_DC: usize = 1;
const RP_I16_AC: usize = 2;
const RP_LUMA_4X4: usize = 3;
const RP_CHROMA_DC: usize = 7;
const RP_CHROMA_AC: usize = 9;
const RP_LUMA_8X8: usize = 6;
use crate::h264::common::cabac_tables::{LAST8X8, SIG8X8};
#[inline(always)]
fn residual_block_eng<const RP: usize, const N: usize>(
    e: &mut Eng,
    data: &[u8],
    ctx: &mut Ctx,
    nzc: &mut [u8; 48],
    cbf_dc: &mut u16,
    iz: usize,
    plane: usize,
    is_intra: bool,
    ndc: (u16, u16),
    out: &mut [i32; N],
) -> u32 {
    const { assert!(N == 4 || N == 16 || N == 64) };
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Entropy);
    let is8 = RP == RP_LUMA_8X8;
    let is_dc = RP == RP_I16_DC || RP == RP_CHROMA_DC;
    let bit = RP + plane;
    let (mut na, mut nb) = (is_intra as u8, is_intra as u8);
    let scan = NZC_CACHE[iz.min(23)].clamp(8, 47);
    if is_dc {
        nb = ((ndc.0 >> bit) & 1) as u8;
        na = ((ndc.1 >> bit) & 1) as u8;
    } else {
        let (nbc, nac) = (nzc[scan - 8], nzc[scan - 1]);
        if nbc != 0xff {
            nb = (nbc != 0) as u8;
        }
        if nac != 0xff {
            na = (nac != 0) as u8;
        }
    }
    if !is8 {
        let _sg = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::EntCbf);
        let cbf = e.decode_decision(data, ctx, 85 + RES_CBF[RP] + (na + (nb << 1)) as usize);
        if cbf == 0 {
            if !is_dc {
                nzc[scan] = 0;
            }
            return 0;
        }
        if is_dc {
            *cbf_dc |= 1 << bit;
        }
    }
    let maxpos = RES_MAXPOS[RP] as usize;
    debug_assert!(maxpos < N);
    let (map, last) = if is8 {
        (402, 417)
    } else {
        let m = RES_MAP[RP];
        (105 + m, 166 + m)
    };
    let mut sig = 0u64;
    let mut last_hit = false;
    let _sg = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::EntSig);
    for i in 0..maxpos {
        let (mi, li) = if is8 {
            (
                (SIG8X8[i & 63] & 15) as usize,
                (LAST8X8[i & 63] & 15) as usize,
            )
        } else {
            (i, i)
        };
        if e.decode_decision(data, ctx, map + mi) != 0 {
            sig |= 1u64 << i;
            if e.decode_decision(data, ctx, last + li) != 0 {
                last_hit = true;
                break;
            }
        }
    }
    if !last_hit {
        sig |= 1u64 << maxpos;
    }
    let coeff_num = sig.count_ones();
    let one = 227 + RES_ONE[RP];
    let abs = one + 5;
    let maxc2 = RES_MAXC2[RP] as usize;
    let (mut c1, mut c2) = (1usize, 0usize);
    drop(_sg);
    let _lg = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::EntLvl);
    let mut m = sig;
    while m != 0 {
        let i = 63 - m.leading_zeros() as usize;
        m &= !(1u64 << i);
        let mut level = 1 + e.decode_decision(data, ctx, one + c1) as i32;
        if level == 2 {
            level += cabac_ueg_level(e, data, ctx, abs + c2) as i32;
            c2 = (c2 + 1).min(maxc2);
            c1 = 0;
        } else if c1 != 0 {
            c1 = (c1 + 1).min(4);
        }
        if e.decode_bypass_sign() != 0 {
            level = -level;
        }
        out[i & (N - 1)] = level;
    }
    if is8 {
        let cn = coeff_num as u8;
        for k in 0..4 {
            nzc[NZC_CACHE[(iz + k).min(23)].min(47)] = cn;
        }
    } else if !is_dc {
        nzc[scan] = coeff_num as u8;
    }
    coeff_num
}
struct ResidualOut<'a> {
    luma: &'a mut [[i32; 16]; 16],
    luma8: &'a mut [[i32; 64]; 4],
    cdc: &'a mut [[i32; 4]; 2],
    cac: &'a mut [[[i32; 16]; 4]; 2],
    nnzs: &'a mut [u8; 24],
}
#[inline(always)]
fn weight_sample(sample: u8, w: i32, o: i32, denom: i32) -> u8 {
    let v = if denom >= 1 {
        ((sample as i32 * w + (1 << (denom - 1))) >> denom) + o
    } else {
        sample as i32 * w + o
    };
    v.clamp(0, 255) as u8
}
#[inline(always)]
fn weight_rows<const W: usize>(
    plane: &mut [u8],
    stride: usize,
    rx: usize,
    ry: usize,
    rh: usize,
    w: i32,
    o: i32,
    denom: i32,
) {
    for dy in 0..rh {
        let row = &mut plane[(ry + dy) * stride + rx..][..W];
        for s in row.iter_mut() {
            *s = weight_sample(*s, w, o, denom);
        }
    }
}
#[inline]
fn weight_block(
    plane: &mut [u8],
    stride: usize,
    rx: usize,
    ry: usize,
    rw: usize,
    rh: usize,
    w: i32,
    o: i32,
    denom: i32,
) {
    match rw {
        16 => weight_rows::<16>(plane, stride, rx, ry, rh, w, o, denom),
        8 => weight_rows::<8>(plane, stride, rx, ry, rh, w, o, denom),
        4 => weight_rows::<4>(plane, stride, rx, ry, rh, w, o, denom),
        2 => weight_rows::<2>(plane, stride, rx, ry, rh, w, o, denom),
        _ => {
            for dy in 0..rh {
                for dx in 0..rw {
                    let i = (ry + dy) * stride + rx + dx;
                    if let Some(s) = plane.get_mut(i) {
                        *s = weight_sample(*s, w, o, denom);
                    }
                }
            }
        }
    }
}
#[derive(Clone, Copy)]
enum I4Res {
    Zero,
    Flat(i32),
    Idx(u8),
}
const I4_Z_OF_XY: [[usize; 4]; 4] = {
    let mut t = [[0usize; 4]; 4];
    let mut i = 0;
    while i < 16 {
        let (x, y) = LUMA_4X4_SCAN_XY[i];
        t[y][x] = i;
        i += 1;
    }
    t
};
const I4_TR_IN_MB: u16 = {
    let mut m = 0u16;
    let mut i = 0;
    while i < 16 {
        let (x, y) = LUMA_4X4_SCAN_XY[i];
        if y > 0 && x < 3 && I4_Z_OF_XY[y - 1][x + 1] < i {
            m |= 1 << i;
        }
        i += 1;
    }
    m
};
#[inline(always)]
fn resolve_ndc(ndc: (Option<u16>, Option<u16>), is_intra: bool) -> (u16, u16) {
    let d = if is_intra { 0xFFFF } else { 0 };
    (ndc.0.unwrap_or(d), ndc.1.unwrap_or(d))
}
fn parse_mb_residual_cabac<const INTRA: bool>(
    cab: &mut crate::h264::decoder::cabac::Cabac,
    nzc: &mut [u8; 48],
    cbf_dc: &mut u16,
    ndc: (Option<u16>, Option<u16>),
    cbp_luma: u32,
    cbp_chroma: u32,
    t8: bool,
    o: ResidualOut,
) {
    let is_intra = INTRA;
    let nd = resolve_ndc(ndc, INTRA);
    let (data, mut e, ctx) = cab.view();
    for id8 in 0..4usize {
        if cbp_luma & (1 << id8) != 0 {
            if t8 {
                o.luma8[id8] = [0i32; 64];
                let n = residual_block_eng::<RP_LUMA_8X8, 64>(
                    &mut e,
                    data,
                    ctx,
                    nzc,
                    cbf_dc,
                    id8 * 4,
                    0,
                    is_intra,
                    nd,
                    &mut o.luma8[id8],
                ) as u8;
                o.nnzs[id8 * 4..id8 * 4 + 4].fill(n);
            } else {
                o.luma[id8 * 4..id8 * 4 + 4].fill([0i32; 16]);
                for id4 in 0..4usize {
                    let iz = id8 * 4 + id4;
                    o.nnzs[iz] = residual_block_eng::<RP_LUMA_4X4, 16>(
                        &mut e,
                        data,
                        ctx,
                        nzc,
                        cbf_dc,
                        iz,
                        0,
                        is_intra,
                        nd,
                        &mut o.luma[iz],
                    ) as u8;
                }
            }
        } else {
            for k in 0..4 {
                nzc[NZC_CACHE[(id8 * 4 + k).min(23)].min(47)] = 0;
            }
        }
    }
    if cbp_chroma >= 1 {
        *o.cdc = [[0i32; 4]; 2];
        for i in 0..2usize {
            residual_block_eng::<RP_CHROMA_DC, 4>(
                &mut e,
                data,
                ctx,
                nzc,
                cbf_dc,
                16 + i * 4,
                i,
                is_intra,
                nd,
                &mut o.cdc[i],
            );
        }
    }
    if cbp_chroma == 2 {
        *o.cac = [[[0i32; 16]; 4]; 2];
        for i in 0..2usize {
            for id4 in 0..4usize {
                o.nnzs[16 + i * 4 + id4] = residual_block_eng::<RP_CHROMA_AC, 16>(
                    &mut e,
                    data,
                    ctx,
                    nzc,
                    cbf_dc,
                    16 + i * 4 + id4,
                    i,
                    is_intra,
                    nd,
                    &mut o.cac[i][id4],
                ) as u8;
            }
        }
    }
    cab.commit(e);
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum BzKind {
    ZeroBi,
    ZeroUni(u8, u8),
    Fp {
        r0: i8,
        r1: i8,
        m0: (i16, i16),
        m1: (i16, i16),
    },
}
enum EdcMsg {
    Job(EdcJob),
    Batch(Vec<EdcJob>),
    Row {
        r: usize,
        bs: Vec<crate::h264::common::deblock::MbBs>,
        qp: Vec<u8>,
        t8: Vec<bool>,
    },
    NeedCtx,
}
pub(crate) struct PixelCtx {
    rec_y: Vec<u8>,
    rec_u: Vec<u8>,
    rec_v: Vec<u8>,
    bak_y: Vec<u8>,
    bak_u: Vec<u8>,
    bak_v: Vec<u8>,
    refs: Vec<crate::h264::decoder::Ref>,
    refs1: Vec<crate::h264::decoder::Ref>,
    weights: Option<WeightTable>,
    weights_l0id: bool,
    scaling: Option<[[i32; 16]; 6]>,
    scaling8: Option<[[i32; 64]; 2]>,
    cw: usize,
    ccw: usize,
    mb_w: usize,
    mb_h: usize,
    chroma_qp_offset: i32,
    flt_rows: usize,
    db_ena: bool,
    db_oa: i32,
    db_ob: i32,
    cur_qp: u8,
    qp_grid: Vec<u8>,
    t8_grid: Vec<bool>,
    bs_store: Vec<crate::h264::common::deblock::MbBs>,
    progress: Option<crate::h264::decoder::Ref>,
}
impl PixelCtx {
    fn publish_progress_rows(&self) {
        let Some(slot) = &self.progress else {
            return;
        };
        if !self.db_ena || self.flt_rows == 0 {
            return;
        }
        publish_filtered_rows_to_slot(
            slot,
            &self.rec_y,
            &self.rec_u,
            &self.rec_v,
            self.cw,
            self.ccw,
            self.mb_h * 16,
            self.flt_rows,
        );
    }
    fn chroma_qp_for(&self, qp: u8) -> u8 {
        crate::h264::common::predict::chroma_qp(
            ((qp as i32 + self.chroma_qp_offset).clamp(0, 51)) as u8,
        )
    }
    #[inline]
    fn dq_const(&self, qp: u8, list: usize) -> DequantQp {
        match &self.scaling {
            Some(s) => DequantQp::weighted(qp, &s[list]),
            None => DQ_FLAT[(qp as usize).min(51)],
        }
    }
    fn dequant_dc4(&self, level: i32, qp: u8, list: usize) -> i32 {
        crate::h264::common::transform::dequantize_dc4(
            level,
            qp,
            self.scaling.as_ref().map(|sc| sc[list][0]),
        )
    }
    fn inv_quant8(&self, raster: &[i32; 64], qp: u8, list: usize) -> [i32; 64] {
        match &self.scaling8 {
            Some(s) => inverse_quant_8x8_dq(raster, &Dequant8Qp::build(qp, &s[list])),
            None => inverse_quant_8x8_dq(raster, &DQ8_FLAT[(qp as usize).min(51)]),
        }
    }
    fn dequant_chroma_dc(&self, levels: &[i32; 4], qp: u8, list: usize) -> [i32; 4] {
        match &self.scaling {
            Some(sc) => inverse_quant_chroma_dc_weighted(levels, qp, sc[list][0]),
            None => inverse_quant_chroma_dc(levels, qp),
        }
    }
    fn weight_partition(
        &self,
        pred_y: &mut [u8; 256],
        c_pred: &mut [[u8; 64]; 2],
        list: usize,
        refi: usize,
        rx: usize,
        ry: usize,
        rw: usize,
        rh: usize,
    ) {
        let Some(wt) = &self.weights else { return };
        let (lw, lo) = wt.luma_wo(list, refi);
        weight_block(pred_y, 16, rx, ry, rw, rh, lw, lo, wt.luma_log2_denom);
        let (crx, cry, crw, crh) = (rx / 2, ry / 2, rw / 2, rh / 2);
        for cc in 0..2 {
            let (cw, co) = wt.chroma_wo(list, refi, cc);
            weight_block(
                &mut c_pred[cc & 1],
                8,
                crx,
                cry,
                crw,
                crh,
                cw,
                co,
                wt.chroma_log2_denom,
            );
        }
    }
    fn recon_p_inter(&mut self, j: &PInterJob) {
        crate::h264::decoder::RefFrame::set_mc_row_need(j.mby, self.mb_h * 16);
        self.cur_qp = j.qp;
        let mut pred_y = [0u8; 256];
        let mut c_pred = [[0u8; 64]; 2];
        {
            let _ms =
                crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecMcStage);
            let (rh16, cch) = (self.mb_h * 16, self.mb_h * 8);
            let gmv = j.gmv;
            let mut gref = [0usize; 16];
            for k in 0..16 {
                gref[k] = (j.gref[k] as usize).min(self.refs.len() - 1);
            }
            let rect_eq = |x4: usize, y4: usize, w4: usize, h4: usize| -> bool {
                let t = y4 * 4 + x4;
                (0..h4).all(|dy| {
                    (0..w4).all(|dx| {
                        let b = ((y4 + dy) * 4 + (x4 + dx)) & 15;
                        gmv[b] == gmv[t] && gref[b] == gref[t]
                    })
                })
            };
            let refs = &self.refs;
            let (cw, ccw) = (self.cw, self.ccw);
            let mc_rect = |x4: usize,
                           y4: usize,
                           w4: usize,
                           h4: usize,
                           pred_y: &mut [u8; 256],
                           c_pred: &mut [[u8; 64]; 2]| {
                let b = y4 * 4 + x4;
                let (mv, gr) = (gmv[b & 15], gref[b & 15]);
                let Some(reference) = refs.get(gr) else {
                    return;
                };
                let (w, h) = (w4 * 4, h4 * 4);
                if w == 16 {
                    crate::h264::common::inter::with_mc_scratch(|scr| {
                        crate::h264::common::inter::mc_luma_padded_pre(
                            scr,
                            &*reference.luma_guard(reference.ch),
                            reference.lstride(),
                            crate::h264::decoder::LPAD,
                            cw,
                            rh16,
                            j.mbx * 16,
                            j.mby * 16 + y4 * 4,
                            w,
                            h,
                            mv.0,
                            mv.1,
                            &mut pred_y[y4 * 64..y4 * 64 + w * h],
                        )
                    });
                } else {
                    let mut t = [0u8; 256];
                    crate::h264::common::inter::with_mc_scratch(|scr| {
                        crate::h264::common::inter::mc_luma_padded_pre(
                            scr,
                            &*reference.luma_guard(reference.ch),
                            reference.lstride(),
                            crate::h264::decoder::LPAD,
                            cw,
                            rh16,
                            j.mbx * 16 + x4 * 4,
                            j.mby * 16 + y4 * 4,
                            w,
                            h,
                            mv.0,
                            mv.1,
                            &mut t[..w * h],
                        )
                    });
                    let _pb =
                        crate::h264::common::prof::scope(crate::h264::common::prof::Stage::PredBuf);
                    for dy in 0..h {
                        pred_y[(y4 * 4 + dy) * 16 + x4 * 4..][..w]
                            .copy_from_slice(&t[dy * w..dy * w + w]);
                    }
                }
                let (cw4, ch4) = (w4 * 2, h4 * 2);
                let nc = cw4 * ch4;
                let (gu, gv) = (
                    reference.chroma_guard(0, reference.ch),
                    reference.chroma_guard(1, reference.ch),
                );
                if cw4 == 8 {
                    let [cu, cv] = &mut *c_pred;
                    crate::h264::common::inter::mc_chroma_padded_pair(
                        &gu,
                        &gv,
                        reference.cstride(),
                        crate::h264::decoder::CPAD,
                        ccw,
                        cch,
                        j.mbx * 8,
                        j.mby * 8 + y4 * 2,
                        cw4,
                        ch4,
                        mv.0,
                        mv.1,
                        &mut cu[y4 * 16..y4 * 16 + nc],
                        &mut cv[y4 * 16..y4 * 16 + nc],
                    );
                } else {
                    let (mut tu, mut tv) = ([0u8; 64], [0u8; 64]);
                    crate::h264::common::inter::mc_chroma_padded_pair(
                        &gu,
                        &gv,
                        reference.cstride(),
                        crate::h264::decoder::CPAD,
                        ccw,
                        cch,
                        j.mbx * 8 + x4 * 2,
                        j.mby * 8 + y4 * 2,
                        cw4,
                        ch4,
                        mv.0,
                        mv.1,
                        &mut tu[..nc],
                        &mut tv[..nc],
                    );
                    let _pb =
                        crate::h264::common::prof::scope(crate::h264::common::prof::Stage::PredBuf);
                    for (cc, tc) in [(0usize, &tu), (1, &tv)] {
                        for dy in 0..ch4 {
                            c_pred[cc][(y4 * 2 + dy) * 8 + x4 * 2..][..cw4]
                                .copy_from_slice(&tc[dy * cw4..dy * cw4 + cw4]);
                        }
                    }
                }
            };
            if rect_eq(0, 0, 4, 4) {
                mc_rect(0, 0, 4, 4, &mut pred_y, &mut c_pred);
            } else if rect_eq(0, 0, 4, 2) && rect_eq(0, 2, 4, 2) {
                mc_rect(0, 0, 4, 2, &mut pred_y, &mut c_pred);
                mc_rect(0, 2, 4, 2, &mut pred_y, &mut c_pred);
            } else if rect_eq(0, 0, 2, 4) && rect_eq(2, 0, 2, 4) {
                mc_rect(0, 0, 2, 4, &mut pred_y, &mut c_pred);
                mc_rect(2, 0, 2, 4, &mut pred_y, &mut c_pred);
            } else {
                for q in 0..4usize {
                    let (qx, qy) = ((q % 2) * 2, (q / 2) * 2);
                    if rect_eq(qx, qy, 2, 2) {
                        mc_rect(qx, qy, 2, 2, &mut pred_y, &mut c_pred);
                    } else if rect_eq(qx, qy, 2, 1) && rect_eq(qx, qy + 1, 2, 1) {
                        mc_rect(qx, qy, 2, 1, &mut pred_y, &mut c_pred);
                        mc_rect(qx, qy + 1, 2, 1, &mut pred_y, &mut c_pred);
                    } else if rect_eq(qx, qy, 1, 2) && rect_eq(qx + 1, qy, 1, 2) {
                        mc_rect(qx, qy, 1, 2, &mut pred_y, &mut c_pred);
                        mc_rect(qx + 1, qy, 1, 2, &mut pred_y, &mut c_pred);
                    } else {
                        for j in 0..4usize {
                            mc_rect(qx + (j % 2), qy + (j / 2), 1, 1, &mut pred_y, &mut c_pred);
                        }
                    }
                }
            }
            if self.weights.is_some() {
                for by in 0..4usize {
                    for bx in 0..4usize {
                        let refi = gref[by * 4 + bx];
                        self.weight_partition(
                            &mut pred_y,
                            &mut c_pred,
                            0,
                            refi,
                            bx * 4,
                            by * 4,
                            4,
                            4,
                        );
                    }
                }
            }
        }
        self.add_inter_residual(
            j.mbx,
            j.mby,
            &pred_y,
            &c_pred,
            Some(&j.luma_scan),
            j.t8.then_some(&j.luma8),
            &j.cdc,
            Some(&j.cac),
            j.cbp_chroma,
            &j.nnzs,
        );
    }
    fn recon_p_inter_nores(&mut self, j: &PInterNoResJob) {
        let mut pred_y = [0u8; 256];
        let mut c_pred = [[0u8; 64]; 2];
        let nrefs = self.refs.len() - 1;
        let gref: [usize; 16] = core::array::from_fn(|k| (j.gref[k] as usize).min(nrefs));
        coalesce_p_inter_mc(
            &self.refs,
            self.cw,
            self.ccw,
            self.mb_h,
            j.mbx,
            j.mby,
            &j.gmv,
            &gref,
            &mut pred_y,
            &mut c_pred,
        );
        if self.weights.is_some() && !self.weights_l0id {
            for by in 0..4usize {
                for bx in 0..4usize {
                    let refi = gref[by * 4 + bx];
                    self.weight_partition(&mut pred_y, &mut c_pred, 0, refi, bx * 4, by * 4, 4, 4);
                }
            }
        }
        let (cw, ccw) = (self.cw, self.ccw);
        let ybase = (j.mby * 16) * cw + j.mbx * 16;
        let ywin = &mut self.rec_y[ybase..ybase + 15 * cw + 16];
        for dy in 0..16 {
            ywin[dy * cw..dy * cw + 16].copy_from_slice(&pred_y[dy * 16..dy * 16 + 16]);
        }
        let cbase = (j.mby * 8) * ccw + j.mbx * 8;
        for c in 0..2 {
            let plane = if c == 0 {
                &mut self.rec_u
            } else {
                &mut self.rec_v
            };
            let cwin = &mut plane[cbase..cbase + 7 * ccw + 8];
            for dy in 0..8 {
                cwin[dy * ccw..dy * ccw + 8].copy_from_slice(&c_pred[c][dy * 8..dy * 8 + 8]);
            }
        }
    }
    fn recon_p_skip(&mut self, mb_x: usize, mb_y: usize, mv: (i32, i32)) {
        crate::h264::decoder::RefFrame::set_mc_row_need(mb_y, self.mb_h * 16);
        let (ch, cch) = (self.mb_h * 16, self.mb_h * 8);
        let mut pred = [0u8; 256];
        let Some(rf0) = self.refs.first() else { return };
        mc_luma_padded(
            &*rf0.luma_guard(rf0.ch),
            rf0.lstride(),
            crate::h264::decoder::LPAD,
            self.cw,
            ch,
            mb_x * 16,
            mb_y * 16,
            16,
            16,
            mv.0,
            mv.1,
            &mut pred,
        );
        if let Some(wt) = &self.weights {
            for p in pred.iter_mut() {
                *p = wt.apply_luma(*p, 0, 0);
            }
        }
        {
            let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::SkipRecon);
            let (cw, base) = (self.cw, (mb_y * 16) * self.cw + mb_x * 16);
            let win = &mut self.rec_y[base..base + 15 * cw + 16];
            for dy in 0..16 {
                win[dy * cw..dy * cw + 16].copy_from_slice(&pred[dy * 16..dy * 16 + 16]);
            }
        }
        for c in 0..2 {
            let mut pc = [0u8; 64];
            let Some(rf0) = self.refs.first() else { return };
            let rc = if c == 0 {
                &*rf0.chroma_guard(0, rf0.ch)
            } else {
                &*rf0.chroma_guard(1, rf0.ch)
            };
            mc_chroma_padded(
                rc,
                rf0.cstride(),
                crate::h264::decoder::CPAD,
                self.ccw,
                cch,
                mb_x * 8,
                mb_y * 8,
                8,
                8,
                mv.0,
                mv.1,
                &mut pc,
            );
            if let Some(wt) = &self.weights {
                for p in pc.iter_mut() {
                    *p = wt.apply_chroma(*p, 0, 0, c);
                }
            }
            let plane = if c == 0 {
                &mut self.rec_u
            } else {
                &mut self.rec_v
            };
            for dy in 0..8 {
                let d = (mb_y * 8 + dy) * self.ccw + mb_x * 8;
                plane[d..d + 8].copy_from_slice(&pc[dy * 8..dy * 8 + 8]);
            }
        }
    }
    fn add_inter_residual(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        pred_y: &[u8; 256],
        c_pred: &[[u8; 64]; 2],
        luma_scan: Option<&[[i32; 16]; 16]>,
        luma8: Option<&[[i32; 64]; 4]>,
        cdc: &[[i32; 4]; 2],
        cac: Option<&[[[i32; 16]; 4]; 2]>,
        cbp_chroma: u32,
        nnzs: &[u8; 24],
    ) {
        let luma_scan = luma_scan.unwrap_or(&ZERO_LUMA_SCAN);
        let cac = cac.unwrap_or(&ZERO_CAC);
        let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecResidAdd);
        let qp = self.cur_qp;
        let qpc = self.chroma_qp_for(qp);
        if let Some(l8) = luma8 {
            for b8 in 0..4usize {
                let (b8x, b8y) = (b8 % 2, b8 / 2);
                let nnz: u32 = (0..4).map(|k| nnzs[b8 * 4 + k] as u32).sum();
                let (px, py) = (mb_x * 16 + b8x * 8, mb_y * 16 + b8y * 8);
                if nnz == 0 {
                    for dy in 0..8 {
                        let d = (py + dy) * self.cw + px;
                        let po = (b8y * 8 + dy) * 16 + b8x * 8;
                        self.rec_y[d..d + 8].copy_from_slice(&pred_y[po..po + 8]);
                    }
                } else {
                    let raster = un_scan_8x8(&l8[b8]);
                    let res8 = self.inv_quant8(&raster, qp, 1);
                    let predb: [i32; 64] = core::array::from_fn(|i| {
                        pred_y[(b8y * 8 + i / 8) * 16 + (b8x * 8 + i % 8)] as i32
                    });
                    let recon = add_residual_8x8(&res8, &predb);
                    for dy in 0..8 {
                        let d = (py + dy) * self.cw + px;
                        self.rec_y[d..][..8].copy_from_slice(&recon[dy * 8..][..8]);
                    }
                }
            }
        }
        let dq3 = self.dq_const(qp, 3);
        for (blk, &(lbx, lby)) in LUMA_4X4_SCAN_XY.iter().enumerate() {
            if luma8.is_some() {
                break;
            }
            let nnz = nnzs[blk];
            let cw = self.cw;
            let p_off = (lby * 4) * 16 + lbx * 4;
            let r_off = (mb_y * 4 + lby) * 4 * cw + (mb_x * 4 + lbx) * 4;
            if nnz == 0 {
                for r in 0..4 {
                    self.rec_y[r_off + r * cw..r_off + r * cw + 4]
                        .copy_from_slice(&pred_y[p_off + r * 16..p_off + r * 16 + 4]);
                }
                continue;
            }
            if nnz == 1 && luma_scan[blk][0] != 0 {
                let f = self.dequant_dc4(luma_scan[blk][0], qp, 3);
                reconstruct_4x4_dc_into(
                    (f + 32) >> 6,
                    pred_y,
                    p_off,
                    16,
                    &mut self.rec_y,
                    r_off,
                    cw,
                );
            } else {
                reconstruct_4x4_scan_into::<false>(
                    &luma_scan[blk],
                    &dq3,
                    0,
                    pred_y,
                    p_off,
                    16,
                    &mut self.rec_y,
                    r_off,
                    cw,
                );
            }
        }
        let mut c_dc = [[0i32; 4]; 2];
        if cbp_chroma != 0 {
            for c in 0..2 {
                c_dc[c] = self.dequant_chroma_dc(&cdc[c], qpc, 4 + c);
            }
        }
        let dqc = [self.dq_const(qpc, 4), self.dq_const(qpc, 5)];
        let ccw = self.ccw;
        for c in 0..2 {
            for &(bx, by) in &CHROMA_4X4_SCAN_XY {
                let mut ac_nz = false;
                if cbp_chroma == 2 {
                    let n = nnzs[(16 + c * 4 + by * 2 + bx).min(23)];
                    ac_nz = n != 0;
                }
                let dc = c_dc[c & 1][(by * 2 + bx) & 3];
                let p_off = (by * 4) * 8 + bx * 4;
                let r_off = (mb_y * 2 + by) * 4 * ccw + (mb_x * 2 + bx) * 4;
                let plane = if c == 0 {
                    &mut self.rec_u
                } else {
                    &mut self.rec_v
                };
                if dc == 0 && !ac_nz {
                    for r in 0..4 {
                        plane[r_off + r * ccw..r_off + r * ccw + 4]
                            .copy_from_slice(&c_pred[c][p_off + r * 8..p_off + r * 8 + 4]);
                    }
                    continue;
                }
                if !ac_nz {
                    reconstruct_4x4_dc_into(
                        (dc + 32) >> 6,
                        &c_pred[c],
                        p_off,
                        8,
                        plane,
                        r_off,
                        ccw,
                    );
                    continue;
                }
                reconstruct_4x4_scan_into::<true>(
                    &cac[c & 1][(by * 2 + bx) & 3],
                    &dqc[c & 1],
                    dc,
                    &c_pred[c],
                    p_off,
                    8,
                    plane,
                    r_off,
                    ccw,
                );
            }
        }
    }
    fn filter_row(&mut self, r: usize) {
        let info = crate::h264::common::deblock::BlockInfo {
            inter: &[],
            nnz: &[],
            mv: &[],
            ref_id: &[],
            mv1: &[],
            ref_id1: &[],
            w4: self.mb_w * 4,
            t8x8: &self.t8_grid,
            bs: &self.bs_store,
            poc0: &[],
            poc1: &[],
            kind: &[],
        };
        crate::h264::common::deblock::filter_frame_rows_pre(
            &mut self.rec_y,
            &mut self.rec_u,
            &mut self.rec_v,
            self.mb_w,
            self.mb_h,
            r..r + 1,
            &self.qp_grid,
            self.chroma_qp_offset,
            self.db_oa,
            self.db_ob,
            &info,
        );
    }
    fn save_bak(&mut self, r: usize) {
        let y0 = (r * 16 + 15) * self.cw;
        self.bak_y.copy_from_slice(&self.rec_y[y0..y0 + self.cw]);
        let c0 = (r * 8 + 7) * self.ccw;
        self.bak_u.copy_from_slice(&self.rec_u[c0..c0 + self.ccw]);
        self.bak_v.copy_from_slice(&self.rec_v[c0..c0 + self.ccw]);
    }
}
fn edc_worker(
    mut ctx: PixelCtx,
    rx: crate::h264::decoder::sync::mpsc::Receiver<EdcMsg>,
    ctx_tx: crate::h264::decoder::sync::mpsc::Sender<PixelCtx>,
    back_rx: crate::h264::decoder::sync::mpsc::Receiver<PixelCtx>,
) -> PixelCtx {
    while let Ok(msg) = rx.recv() {
        match msg {
            EdcMsg::Batch(jobs) => {
                for j in jobs {
                    match j {
                        EdcJob::Skip { mbx, mby, mv } => ctx.recon_p_skip(mbx, mby, mv),
                        EdcJob::Inter(j) => ctx.recon_p_inter(&j),
                        EdcJob::InterNoRes(j) => ctx.recon_p_inter_nores(&j),
                        EdcJob::B(j) => ctx.recon_b(&j),
                        EdcJob::BSkip { mbx, mby, regions } => ctx.recon_b_skip(mbx, mby, &regions),
                    }
                }
            }
            EdcMsg::Job(EdcJob::Skip { mbx, mby, mv }) => ctx.recon_p_skip(mbx, mby, mv),
            EdcMsg::Job(EdcJob::Inter(j)) => ctx.recon_p_inter(&j),
            EdcMsg::Job(EdcJob::InterNoRes(j)) => ctx.recon_p_inter_nores(&j),
            EdcMsg::Job(EdcJob::B(j)) => ctx.recon_b(&j),
            EdcMsg::Job(EdcJob::BSkip { mbx, mby, regions }) => {
                ctx.recon_b_skip(mbx, mby, &regions)
            }
            EdcMsg::Row { r, bs, qp, t8 } => {
                let (w, base) = (ctx.mb_w, r * ctx.mb_w);
                ctx.bs_store[base..base + w].copy_from_slice(&bs);
                ctx.qp_grid[base..base + w].copy_from_slice(&qp);
                ctx.t8_grid[base..base + w].copy_from_slice(&t8);
                if ctx.db_ena {
                    ctx.save_bak(r);
                    ctx.filter_row(r);
                    ctx.flt_rows = r + 1;
                    ctx.publish_progress_rows();
                }
            }
            EdcMsg::NeedCtx => {
                ctx_tx.send(ctx).expect("parse thread alive");
                ctx = back_rx.recv().expect("ctx returned after intra");
            }
        }
    }
    ctx
}
pub(crate) struct BRegion {
    px: usize,
    py: usize,
    rw: usize,
    rh: usize,
    refi0: i32,
    refi1: i32,
    mv0: (i32, i32),
    mv1: (i32, i32),
    w: Option<(i32, i32)>,
}
pub(crate) struct BJob {
    mbx: usize,
    mby: usize,
    qp: u8,
    cbp_chroma: u32,
    skip: bool,
    regions: Vec<BRegion>,
    luma_scan: Option<[[i32; 16]; 16]>,
    luma8: Option<[[i32; 64]; 4]>,
    cdc: [[i32; 4]; 2],
    cac: Option<[[[i32; 16]; 4]; 2]>,
    nnzs: [u8; 24],
}
impl PixelCtx {
    fn b_mc(
        &self,
        mb_x: usize,
        mb_y: usize,
        px: usize,
        py: usize,
        rw: usize,
        rh: usize,
        refi0: i32,
        mv0: (i32, i32),
        refi1: i32,
        mv1: (i32, i32),
        pred_y: &mut [u8; 256],
        c_pred: &mut [[u8; 64]; 2],
        wparam: Option<(i32, i32)>,
    ) {
        let _gb = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecBMc);
        let refi0 = if refi0 >= 0 {
            (refi0 as usize).min(self.refs.len().saturating_sub(1)) as i32
        } else {
            -1
        };
        let refi1 = if refi1 >= 0 {
            (refi1 as usize).min(self.refs1.len().saturating_sub(1)) as i32
        } else {
            -1
        };
        if (refi0 >= 0 && self.refs.is_empty()) || (refi1 >= 0 && self.refs1.is_empty()) {
            return;
        }
        let (ch, cch) = (self.mb_h * 16, self.mb_h * 8);
        let weights = wparam;
        let full = px == 0 && rw == 16;
        let _gl = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecBLuma);
        let chroma_done = crate::h264::common::inter::with_mc_scratch(|scr| {
            match (refi0 >= 0, refi1 >= 0, full) {
                (true, false, true) => {
                    let Some(rf) = self.refs.get(refi0 as usize) else {
                        return false;
                    };
                    crate::h264::common::inter::mc_luma_padded_pre(
                        scr,
                        &*rf.luma_guard(rf.ch),
                        rf.lstride(),
                        crate::h264::decoder::LPAD,
                        self.cw,
                        ch,
                        mb_x * 16,
                        mb_y * 16 + py,
                        rw,
                        rh,
                        mv0.0,
                        mv0.1,
                        &mut pred_y[py * 16..py * 16 + rw * rh],
                    );
                    false
                }
                (false, true, true) => {
                    let Some(rf) = self.refs1.get(refi1 as usize) else {
                        return false;
                    };
                    crate::h264::common::inter::mc_luma_padded_pre(
                        scr,
                        &*rf.luma_guard(rf.ch),
                        rf.lstride(),
                        crate::h264::decoder::LPAD,
                        self.cw,
                        ch,
                        mb_x * 16,
                        mb_y * 16 + py,
                        rw,
                        rh,
                        mv1.0,
                        mv1.1,
                        &mut pred_y[py * 16..py * 16 + rw * rh],
                    );
                    false
                }
                (true, true, true) => {
                    let Some(rf) = self.refs.get(refi0 as usize) else {
                        return false;
                    };
                    crate::h264::common::inter::mc_luma_padded_pre(
                        scr,
                        &*rf.luma_guard(rf.ch),
                        rf.lstride(),
                        crate::h264::decoder::LPAD,
                        self.cw,
                        ch,
                        mb_x * 16,
                        mb_y * 16 + py,
                        rw,
                        rh,
                        mv0.0,
                        mv0.1,
                        &mut pred_y[py * 16..py * 16 + rw * rh],
                    );
                    let mut b = [0u8; 256];
                    let Some(rf) = self.refs1.get(refi1 as usize) else {
                        return false;
                    };
                    crate::h264::common::inter::mc_luma_padded_pre(
                        scr,
                        &*rf.luma_guard(rf.ch),
                        rf.lstride(),
                        crate::h264::decoder::LPAD,
                        self.cw,
                        ch,
                        mb_x * 16,
                        mb_y * 16 + py,
                        rw,
                        rh,
                        mv1.0,
                        mv1.1,
                        &mut b[..rw * rh],
                    );
                    drop(_gl);
                    let _gbl = crate::h264::common::prof::scope(
                        crate::h264::common::prof::Stage::DecBBlend,
                    );
                    let dst = &mut pred_y[py * 16..py * 16 + rw * rh];
                    match weights {
                        None => {
                            for (d, s) in dst.iter_mut().zip(&b[..rw * rh]) {
                                *d = ((*d as u16 + *s as u16 + 1) >> 1) as u8;
                            }
                        }
                        Some((w0, w1)) => {
                            for (d, s) in dst.iter_mut().zip(&b[..rw * rh]) {
                                *d = ((*d as i32 * w0 + *s as i32 * w1 + 32) >> 6).clamp(0, 255)
                                    as u8;
                            }
                        }
                    }
                    let _gc = crate::h264::common::prof::scope(
                        crate::h264::common::prof::Stage::DecBChroma,
                    );
                    self.b_mc_chroma(
                        mb_x, mb_y, px, py, rw, rh, refi0, mv0, refi1, mv1, c_pred, weights, cch,
                    );
                    true
                }
                _ => {
                    let (mut a, mut b) = ([0u8; 256], [0u8; 256]);
                    if refi0 >= 0 {
                        let Some(rf) = self.refs.get(refi0 as usize) else {
                            return false;
                        };
                        crate::h264::common::inter::mc_luma_padded_pre(
                            scr,
                            &*rf.luma_guard(rf.ch),
                            rf.lstride(),
                            crate::h264::decoder::LPAD,
                            self.cw,
                            ch,
                            mb_x * 16 + px,
                            mb_y * 16 + py,
                            rw,
                            rh,
                            mv0.0,
                            mv0.1,
                            &mut a[..rw * rh],
                        );
                    }
                    if refi1 >= 0 {
                        let Some(rf) = self.refs1.get(refi1 as usize) else {
                            return false;
                        };
                        crate::h264::common::inter::mc_luma_padded_pre(
                            scr,
                            &*rf.luma_guard(rf.ch),
                            rf.lstride(),
                            crate::h264::decoder::LPAD,
                            self.cw,
                            ch,
                            mb_x * 16 + px,
                            mb_y * 16 + py,
                            rw,
                            rh,
                            mv1.0,
                            mv1.1,
                            &mut b[..rw * rh],
                        );
                    }
                    drop(_gl);
                    let _gbl = crate::h264::common::prof::scope(
                        crate::h264::common::prof::Stage::DecBBlend,
                    );
                    match (refi0 >= 0, refi1 >= 0) {
                        (true, true) => {
                            for dy in 0..rh {
                                let (ar, br) =
                                    (&a[dy * rw..dy * rw + rw], &b[dy * rw..dy * rw + rw]);
                                let base = (py + dy) * 16 + px;
                                let dst = &mut pred_y[base..base + rw];
                                match weights {
                                    None => {
                                        for ((d, p), q) in dst.iter_mut().zip(ar).zip(br) {
                                            *d = ((*p as u16 + *q as u16 + 1) >> 1) as u8;
                                        }
                                    }
                                    Some((w0, w1)) => {
                                        for ((d, p), q) in dst.iter_mut().zip(ar).zip(br) {
                                            *d = ((*p as i32 * w0 + *q as i32 * w1 + 32) >> 6)
                                                .clamp(0, 255)
                                                as u8;
                                        }
                                    }
                                }
                            }
                        }
                        (true, false) => {
                            for dy in 0..rh {
                                let d = (py + dy) * 16 + px;
                                pred_y[d..d + rw].copy_from_slice(&a[dy * rw..dy * rw + rw]);
                            }
                        }
                        _ => {
                            for dy in 0..rh {
                                let d = (py + dy) * 16 + px;
                                pred_y[d..d + rw].copy_from_slice(&b[dy * rw..dy * rw + rw]);
                            }
                        }
                    }
                    false
                }
            }
        });
        if chroma_done {
            return;
        }
        let _gc = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecBChroma);
        self.b_mc_chroma(
            mb_x, mb_y, px, py, rw, rh, refi0, mv0, refi1, mv1, c_pred, weights, cch,
        );
    }
    fn b_mc_chroma(
        &self,
        mb_x: usize,
        mb_y: usize,
        px: usize,
        py: usize,
        rw: usize,
        rh: usize,
        refi0: i32,
        mv0: (i32, i32),
        refi1: i32,
        mv1: (i32, i32),
        c_pred: &mut [[u8; 64]; 2],
        weights: Option<(i32, i32)>,
        cch: usize,
    ) {
        let (crx, cry, crw, crh) = (px / 2, py / 2, rw / 2, rh / 2);
        let full = crx == 0 && crw == 8;
        for c in 0..2 {
            match (refi0 >= 0, refi1 >= 0, full) {
                (true, false, true) => {
                    let Some(rf) = self.refs.get(refi0 as usize) else {
                        return;
                    };
                    let pl = if c == 0 {
                        &*rf.chroma_guard(0, rf.ch)
                    } else {
                        &*rf.chroma_guard(1, rf.ch)
                    };
                    mc_chroma_padded(
                        pl,
                        rf.cstride(),
                        crate::h264::decoder::CPAD,
                        self.ccw,
                        cch,
                        mb_x * 8,
                        mb_y * 8 + cry,
                        crw,
                        crh,
                        mv0.0,
                        mv0.1,
                        &mut c_pred[c][cry * 8..cry * 8 + crw * crh],
                    );
                }
                (false, true, true) => {
                    let Some(rf) = self.refs1.get(refi1 as usize) else {
                        return;
                    };
                    let pl = if c == 0 {
                        &*rf.chroma_guard(0, rf.ch)
                    } else {
                        &*rf.chroma_guard(1, rf.ch)
                    };
                    mc_chroma_padded(
                        pl,
                        rf.cstride(),
                        crate::h264::decoder::CPAD,
                        self.ccw,
                        cch,
                        mb_x * 8,
                        mb_y * 8 + cry,
                        crw,
                        crh,
                        mv1.0,
                        mv1.1,
                        &mut c_pred[c][cry * 8..cry * 8 + crw * crh],
                    );
                }
                (true, true, true) => {
                    let Some(rf) = self.refs.get(refi0 as usize) else {
                        return;
                    };
                    let pl = if c == 0 {
                        &*rf.chroma_guard(0, rf.ch)
                    } else {
                        &*rf.chroma_guard(1, rf.ch)
                    };
                    mc_chroma_padded(
                        pl,
                        rf.cstride(),
                        crate::h264::decoder::CPAD,
                        self.ccw,
                        cch,
                        mb_x * 8,
                        mb_y * 8 + cry,
                        crw,
                        crh,
                        mv0.0,
                        mv0.1,
                        &mut c_pred[c][cry * 8..cry * 8 + crw * crh],
                    );
                    let mut cb = [0u8; 64];
                    let Some(rf) = self.refs1.get(refi1 as usize) else {
                        return;
                    };
                    let pl = if c == 0 {
                        &*rf.chroma_guard(0, rf.ch)
                    } else {
                        &*rf.chroma_guard(1, rf.ch)
                    };
                    mc_chroma_padded(
                        pl,
                        rf.cstride(),
                        crate::h264::decoder::CPAD,
                        self.ccw,
                        cch,
                        mb_x * 8,
                        mb_y * 8 + cry,
                        crw,
                        crh,
                        mv1.0,
                        mv1.1,
                        &mut cb[..crw * crh],
                    );
                    let dst = &mut c_pred[c][cry * 8..cry * 8 + crw * crh];
                    match weights {
                        None => {
                            for (d, s) in dst.iter_mut().zip(&cb[..crw * crh]) {
                                *d = ((*d as u16 + *s as u16 + 1) >> 1) as u8;
                            }
                        }
                        Some((w0, w1)) => {
                            for (d, s) in dst.iter_mut().zip(&cb[..crw * crh]) {
                                *d = ((*d as i32 * w0 + *s as i32 * w1 + 32) >> 6).clamp(0, 255)
                                    as u8;
                            }
                        }
                    }
                }
                _ => {
                    let (mut ca, mut cb) = ([0u8; 64], [0u8; 64]);
                    if refi0 >= 0 {
                        let Some(rf) = self.refs.get(refi0 as usize) else {
                            return;
                        };
                        let pl = if c == 0 {
                            &*rf.chroma_guard(0, rf.ch)
                        } else {
                            &*rf.chroma_guard(1, rf.ch)
                        };
                        mc_chroma_padded(
                            pl,
                            rf.cstride(),
                            crate::h264::decoder::CPAD,
                            self.ccw,
                            cch,
                            mb_x * 8 + crx,
                            mb_y * 8 + cry,
                            crw,
                            crh,
                            mv0.0,
                            mv0.1,
                            &mut ca[..crw * crh],
                        );
                    }
                    if refi1 >= 0 {
                        let Some(rf) = self.refs1.get(refi1 as usize) else {
                            return;
                        };
                        let pl = if c == 0 {
                            &*rf.chroma_guard(0, rf.ch)
                        } else {
                            &*rf.chroma_guard(1, rf.ch)
                        };
                        mc_chroma_padded(
                            pl,
                            rf.cstride(),
                            crate::h264::decoder::CPAD,
                            self.ccw,
                            cch,
                            mb_x * 8 + crx,
                            mb_y * 8 + cry,
                            crw,
                            crh,
                            mv1.0,
                            mv1.1,
                            &mut cb[..crw * crh],
                        );
                    }
                    match (refi0 >= 0, refi1 >= 0) {
                        (true, true) => {
                            for dy in 0..crh {
                                let (pr, qr) =
                                    (&ca[dy * crw..dy * crw + crw], &cb[dy * crw..dy * crw + crw]);
                                let base = (cry + dy) * 8 + crx;
                                let dst = &mut c_pred[c][base..base + crw];
                                match weights {
                                    None => {
                                        for ((d, p), q) in dst.iter_mut().zip(pr).zip(qr) {
                                            *d = ((*p as u16 + *q as u16 + 1) >> 1) as u8;
                                        }
                                    }
                                    Some((w0, w1)) => {
                                        for ((d, p), q) in dst.iter_mut().zip(pr).zip(qr) {
                                            *d = ((*p as i32 * w0 + *q as i32 * w1 + 32) >> 6)
                                                .clamp(0, 255)
                                                as u8;
                                        }
                                    }
                                }
                            }
                        }
                        (true, false) => {
                            for dy in 0..crh {
                                let d = (cry + dy) * 8 + crx;
                                c_pred[c][d..d + crw]
                                    .copy_from_slice(&ca[dy * crw..dy * crw + crw]);
                            }
                        }
                        _ => {
                            for dy in 0..crh {
                                let d = (cry + dy) * 8 + crx;
                                c_pred[c][d..d + crw]
                                    .copy_from_slice(&cb[dy * crw..dy * crw + crw]);
                            }
                        }
                    }
                }
            }
        }
    }
    fn recon_b_skip(&mut self, mbx: usize, mby: usize, regions: &[BRegion]) {
        crate::h264::decoder::RefFrame::set_mc_row_need(mby, self.mb_h * 16);
        let mut pred_y = [0u8; 256];
        let mut c_pred = [[0u8; 64]; 2];
        for r in regions {
            self.b_mc(
                mbx,
                mby,
                r.px,
                r.py,
                r.rw,
                r.rh,
                r.refi0,
                r.mv0,
                r.refi1,
                r.mv1,
                &mut pred_y,
                &mut c_pred,
                r.w,
            );
        }
        for dy in 0..16 {
            let d = (mby * 16 + dy) * self.cw + mbx * 16;
            self.rec_y[d..d + 16].copy_from_slice(&pred_y[dy * 16..dy * 16 + 16]);
        }
        for c in 0..2 {
            let plane = if c == 0 {
                &mut self.rec_u
            } else {
                &mut self.rec_v
            };
            for dy in 0..8 {
                let d = (mby * 8 + dy) * self.ccw + mbx * 8;
                plane[d..d + 8].copy_from_slice(&c_pred[c][dy * 8..dy * 8 + 8]);
            }
        }
    }
    fn recon_b(&mut self, j: &BJob) {
        crate::h264::decoder::RefFrame::set_mc_row_need(j.mby, self.mb_h * 16);
        let mut pred_y = [0u8; 256];
        let mut c_pred = [[0u8; 64]; 2];
        for r in &j.regions {
            self.b_mc(
                j.mbx,
                j.mby,
                r.px,
                r.py,
                r.rw,
                r.rh,
                r.refi0,
                r.mv0,
                r.refi1,
                r.mv1,
                &mut pred_y,
                &mut c_pred,
                r.w,
            );
        }
        if j.skip {
            for dy in 0..16 {
                let d = (j.mby * 16 + dy) * self.cw + j.mbx * 16;
                self.rec_y[d..d + 16].copy_from_slice(&pred_y[dy * 16..dy * 16 + 16]);
            }
            for c in 0..2 {
                let plane = if c == 0 {
                    &mut self.rec_u
                } else {
                    &mut self.rec_v
                };
                for dy in 0..8 {
                    let d = (j.mby * 8 + dy) * self.ccw + j.mbx * 8;
                    plane[d..d + 8].copy_from_slice(&c_pred[c][dy * 8..dy * 8 + 8]);
                }
            }
        } else {
            self.cur_qp = j.qp;
            self.add_inter_residual(
                j.mbx,
                j.mby,
                &pred_y,
                &c_pred,
                j.luma_scan.as_ref(),
                j.luma8.as_ref(),
                &j.cdc,
                j.cac.as_ref(),
                j.cbp_chroma,
                &j.nnzs,
            );
        }
    }
}
fn coalesce_p_inter_mc(
    refs: &[crate::h264::decoder::Ref],
    cw: usize,
    ccw: usize,
    mb_h: usize,
    mbx: usize,
    mby: usize,
    gmv: &[(i32, i32); 16],
    gref: &[usize; 16],
    pred_y: &mut [u8; 256],
    c_pred: &mut [[u8; 64]; 2],
) {
    let _ms = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecMcStage);
    let (rh16, cch) = (mb_h * 16, mb_h * 8);
    let rect_eq = |x4: usize, y4: usize, w4: usize, h4: usize| -> bool {
        let t = y4 * 4 + x4;
        (0..h4).all(|dy| {
            (0..w4).all(|dx| {
                let b = ((y4 + dy) * 4 + (x4 + dx)) & 15;
                gmv[b] == gmv[t] && gref[b] == gref[t]
            })
        })
    };
    let mut mc_rect = |x4: usize, y4: usize, w4: usize, h4: usize| {
        let b = y4 * 4 + x4;
        let (mv, gr) = (gmv[b & 15], gref[b & 15]);
        let Some(reference) = refs.get(gr) else {
            return;
        };
        let (w, h) = (w4 * 4, h4 * 4);
        if w == 16 {
            crate::h264::common::inter::with_mc_scratch(|scr| {
                crate::h264::common::inter::mc_luma_padded_pre(
                    scr,
                    &*reference.luma_guard(reference.ch),
                    reference.lstride(),
                    crate::h264::decoder::LPAD,
                    cw,
                    rh16,
                    mbx * 16,
                    mby * 16 + y4 * 4,
                    w,
                    h,
                    mv.0,
                    mv.1,
                    &mut pred_y[y4 * 64..y4 * 64 + w * h],
                )
            });
        } else {
            let mut t = [0u8; 256];
            crate::h264::common::inter::with_mc_scratch(|scr| {
                crate::h264::common::inter::mc_luma_padded_pre(
                    scr,
                    &*reference.luma_guard(reference.ch),
                    reference.lstride(),
                    crate::h264::decoder::LPAD,
                    cw,
                    rh16,
                    mbx * 16 + x4 * 4,
                    mby * 16 + y4 * 4,
                    w,
                    h,
                    mv.0,
                    mv.1,
                    &mut t[..w * h],
                )
            });
            let _pb = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::PredBuf);
            for dy in 0..h {
                pred_y[(y4 * 4 + dy) * 16 + x4 * 4..][..w].copy_from_slice(&t[dy * w..dy * w + w]);
            }
        }
        let (cw4, ch4) = (w4 * 2, h4 * 2);
        let nc = cw4 * ch4;
        let (gu, gv) = (
            reference.chroma_guard(0, reference.ch),
            reference.chroma_guard(1, reference.ch),
        );
        if cw4 == 8 {
            let [cu, cv] = &mut *c_pred;
            crate::h264::common::inter::mc_chroma_padded_pair(
                &gu,
                &gv,
                reference.cstride(),
                crate::h264::decoder::CPAD,
                ccw,
                cch,
                mbx * 8,
                mby * 8 + y4 * 2,
                cw4,
                ch4,
                mv.0,
                mv.1,
                &mut cu[y4 * 16..y4 * 16 + nc],
                &mut cv[y4 * 16..y4 * 16 + nc],
            );
        } else {
            let (mut tu, mut tv) = ([0u8; 64], [0u8; 64]);
            crate::h264::common::inter::mc_chroma_padded_pair(
                &gu,
                &gv,
                reference.cstride(),
                crate::h264::decoder::CPAD,
                ccw,
                cch,
                mbx * 8 + x4 * 2,
                mby * 8 + y4 * 2,
                cw4,
                ch4,
                mv.0,
                mv.1,
                &mut tu[..nc],
                &mut tv[..nc],
            );
            let _pb = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::PredBuf);
            for (cc, tc) in [(0usize, &tu), (1, &tv)] {
                for dy in 0..ch4 {
                    c_pred[cc][(y4 * 2 + dy) * 8 + x4 * 2..][..cw4]
                        .copy_from_slice(&tc[dy * cw4..dy * cw4 + cw4]);
                }
            }
        }
    };
    if rect_eq(0, 0, 4, 4) {
        mc_rect(0, 0, 4, 4);
    } else if rect_eq(0, 0, 4, 2) && rect_eq(0, 2, 4, 2) {
        mc_rect(0, 0, 4, 2);
        mc_rect(0, 2, 4, 2);
    } else if rect_eq(0, 0, 2, 4) && rect_eq(2, 0, 2, 4) {
        mc_rect(0, 0, 2, 4);
        mc_rect(2, 0, 2, 4);
    } else {
        for q in 0..4usize {
            let (qx, qy) = ((q % 2) * 2, (q / 2) * 2);
            if rect_eq(qx, qy, 2, 2) {
                mc_rect(qx, qy, 2, 2);
            } else if rect_eq(qx, qy, 2, 1) && rect_eq(qx, qy + 1, 2, 1) {
                mc_rect(qx, qy, 2, 1);
                mc_rect(qx, qy + 1, 2, 1);
            } else if rect_eq(qx, qy, 1, 2) && rect_eq(qx + 1, qy, 1, 2) {
                mc_rect(qx, qy, 1, 2);
                mc_rect(qx + 1, qy, 1, 2);
            } else {
                for j in 0..4usize {
                    mc_rect(qx + (j % 2), qy + (j / 2), 1, 1);
                }
            }
        }
    }
}
enum EdcJob {
    Skip {
        mbx: usize,
        mby: usize,
        mv: (i32, i32),
    },
    Inter(Box<PInterJob>),
    B(Box<BJob>),
    BSkip {
        mbx: usize,
        mby: usize,
        regions: Vec<BRegion>,
    },
    InterNoRes(Box<PInterNoResJob>),
}
struct PInterNoResJob {
    mbx: usize,
    mby: usize,
    t8: bool,
    gmv: [(i32, i32); 16],
    gref: [u8; 16],
}
static ZERO_LUMA_SCAN: [[i32; 16]; 16] = [[0; 16]; 16];
static TX_FOR_TD: [i32; 256] = {
    let mut t = [0i32; 256];
    let mut i = 0usize;
    while i < 256 {
        let td = i as i32 - 128;
        t[i] = if td == 0 {
            0
        } else {
            (16384 + td.abs() / 2) / td
        };
        i += 1;
    }
    t
};
#[inline(always)]
fn tx_for_td(td: i32) -> i32 {
    debug_assert!((-128..=127).contains(&td), "td must be clamped before this");
    TX_FOR_TD[(td.clamp(-128, 127) + 128) as usize]
}
static ZERO_NZC: [u8; 24] = [0u8; 24];
static ZERO_CAC: [[[i32; 16]; 4]; 2] = [[[0; 16]; 4]; 2];
struct PInterJob {
    mbx: usize,
    mby: usize,
    qp: u8,
    cbp_chroma: u32,
    t8: bool,
    gmv: [(i32, i32); 16],
    gref: [u8; 16],
    luma_scan: [[i32; 16]; 16],
    luma8: [[i32; 64]; 4],
    cdc: [[i32; 4]; 2],
    cac: [[[i32; 16]; 4]; 2],
    nnzs: [u8; 24],
}
impl PInterJob {
    fn blank() -> Self {
        PInterJob {
            mbx: 0,
            mby: 0,
            qp: 0,
            cbp_chroma: 0,
            t8: false,
            gmv: [(0, 0); 16],
            gref: [0; 16],
            luma_scan: [[0; 16]; 16],
            luma8: [[0; 64]; 4],
            cdc: [[0; 4]; 2],
            cac: [[[0; 16]; 4]; 2],
            nnzs: [0; 24],
        }
    }
}
fn edc_on() -> bool {
    {
        return true;
    }
}
fn batch_on() -> bool {
    {
        return true;
    }
}
fn nores_on() -> bool {
    {
        return true;
    }
}
fn fat_slice_on() -> bool {
    {
        return false;
    }
}
fn no_skipband() -> bool {
    {
        return false;
    }
}
fn no_runmv() -> bool {
    {
        return false;
    }
}
fn no_skipfp() -> bool {
    {
        return false;
    }
}
fn no_bskipfast() -> bool {
    {
        return false;
    }
}
fn double_recon() -> bool {
    {
        return false;
    }
}
fn edc_bound() -> usize {
    static V: crate::h264::common::once::OnceLock<usize> =
        crate::h264::common::once::OnceLock::new();
    *V.get_or_init(|| {
        crate::h264::common::knob("RS_H264_EDC_BOUND")
            .and_then(|v| v.parse().ok())
            .unwrap_or(256)
    })
}
fn edc_dispatch(mb_w: usize, mb_h: usize, bits_per_mb: f64, cabac: bool) -> bool {
    const BITS_MIN: f64 = 38.4;
    const MAX_MBS: usize = 5000;
    cabac && bits_per_mb > BITS_MIN && mb_w * mb_h <= MAX_MBS
}
fn edc_mt() -> Option<bool> {
    static V: crate::h264::common::once::OnceLock<Option<bool>> =
        crate::h264::common::once::OnceLock::new();
    *V.get_or_init(
        || match crate::h264::common::knob("RS_H264_EDC_MT").as_deref() {
            Some("0") => Some(false),
            Some("1") => Some(true),
            Some("auto") => None,
            _ => Some(false),
        },
    )
}
fn edc_spawn_worker(mb_w: usize, mb_h: usize, bits_per_mb: f64, cabac: bool) -> bool {
    {
        if crate::h264::decoder::frame_threads() > 1 {
            return false;
        }
        edc_mt().unwrap_or_else(|| edc_dispatch(mb_w, mb_h, bits_per_mb, cabac))
    }
}
fn bs_pre_on() -> bool {
    {
        return true;
    }
}
fn kind_loads() -> bool {
    {
        return false;
    }
}
fn rowdb_on() -> bool {
    {
        return true;
    }
}
fn rowhook_eager() -> bool {
    {
        return false;
    }
}
#[inline]
fn direct_memo_on() -> bool {
    {
        return true;
    }
}
use crate::h264::common::cabac_tables::CACHE30;
use crate::h264::common::cabac_tables::G_SCAN4;
fn parse_sub_mb_type_p_cabac(cab: &mut crate::h264::decoder::cabac::Cabac) -> u32 {
    const S: usize = 21;
    let (data, mut e, ctx) = cab.view();
    let v = if e.decode_decision(data, ctx, S) != 0 {
        0
    } else if e.decode_decision(data, ctx, S + 1) != 0 {
        3 - e.decode_decision(data, ctx, S + 2)
    } else {
        1
    };
    cab.commit(e);
    v
}
#[inline(always)]
fn intra_mb_type_eng(e: &mut Eng, data: &[u8], ctx: &mut Ctx, base: usize) -> u32 {
    if e.decode_decision(data, ctx, base) == 0 {
        return 0;
    }
    if e.decode_terminate(data) {
        return 25;
    }
    let mut t = 1 + 12 * e.decode_decision(data, ctx, base + 1) as u32;
    if e.decode_decision(data, ctx, base + 2) != 0 {
        t += 4 + 4 * e.decode_decision(data, ctx, base + 2) as u32;
    }
    t += 2 * e.decode_decision(data, ctx, base + 3) as u32;
    t += e.decode_decision(data, ctx, base + 3) as u32;
    t
}

#[inline]
fn parse_mb_type_b_cabac(cab: &mut crate::h264::decoder::cabac::Cabac, ctx_inc: usize) -> u32 {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Syntax);
    const B: usize = 27;
    let (data, mut e, ctx) = cab.view();
    let v = mb_type_b_eng(&mut e, data, ctx, B, ctx_inc);
    cab.commit(e);
    v
}
#[inline(always)]
fn mb_type_b_eng(e: &mut Eng, data: &[u8], ctx: &mut Ctx, b: usize, ctx_inc: usize) -> u32 {
    if e.decode_decision(data, ctx, b + ctx_inc) == 0 {
        return 0;
    }
    if e.decode_decision(data, ctx, b + 3) == 0 {
        return 1 + e.decode_decision(data, ctx, b + 5);
    }
    let mut m = e.decode_decision(data, ctx, b + 4) << 3;
    m |= e.decode_decision(data, ctx, b + 5) << 2;
    m |= e.decode_decision(data, ctx, b + 5) << 1;
    m |= e.decode_decision(data, ctx, b + 5);
    if m < 8 {
        return m + 3;
    }
    if m == 13 {
        return intra_mb_type_eng(e, data, ctx, 32) + 23;
    }
    if m == 14 {
        return 11;
    }
    if m == 15 {
        return 22;
    }
    m = (m << 1) | e.decode_decision(data, ctx, b + 5);
    m - 4
}
fn parse_sub_mb_type_b_cabac(cab: &mut crate::h264::decoder::cabac::Cabac) -> u32 {
    const B: usize = 36;
    let (data, mut e, ctx) = cab.view();
    let v = 'v: {
        if e.decode_decision(data, ctx, B) == 0 {
            break 'v 0;
        }
        if e.decode_decision(data, ctx, B + 1) == 0 {
            break 'v 1 + e.decode_decision(data, ctx, B + 3);
        }
        let mut st = 3u32;
        if e.decode_decision(data, ctx, B + 2) != 0 {
            if e.decode_decision(data, ctx, B + 3) != 0 {
                break 'v 11 + e.decode_decision(data, ctx, B + 3);
            }
            st += 4;
        }
        st += 2 * e.decode_decision(data, ctx, B + 3);
        st += e.decode_decision(data, ctx, B + 3);
        st
    };
    cab.commit(e);
    v
}
fn parse_mvd_partition(
    cab: &mut crate::h264::decoder::cabac::Cabac,
    part_idx: usize,
    zblocks: &[usize],
    mvdc: &mut [[i16; 2]; 30],
    refc: &mut [i8; 30],
    mmvd: &mut [[i16; 2]; 16],
    mref: &mut [i8; 16],
    ref_idx: i8,
) -> (i32, i32) {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Syntax);
    let s = CACHE30[part_idx & 15].clamp(6, 29);
    let (av_b, av_a) = (refc[s - 6] >= 0, refc[s - 1] >= 0);
    let (nb, na) = (mvdc[s - 6], mvdc[s - 1]);
    let ctx = |comp: usize| -> usize {
        let mut a = 0i32;
        if av_b {
            a += nb[comp].unsigned_abs() as i32;
        }
        if av_a {
            a += na[comp].unsigned_abs() as i32;
        }
        if a >= 3 { 1 + (a > 32) as usize } else { 0 }
    };
    let (cx, cy) = (ctx(0), ctx(1));
    let (data, mut e, ctx) = cab.view();
    let mvx = mvd_component(&mut e, data, ctx, 0, cx);
    let mvy = mvd_component(&mut e, data, ctx, 1, cy);
    cab.commit(e);
    let mvd = [mvx, mvy];
    if zblocks.len() == 16 {
        mmvd.fill(mvd);
        mref.fill(ref_idx);
        for r in [7usize, 13, 19, 25] {
            mvdc[r..r + 4].fill(mvd);
            refc[r..r + 4].fill(ref_idx);
        }
    } else {
        for &zb in zblocks {
            let (c, g) = (CACHE30[zb & 15], G_SCAN4[zb & 15]);
            mvdc[c] = mvd;
            refc[c] = ref_idx;
            mmvd[g] = mvd;
            mref[g] = ref_idx;
        }
    }
    (mvx as i32, mvy as i32)
}
pub fn parse_ref_idx_cabac(cab: &mut crate::h264::decoder::cabac::Cabac, ctx0: usize) -> i8 {
    const B: usize = 54;
    let (data, mut e, ctx) = cab.view();
    let mut r = 0i8;
    let mut bin_idx = 0u32;
    while bin_idx < 32 {
        let c = match bin_idx {
            0 => ctx0,
            1 => 4,
            _ => 5,
        };
        if e.decode_decision(data, ctx, B + c) == 0 {
            break;
        }
        r += 1;
        bin_idx += 1;
    }
    cab.commit(e);
    r
}
#[inline(always)]
fn decode_ueg_mv(e: &mut Eng, data: &[u8], ctx: &mut Ctx, base: usize) -> u32 {
    const P2C: [usize; 8] = [0, 1, 2, 3, 3, 3, 3, 3];
    if e.decode_decision(data, ctx, base) == 0 {
        return 0;
    }
    let mut code = 0u32;
    let mut count = 1usize;
    let mut tmp;
    loop {
        tmp = e.decode_decision(data, ctx, base + P2C[count & 7]);
        code += 1;
        count += 1;
        if tmp == 0 || count == 8 {
            break;
        }
    }
    if tmp != 0 {
        let (v, e2) = cabac_exp_bypass(*e, data, 3);
        *e = e2;
        code += v + 1;
    }
    code
}
#[inline(always)]
fn mvd_component(e: &mut Eng, data: &[u8], ctx: &mut Ctx, comp: usize, ctx_inc: usize) -> i16 {
    let base = 40 + comp * 7;
    if e.decode_decision(data, ctx, base + ctx_inc) == 0 {
        return 0;
    }
    let mag = (decode_ueg_mv(e, data, ctx, base + 3) + 1) as i16;
    if e.decode_bypass(data) != 0 {
        -mag
    } else {
        mag
    }
}
#[inline(always)]
fn mb_type_p_eng(e: &mut Eng, data: &[u8], ctx: &mut Ctx) -> u32 {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Syntax);
    const S: usize = 11;
    let v = 'v: {
        if e.decode_decision(data, ctx, S + 3) == 0 {
            break 'v if e.decode_decision(data, ctx, S + 4) != 0 {
                if e.decode_decision(data, ctx, S + 6) != 0 {
                    1
                } else {
                    2
                }
            } else if e.decode_decision(data, ctx, S + 5) != 0 {
                3
            } else {
                0
            };
        }
        if e.decode_decision(data, ctx, S + 6) == 0 {
            break 'v 5;
        }
        if e.decode_terminate(data) {
            break 'v 30;
        }
        let mut t = 6 + e.decode_decision(data, ctx, S + 7) * 12;
        if e.decode_decision(data, ctx, S + 8) != 0 {
            t += 4;
            if e.decode_decision(data, ctx, S + 8) != 0 {
                t += 4;
            }
        }
        t += e.decode_decision(data, ctx, S + 9) << 1;
        t += e.decode_decision(data, ctx, S + 9);
        t
    };
    v
}
fn parse_mb_type_i_cabac(cab: &mut crate::h264::decoder::cabac::Cabac, ctx_inc: usize) -> u32 {
    const O: usize = 3;
    let (data, mut e, ctx) = cab.view();
    let v = 'v: {
        if e.decode_decision(data, ctx, O + ctx_inc) == 0 {
            break 'v 0;
        }
        if e.decode_terminate(data) {
            break 'v 25;
        }
        let mut t = 1 + e.decode_decision(data, ctx, O + 3) * 12;
        if e.decode_decision(data, ctx, O + 4) != 0 {
            t += 4;
            if e.decode_decision(data, ctx, O + 5) != 0 {
                t += 4;
            }
        }
        t += e.decode_decision(data, ctx, O + 6) << 1;
        t += e.decode_decision(data, ctx, O + 7);
        t
    };
    cab.commit(e);
    v
}
#[inline(always)]
fn intra4x4_pred_mode_eng(e: &mut Eng, data: &[u8], ctx: &mut Ctx) -> i32 {
    const IPR: usize = 68;
    if e.decode_decision(data, ctx, IPR) == 1 {
        return -1;
    }
    let mut m = e.decode_decision(data, ctx, IPR + 1) as i32;
    m |= (e.decode_decision(data, ctx, IPR + 1) as i32) << 1;
    m |= (e.decode_decision(data, ctx, IPR + 1) as i32) << 2;
    m
}
fn parse_intra_chroma_pred_mode_cabac(
    cab: &mut crate::h264::decoder::cabac::Cabac,
    ctx_inc: usize,
) -> u32 {
    const CIPR: usize = 64;
    if cab.decode_decision(CIPR + ctx_inc) == 0 {
        return 0;
    }
    if cab.decode_decision(CIPR + 3) == 0 {
        return 1;
    }
    if cab.decode_decision(CIPR + 3) == 0 {
        return 2;
    }
    3
}
pub fn parse_cbp_cabac(
    cab: &mut crate::h264::decoder::cabac::Cabac,
    top: Option<u8>,
    left: Option<u8>,
) -> u32 {
    let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Syntax);
    const CBP: usize = 73;
    let tz = top.map_or(0u32, |c| !(c as u32) & 0xF);
    let lz = left.map_or(0u32, |c| !(c as u32) & 0xF);
    let tc = top.map_or(0u32, |c| (c >> 4) as u32);
    let lc = left.map_or(0u32, |c| (c >> 4) as u32);
    let (data, mut e, ctx) = cab.view();
    let b0 = e.decode_decision(
        data,
        ctx,
        CBP + (((lz >> 1) & 1) + (((tz >> 2) & 1) << 1)) as usize,
    );
    let b1 = e.decode_decision(
        data,
        ctx,
        CBP + ((b0 ^ 1) + (((tz >> 3) & 1) << 1)) as usize,
    );
    let b2 = e.decode_decision(
        data,
        ctx,
        CBP + (((lz >> 3) & 1) + ((b0 ^ 1) << 1)) as usize,
    );
    let b3 = e.decode_decision(data, ctx, CBP + ((b2 ^ 1) + ((b1 ^ 1) << 1)) as usize);
    let mut cbp = b0 | (b1 << 1) | (b2 << 2) | (b3 << 3);
    let (ct, cl) = ((tc != 0) as u32, (lc != 0) as u32);
    if e.decode_decision(data, ctx, CBP + 4 + (cl + (ct << 1)) as usize) != 0 {
        let (ct2, cl2) = ((tc == 2) as u32, (lc == 2) as u32);
        let c1 = e.decode_decision(data, ctx, CBP + 8 + (cl2 + (ct2 << 1)) as usize);
        cbp |= 1 << (4 + c1);
    }
    cab.commit(e);
    cbp
}
#[inline]
fn read_ref_idx(r: &mut BitReader, num_ref_active: usize) -> Result<i32, OutOfData> {
    if num_ref_active == 2 {
        Ok(if r.read_bit()? { 0 } else { 1 })
    } else {
        Ok(r.read_ue()? as i32)
    }
}
#[derive(Clone, Copy, PartialEq)]
enum BPred {
    L0,
    L1,
    Bi,
}
impl BPred {
    fn uses(self, list: usize) -> bool {
        matches!(
            (self, list),
            (BPred::L0, 0) | (BPred::L1, 1) | (BPred::Bi, 0) | (BPred::Bi, 1)
        )
    }
}
const B16X16: &[(usize, usize, usize, usize)] = &[(0, 0, 16, 16)];
const B16X8: &[(usize, usize, usize, usize)] = &[(0, 0, 16, 8), (0, 8, 16, 8)];
const B8X16: &[(usize, usize, usize, usize)] = &[(0, 0, 8, 16), (8, 0, 8, 16)];
type Region = (usize, usize, usize, usize);

fn b_inter_layout(mb_type: u32) -> (&'static [Region], u8, [BPred; 2]) {
    use BPred::*;
    match mb_type {
        1 => (B16X16, 0, [L0, L0]),
        2 => (B16X16, 0, [L1, L1]),
        3 => (B16X16, 0, [Bi, Bi]),
        4 => (B16X8, 1, [L0, L0]),
        5 => (B8X16, 2, [L0, L0]),
        6 => (B16X8, 1, [L1, L1]),
        7 => (B8X16, 2, [L1, L1]),
        8 => (B16X8, 1, [L0, L1]),
        9 => (B8X16, 2, [L0, L1]),
        10 => (B16X8, 1, [L1, L0]),
        11 => (B8X16, 2, [L1, L0]),
        12 => (B16X8, 1, [L0, Bi]),
        13 => (B8X16, 2, [L0, Bi]),
        14 => (B16X8, 1, [L1, Bi]),
        15 => (B8X16, 2, [L1, Bi]),
        16 => (B16X8, 1, [Bi, L0]),
        17 => (B8X16, 2, [Bi, L0]),
        18 => (B16X8, 1, [Bi, L1]),
        19 => (B8X16, 2, [Bi, L1]),
        20 => (B16X8, 1, [Bi, Bi]),
        _ => (B8X16, 2, [Bi, Bi]),
    }
}
fn b_sub_uses(st: u32, list: usize) -> bool {
    let pred = match st {
        1 | 4 | 5 | 10 => 0,
        2 | 6 | 7 | 11 => 1,
        _ => 2,
    };
    (list == 0 && pred != 1) || (list == 1 && pred != 0)
}
fn b_sub_parts(st: u32) -> &'static [(usize, usize, usize, usize)] {
    match st {
        1..=3 => &[(0, 0, 8, 8)],
        4 | 6 | 8 => &[(0, 0, 8, 4), (0, 4, 8, 4)],
        5 | 7 | 9 => &[(0, 0, 4, 8), (4, 0, 4, 8)],
        _ => &[(0, 0, 4, 4), (4, 0, 4, 4), (0, 4, 4, 4), (4, 4, 4, 4)],
    }
}
fn sub_mb_partitions(sub_type: u32) -> &'static [(usize, usize, usize, usize)] {
    match sub_type {
        0 => &[(0, 0, 8, 8)],
        1 => &[(0, 0, 8, 4), (0, 4, 8, 4)],
        2 => &[(0, 0, 4, 8), (4, 0, 4, 8)],
        _ => &[(0, 0, 4, 4), (4, 0, 4, 4), (0, 4, 4, 4), (4, 4, 4, 4)],
    }
}
#[inline]
fn restride(
    dst: &mut [u8],
    dst_stride: usize,
    x0: usize,
    y0: usize,
    src: &[u8],
    w: usize,
    h: usize,
) {
    macro_rules! rows {
        ($ n : expr) => {{
            for dy in 0..h {
                dst[(y0 + dy) * dst_stride + x0..][..$n].copy_from_slice(&src[dy * $n..][..$n]);
            }
        }};
    }
    match w {
        16 => rows!(16),
        8 => rows!(8),
        4 => rows!(4),
        2 => rows!(2),
        _ => {
            for dy in 0..h {
                dst[(y0 + dy) * dst_stride + x0..][..w].copy_from_slice(&src[dy * w..][..w]);
            }
        }
    }
}
fn un_scan_8x8(scan: &[i32; 64]) -> [i32; 64] {
    const ZZ8: [usize; 64] = [
        0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27,
        20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51,
        58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
    ];
    let mut out = [0i32; 64];
    for k in 0..64 {
        out[ZZ8[k]] = scan[k];
    }
    out
}
