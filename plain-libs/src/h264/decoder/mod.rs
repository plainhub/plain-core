pub(crate) mod sync {
    pub use std::sync::OnceLock as Once;
    pub use std::sync::{Arc, Condvar, Mutex, RwLock, RwLockReadGuard, mpsc};
    pub fn yield_now() {
        std::thread::yield_now();
    }
}
mod cabac;
mod frame_mt;
mod mb16;
mod params;
pub use mb16::MV_DUMP;
pub use mb16::MvField;
pub use params::{Pps, Sps};
pub(crate) fn frame_threads() -> usize {
    frame_mt::frame_threads()
}
pub(crate) fn row_publish_on() -> bool {
    frame_mt::row_publish_on()
}
pub(crate) fn row_progress_on() -> bool {
    frame_mt::row_progress_on()
}
fn abl_deblock() -> bool {
    {
        return false;
    }
}
use crate::h264::common::bit_reader::OutOfData;
use crate::h264::common::nal::{emulation_unprevent, split_annex_b};
use crate::h264::common::{BitReader, NalUnitType, YuvFrame};
use mb16::{FrameDecoder, GridPool, WeightTable};
use std::borrow::ToOwned;
use std::boxed::Box;
use std::format;
use std::string::{String, ToString};
use std::vec;
use std::vec::Vec;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    Truncated,
    MissingParameterSet,
    Unsupported(&'static str),
}
impl From<OutOfData> for DecodeError {
    fn from(_: OutOfData) -> Self {
        DecodeError::Truncated
    }
}
impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DecodeError::Truncated => f.write_str("bitstream truncated"),
            DecodeError::MissingParameterSet => f.write_str("slice before SPS/PPS"),
            DecodeError::Unsupported(s) => write!(f, "unsupported coding tool: {s}"),
        }
    }
}
impl core::error::Error for DecodeError {}
pub(crate) type Ref = crate::h264::decoder::sync::Arc<RefFrame>;
#[derive(Debug, Default)]
pub(crate) struct RefFrame {
    pub py: Vec<u8>,
    pub pu: Vec<u8>,
    pub pv: Vec<u8>,
    pub cw: usize,
    pub ch: usize,
    pub ready_rows: core::sync::atomic::AtomicUsize,
    pub live: Option<crate::h264::decoder::sync::Arc<LivePlanes>>,
    pub frozen: crate::h264::decoder::sync::Once<FrozenPlanes>,
    pub frame_num: u32,
    pub poc: i32,
    pub mv: Vec<(i32, i32)>,
    pub ref_idx: Vec<i32>,
    pub mv1: Vec<(i32, i32)>,
    pub ref_idx1: Vec<i32>,
    pub ref_poc: Vec<i32>,
    pub w4: usize,
    pub long_term: bool,
    pub long_term_idx: u32,
}
impl Clone for RefFrame {
    fn clone(&self) -> Self {
        use core::sync::atomic::Ordering::Relaxed;
        let frozen = crate::h264::decoder::sync::Once::new();
        if let Some(p) = self.frozen.get() {
            let _ = frozen.set(p.clone());
        }
        Self {
            py: self.py.clone(),
            pu: self.pu.clone(),
            pv: self.pv.clone(),
            cw: self.cw,
            ch: self.ch,
            ready_rows: core::sync::atomic::AtomicUsize::new(self.ready_rows.load(Relaxed)),
            live: self.live.clone(),
            frozen,
            frame_num: self.frame_num,
            poc: self.poc,
            mv: self.mv.clone(),
            ref_idx: self.ref_idx.clone(),
            mv1: self.mv1.clone(),
            ref_idx1: self.ref_idx1.clone(),
            ref_poc: self.ref_poc.clone(),
            w4: self.w4,
            long_term: self.long_term,
            long_term_idx: self.long_term_idx,
        }
    }
}
pub(crate) const LPAD: usize = 16;
pub(crate) const CPAD: usize = 8;
thread_local! { static MC_ROW_NEED : core :: cell :: Cell < usize > = const { core :: cell :: Cell :: new (usize :: MAX) } ; }
#[derive(Debug, Clone)]
pub(crate) struct FrozenPlanes {
    pub py: Vec<u8>,
    pub pu: Vec<u8>,
    pub pv: Vec<u8>,
}
#[derive(Debug)]
pub(crate) struct LivePlanes {
    pub py: crate::h264::decoder::sync::RwLock<Vec<u8>>,
    pub pu: crate::h264::decoder::sync::RwLock<Vec<u8>>,
    pub pv: crate::h264::decoder::sync::RwLock<Vec<u8>>,
    pub meta: crate::h264::decoder::sync::RwLock<LiveMeta>,
    pub wait: crate::h264::decoder::sync::Mutex<()>,
    pub cv: crate::h264::decoder::sync::Condvar,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct LiveMeta {
    pub frame_num: u32,
    pub poc: i32,
    pub long_term: bool,
    pub long_term_idx: u32,
    pub mv: Vec<(i32, i32)>,
    pub ref_idx: Vec<i32>,
    pub mv1: Vec<(i32, i32)>,
    pub ref_idx1: Vec<i32>,
    pub ref_poc: Vec<i32>,
    pub w4: usize,
    pub motion_ready: bool,
}
pub(crate) enum PlaneGuard<'a> {
    Borrowed(&'a [u8]),
    Locked(crate::h264::decoder::sync::RwLockReadGuard<'a, Vec<u8>>),
}
impl core::ops::Deref for PlaneGuard<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Self::Borrowed(s) => s,
            Self::Locked(g) => g.as_slice(),
        }
    }
}
impl RefFrame {
    #[inline]
    pub(crate) fn rows_needed_for_mb(mb_y: usize, ch: usize) -> usize {
        ((mb_y + 1) * 16 + LPAD).min(ch.max(1))
    }
    #[inline]
    pub(crate) fn set_mc_row_need(mb_y: usize, ch: usize) {
        if !crate::h264::decoder::row_progress_on() {
            return;
        }
        MC_ROW_NEED.with(|c| c.set(Self::rows_needed_for_mb(mb_y, ch)));
    }
    #[inline]
    fn effective_need(&self, need_rows: usize) -> usize {
        let hint = MC_ROW_NEED.with(|c| c.get());
        need_rows.min(hint).min(self.ch.max(1))
    }
    #[inline]
    pub(crate) fn luma_guard(&self, need_rows: usize) -> PlaneGuard<'_> {
        if self.live.is_none() {
            if let Some(f) = self.frozen.get() {
                return PlaneGuard::Borrowed(&f.py);
            }
            return PlaneGuard::Borrowed(&self.py);
        }
        let need = self.effective_need(need_rows);
        self.wait_ready_rows(need);
        if let Some(f) = self.frozen.get() {
            return PlaneGuard::Borrowed(&f.py);
        }
        if !self.py.is_empty() {
            return PlaneGuard::Borrowed(&self.py);
        }
        if let Some(live) = &self.live {
            PlaneGuard::Locked(live.py.read().unwrap())
        } else {
            PlaneGuard::Borrowed(&self.py)
        }
    }
    #[inline]
    pub(crate) fn chroma_guard(&self, plane: usize, need_rows: usize) -> PlaneGuard<'_> {
        if self.live.is_none() {
            if let Some(f) = self.frozen.get() {
                return if plane == 0 {
                    PlaneGuard::Borrowed(&f.pu)
                } else {
                    PlaneGuard::Borrowed(&f.pv)
                };
            }
            return if plane == 0 {
                PlaneGuard::Borrowed(&self.pu)
            } else {
                PlaneGuard::Borrowed(&self.pv)
            };
        }
        let need = self.effective_need(need_rows);
        self.wait_ready_rows(need);
        if let Some(f) = self.frozen.get() {
            return if plane == 0 {
                PlaneGuard::Borrowed(&f.pu)
            } else {
                PlaneGuard::Borrowed(&f.pv)
            };
        }
        if !self.py.is_empty() {
            return if plane == 0 {
                PlaneGuard::Borrowed(&self.pu)
            } else {
                PlaneGuard::Borrowed(&self.pv)
            };
        }
        if let Some(live) = &self.live {
            if plane == 0 {
                PlaneGuard::Locked(live.pu.read().unwrap())
            } else {
                PlaneGuard::Locked(live.pv.read().unwrap())
            }
        } else if plane == 0 {
            PlaneGuard::Borrowed(&self.pu)
        } else {
            PlaneGuard::Borrowed(&self.pv)
        }
    }

    #[inline]
    pub(crate) fn fn_num(&self) -> u32 {
        if let Some(live) = &self.live {
            live.meta.read().unwrap().frame_num
        } else {
            self.frame_num
        }
    }
    #[inline]
    pub(crate) fn pic_poc(&self) -> i32 {
        if let Some(live) = &self.live {
            live.meta.read().unwrap().poc
        } else {
            self.poc
        }
    }
    #[inline]
    pub(crate) fn is_long_term(&self) -> bool {
        if let Some(live) = &self.live {
            live.meta.read().unwrap().long_term
        } else {
            self.long_term
        }
    }
    #[inline]
    pub(crate) fn lt_idx(&self) -> u32 {
        if let Some(live) = &self.live {
            live.meta.read().unwrap().long_term_idx
        } else {
            self.long_term_idx
        }
    }
    pub(crate) fn set_long_term_marks(&self, long_term: bool, idx: u32) {
        if let Some(live) = &self.live {
            let mut m = live.meta.write().unwrap();
            m.long_term = long_term;
            m.long_term_idx = idx;
        }
    }
    pub(crate) fn set_frame_num_live(&self, frame_num: u32) {
        if let Some(live) = &self.live {
            live.meta.write().unwrap().frame_num = frame_num;
        }
    }
    pub(crate) fn wait_motion_ready(&self) {
        if self.live.is_none() {
            return;
        }
        if let Some(live) = &self.live {
            if live.meta.read().unwrap().motion_ready {
                return;
            }
        }
        if let Some(live) = &self.live {
            let mut g = live.wait.lock().unwrap();
            while !live.meta.read().unwrap().motion_ready {
                g = live.cv.wait(g).unwrap();
            }
        }
    }
    #[inline]
    pub fn mark_fully_ready(&self) {
        if let Some(live) = &self.live {
            let _g = live.wait.lock().unwrap();
            self.ready_rows
                .store(self.ch, core::sync::atomic::Ordering::Release);
            live.cv.notify_all();
        } else {
            self.ready_rows
                .store(self.ch, core::sync::atomic::Ordering::Release);
        }
    }
    #[inline]
    pub fn publish_ready_rows(&self, rows: usize) {
        let r = rows.min(self.ch);
        if let Some(live) = &self.live {
            let _g = live.wait.lock().unwrap();
            let prev = self
                .ready_rows
                .fetch_max(r, core::sync::atomic::Ordering::Release);
            if r > prev {
                live.cv.notify_all();
            }
        } else {
            let _ = self
                .ready_rows
                .fetch_max(r, core::sync::atomic::Ordering::Release);
        }
    }
    #[inline]
    pub fn wait_ready_rows(&self, rows: usize) {
        use core::sync::atomic::Ordering::Acquire;
        let need = rows.min(self.ch);
        if need == 0 || self.ready_rows.load(Acquire) >= need {
            return;
        }
        if self.frozen.get().is_some() {
            return;
        }
        if let Some(live) = &self.live {
            let mut g = live.wait.lock().unwrap();
            while self.ready_rows.load(Acquire) < need {
                g = live.cv.wait(g).unwrap();
            }
        } else {
            while self.ready_rows.load(Acquire) < need {
                crate::h264::decoder::sync::yield_now();
            }
        }
    }
    #[inline]
    pub fn lstride(&self) -> usize {
        self.cw + 2 * LPAD
    }
    #[inline]
    pub fn cstride(&self) -> usize {
        self.cw / 2 + 2 * CPAD
    }
    #[inline]
    pub fn lrows(&self) -> usize {
        self.ch + 2 * LPAD
    }
    #[inline]
    pub fn crows(&self) -> usize {
        self.ch / 2 + 2 * CPAD
    }
}
#[derive(Clone, Copy)]
enum Mmco {
    Unref(u32),
    UnrefLong(u32),
    AssignLong(u32, u32),
    MaxLong(u32),
    Reset,
    CurrentLong(u32),
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContentRoute {
    Light,
    Mid,
    DenseInter,
    EntropyExtreme,
}
fn route_for(
    cabac: bool,
    t8x8: bool,
    bits_per_mb: f64,
    skip_frac: f64,
    coded_frac: f64,
) -> ContentRoute {
    let (root_coded, light_skip, extreme_bits) = if !cabac {
        (0.6116, 0.6433, 406.4)
    } else if t8x8 {
        (0.4244, 0.6358, 331.7)
    } else {
        (0.3709, 0.6343, 315.2)
    };
    if coded_frac <= root_coded {
        if skip_frac > light_skip {
            ContentRoute::Light
        } else {
            ContentRoute::Mid
        }
    } else if bits_per_mb <= extreme_bits {
        ContentRoute::DenseInter
    } else {
        ContentRoute::EntropyExtreme
    }
}
struct PendingPic {
    fd: mb16::FrameDecoder,
    frame_num: u32,
    poc: i32,
    next_mb: usize,
    total_mb: usize,
    slice_count: u16,
    deblock: bool,
    filter_offset_a: i32,
    filter_offset_b: i32,
    crop_r: usize,
    crop_b: usize,
    max_refs: usize,
    log2_max_frame_num: u32,
    is_reference: bool,
    idr_long_term: bool,
    mmco_ops: Vec<Mmco>,
    route_bits: u64,
    route_cabac: bool,
    route_t8x8: bool,
}
fn no_pool() -> bool {
    {
        return false;
    }
}
#[derive(Default)]
pub struct Decoder {
    last_route: Option<ContentRoute>,
    route_ema: Option<(f64, f64, f64)>,
    pub(crate) sps: std::collections::BTreeMap<u32, crate::h264::decoder::sync::Arc<Sps>>,
    pub(crate) pps: std::collections::BTreeMap<u32, crate::h264::decoder::sync::Arc<Pps>>,
    pub(crate) refs: Vec<Ref>,
    cur: Option<PendingPic>,
    pub(crate) poc: PocState,
    pub(crate) last_poc: i32,
    pub(crate) prev_ref_frame_num: u32,
    grid_pool: GridPool,
    sc_reorder0: Vec<(u32, u32)>,
    sc_reorder1: Vec<(u32, u32)>,
    sc_mmco: Vec<Mmco>,
    plane_pool: Vec<Vec<u8>>,
    retired: Vec<Ref>,
    pub(crate) detach_dpb: bool,
    pub(crate) detached_ref: Option<Ref>,
    pub(crate) progress_slot: Option<Ref>,
    detached_mmco: Vec<Mmco>,
    detached_frame_num: u32,
    detached_log2_max_frame_num: u32,
    detached_max_refs: usize,
    detached_idr_long_term: bool,
    pub(crate) frame_mt_row_progress: bool,
}
#[derive(Clone, Default)]
pub(crate) struct PocState {
    prev_msb: i32,
    prev_lsb: i32,
    prev_frame_num: u32,
    prev_frame_num_offset: i64,
}
impl PocState {
    pub(crate) fn compute_poc(
        &mut self,
        sps: &Sps,
        is_idr: bool,
        nal_ref_idc: u8,
        frame_num: u32,
        poc_lsb: u32,
        delta_bottom: i32,
    ) -> i32 {
        match sps.pic_order_cnt_type {
            0 => {
                let max_lsb = 1i32 << sps.log2_max_pic_order_cnt_lsb;
                let (prev_msb, prev_lsb) = if is_idr {
                    (0, 0)
                } else {
                    (self.prev_msb, self.prev_lsb)
                };
                let lsb = poc_lsb as i32;
                let msb = if lsb < prev_lsb && prev_lsb - lsb >= max_lsb / 2 {
                    prev_msb + max_lsb
                } else if lsb > prev_lsb && lsb - prev_lsb > max_lsb / 2 {
                    prev_msb - max_lsb
                } else {
                    prev_msb
                };
                let top = msb + lsb;
                let poc = top.min(top + delta_bottom);
                if nal_ref_idc != 0 {
                    self.prev_msb = msb;
                    self.prev_lsb = lsb;
                }
                poc
            }
            2 => {
                let max_fn = 1i64 << sps.log2_max_frame_num;
                let offset = if is_idr {
                    0
                } else if self.prev_frame_num > frame_num {
                    self.prev_frame_num_offset + max_fn
                } else {
                    self.prev_frame_num_offset
                };
                let poc = if is_idr {
                    0
                } else {
                    2 * (offset + frame_num as i64) - i64::from(nal_ref_idc == 0)
                };
                self.prev_frame_num_offset = offset;
                self.prev_frame_num = frame_num;
                poc as i32
            }
            _ => {
                self.prev_frame_num = frame_num;
                frame_num as i32 * 2
            }
        }
    }
}
impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn decode(&mut self, annex_b: &[u8]) -> Result<Option<YuvFrame>, DecodeError> {
        let _g = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Total);
        let mut frame = None;
        let nals = {
            let _s =
                crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecNalSplit);
            split_annex_b(annex_b)
        };
        for nal in nals {
            if nal.is_empty() {
                continue;
            }
            let nal_type = NalUnitType::from_id(nal[0]);
            let rbsp = {
                let _s =
                    crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecRbsp);
                emulation_unprevent(&nal[1..])
            };
            match nal_type {
                NalUnitType::Sps => {
                    let s = Sps::parse(&rbsp)?;
                    self.sps.insert(
                        s.seq_parameter_set_id,
                        crate::h264::decoder::sync::Arc::new(s),
                    );
                }
                NalUnitType::Pps => {
                    let p = Pps::parse(&rbsp)?;
                    self.pps.insert(
                        p.pic_parameter_set_id,
                        crate::h264::decoder::sync::Arc::new(p),
                    );
                }
                NalUnitType::IdrSlice | NalUnitType::NonIdrSlice => {
                    let nal_ref_idc = (nal[0] >> 5) & 3;
                    let is_idr = nal_type == NalUnitType::IdrSlice;
                    if let Some(f) = self.decode_slice(&rbsp, is_idr, nal_ref_idc)? {
                        frame = Some(f);
                    }
                }
                _ => {}
            }
        }
        Ok(frame)
    }

    fn decode_slice(
        &mut self,
        rbsp: &[u8],
        is_idr: bool,
        nal_ref_idc: u8,
    ) -> Result<Option<YuvFrame>, DecodeError> {
        let mut r = BitReader::new(rbsp);
        let first_mb_in_slice = r.read_ue()? as usize;
        let slice_type = r.read_ue()?;
        let is_p = matches!(slice_type, 0 | 5);
        let is_b = matches!(slice_type, 1 | 6);
        let is_i = matches!(slice_type, 2 | 7);
        if !is_p && !is_b && !is_i {
            return Err(DecodeError::Unsupported("SP/SI slices"));
        }
        let pic_parameter_set_id = r.read_ue()?;
        let pps = self
            .pps
            .get(&pic_parameter_set_id)
            .cloned()
            .ok_or(DecodeError::MissingParameterSet)?;
        let sps = self
            .sps
            .get(&pps.seq_parameter_set_id)
            .cloned()
            .ok_or(DecodeError::MissingParameterSet)?;
        let sps = &sps;
        let pps = &pps;
        let cabac = pps.entropy_coding_mode_flag;
        let frame_num = r.read_bits(sps.log2_max_frame_num)?;
        if is_idr {
            let _idr_pic_id = r.read_ue()?;
        }
        let mut poc_lsb = 0u32;
        let mut delta_poc_bottom = 0i32;
        if sps.pic_order_cnt_type == 0 {
            poc_lsb = r.read_bits(sps.log2_max_pic_order_cnt_lsb)?;
            if pps.bottom_field_pic_order_present {
                delta_poc_bottom = r.read_se()?;
            }
        } else if sps.pic_order_cnt_type == 1 && !sps.delta_pic_order_always_zero {
            let _delta_pic_order_cnt_0 = r.read_se()?;
            if pps.bottom_field_pic_order_present {
                let _delta_pic_order_cnt_1 = r.read_se()?;
            }
        }
        let pic_poc = if first_mb_in_slice == 0 {
            self.compute_poc(
                sps,
                is_idr,
                nal_ref_idc,
                frame_num,
                poc_lsb,
                delta_poc_bottom,
            )
        } else {
            self.cur.as_ref().map_or(0, |p| p.poc)
        };
        if pps.redundant_pic_cnt_present_flag {
            let redundant_pic_cnt = r.read_ue()?;
            if redundant_pic_cnt != 0 {
                return Ok(None);
            }
        }
        if crate::h264::decoder::mb16::dump_mb_on() {
            eprintln!(
                "SLICE fn={frame_num} poc={pic_poc} nal_ref_idc={nal_ref_idc} is_p={is_p} is_b={is_b} first_mb={first_mb_in_slice}"
            );
        }
        let direct_spatial = if is_b { r.read_bit()? } else { true };
        let mut num_ref_idx_l0 = pps.num_ref_idx_l0_default as usize;
        let mut num_ref_idx_l1 = pps.num_ref_idx_l1_default as usize;
        let mut reorder_l0: Vec<(u32, u32)> = core::mem::take(&mut self.sc_reorder0);
        reorder_l0.clear();
        let mut reorder_l1: Vec<(u32, u32)> = core::mem::take(&mut self.sc_reorder1);
        reorder_l1.clear();
        if is_p || is_b {
            if r.read_bit()? {
                num_ref_idx_l0 = (r.read_ue()? + 1) as usize;
                if is_b {
                    num_ref_idx_l1 = (r.read_ue()? + 1) as usize;
                }
            }
            if r.read_bit()? {
                parse_ref_pic_list_modification(&mut r, &mut reorder_l0)?;
            }
            if is_b && r.read_bit()? {
                parse_ref_pic_list_modification(&mut r, &mut reorder_l1)?;
            }
        }
        let weights = if is_p && pps.weighted_pred {
            Some(parse_pred_weight_table(&mut r, num_ref_idx_l0, 0, false)?)
        } else if is_b && pps.weighted_bipred_idc == 1 {
            return Err(DecodeError::Unsupported("explicit B weighted prediction"));
        } else {
            None
        };
        let mut idr_long_term = false;
        let mut mmco_ops: Vec<Mmco> = core::mem::take(&mut self.sc_mmco);
        mmco_ops.clear();
        if nal_ref_idc == 0 {
        } else if is_idr {
            let _no_output_of_prior_pics = r.read_bit()?;
            idr_long_term = r.read_bit()?;
        } else if r.read_bit()? {
            loop {
                let op = r.read_ue()?;
                match op {
                    0 => break,
                    1 => mmco_ops.push(Mmco::Unref(r.read_ue()?)),
                    2 => mmco_ops.push(Mmco::UnrefLong(r.read_ue()?)),
                    3 => {
                        let diff = r.read_ue()?;
                        let idx = r.read_ue()?;
                        mmco_ops.push(Mmco::AssignLong(diff, idx));
                    }
                    4 => mmco_ops.push(Mmco::MaxLong(r.read_ue()?)),
                    5 => mmco_ops.push(Mmco::Reset),
                    6 => mmco_ops.push(Mmco::CurrentLong(r.read_ue()?)),
                    _ => return Err(DecodeError::Unsupported("invalid MMCO")),
                }
                if mmco_ops.len() > 128 {
                    return Err(DecodeError::Truncated);
                }
            }
        }
        let cabac_init_idc = if cabac && !is_i {
            let v = r.read_ue()?;
            if v > 2 {
                return Err(DecodeError::Unsupported("invalid cabac_init_idc"));
            }
            v
        } else {
            0
        };
        let slice_qp_delta = r.read_se()?;
        let mut deblock = true;
        let mut deblock_idc2 = false;
        let (mut filter_offset_a, mut filter_offset_b) = (0i32, 0i32);
        if pps.deblocking_filter_control_present_flag {
            let disable_deblocking_filter_idc = r.read_ue()?;
            deblock = disable_deblocking_filter_idc != 1;
            deblock_idc2 = disable_deblocking_filter_idc == 2;
            if disable_deblocking_filter_idc != 1 {
                filter_offset_a = r.read_se()? * 2;
                filter_offset_b = r.read_se()? * 2;
            }
        }
        let slice_qp = (pps.pic_init_qp + slice_qp_delta).clamp(0, 51) as u8;
        if first_mb_in_slice == 0 && !is_idr && sps.gaps_in_frame_num_allowed {
            self.insert_frame_num_gaps(
                frame_num,
                1u32 << sps.log2_max_frame_num,
                sps.max_num_ref_frames.max(1) as usize,
                sps.pic_width_in_mbs * 16,
                sps.pic_height_in_mbs * 16,
            );
        }
        let max_fn = 1u32 << sps.log2_max_frame_num;
        let (ref_list0, ref_list1) = if is_b {
            build_ref_list_b(
                &self.refs,
                pic_poc,
                frame_num,
                max_fn,
                num_ref_idx_l0,
                num_ref_idx_l1,
                &reorder_l0,
                &reorder_l1,
            )?
        } else if is_p {
            (
                build_ref_list_p(&self.refs, frame_num, max_fn, num_ref_idx_l0, &reorder_l0)?,
                Vec::new(),
            )
        } else {
            (Vec::new(), Vec::new())
        };
        self.sc_reorder0 = reorder_l0;
        self.sc_reorder1 = reorder_l1;
        if first_mb_in_slice == 0 {
            if is_idr {
                self.refs.clear();
            }
            let _g_setup =
                crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DecSetup);
            let mut fd = FrameDecoder::with_pool(
                sps.pic_width_in_mbs,
                sps.pic_height_in_mbs,
                slice_qp,
                pps.chroma_qp_index_offset,
                ref_list0,
                num_ref_idx_l0,
                pps.constrained_intra_pred_flag,
                pps.transform_8x8_mode_flag,
                sps.profile_idc != 66,
                if no_pool() {
                    GridPool::default()
                } else {
                    core::mem::take(&mut self.grid_pool)
                },
            );
            if is_b {
                fd.set_b_context(
                    ref_list1,
                    num_ref_idx_l1,
                    direct_spatial,
                    pic_poc,
                    pps.weighted_bipred_idc,
                    sps.direct_8x8_inference,
                );
            }
            if sps.has_scaling || pps.pic_scaling_matrix_present {
                let (s4, s8) = resolve_scaling(sps, pps);
                fd.set_scaling(s4, s8);
            }
            fd.set_transform_bypass(sps.transform_bypass);
            if let Some(w) = weights {
                fd.set_weights(w);
            }
            if let Some(slot) = self.progress_slot.clone() {
                fd.set_progress_slot(slot);
            }
            if let Some(prev) = self.cur.take() {
                if prev.next_mb < prev.total_mb {
                    return Err(DecodeError::Truncated);
                }
            }
            self.cur = Some(PendingPic {
                fd,
                frame_num,
                poc: pic_poc,
                next_mb: 0,
                route_bits: 0,
                route_cabac: pps.entropy_coding_mode_flag,
                route_t8x8: pps.transform_8x8_mode_flag,
                total_mb: sps.pic_width_in_mbs * sps.pic_height_in_mbs,
                slice_count: 0,
                deblock,
                filter_offset_a,
                filter_offset_b,
                crop_r: sps.frame_crop_right as usize,
                crop_b: sps.frame_crop_bottom as usize,
                max_refs: sps.max_num_ref_frames.max(1) as usize,
                log2_max_frame_num: sps.log2_max_frame_num,
                is_reference: nal_ref_idc != 0,
                idr_long_term,
                mmco_ops,
            });
        } else {
            let Some(pic) = self.cur.as_mut() else {
                return Err(DecodeError::Unsupported(
                    "slice continues a missing picture",
                ));
            };
            pic.fd.begin_slice(slice_qp, ref_list0, num_ref_idx_l0);
            if is_b {
                pic.fd.set_b_context(
                    ref_list1,
                    num_ref_idx_l1,
                    direct_spatial,
                    pic.poc,
                    pps.weighted_bipred_idc,
                    sps.direct_8x8_inference,
                );
            }
            if sps.has_scaling || pps.pic_scaling_matrix_present {
                let (s4, s8) = resolve_scaling(sps, pps);
                pic.fd.set_scaling(s4, s8);
            }
            pic.fd.set_transform_bypass(sps.transform_bypass);
            if let Some(w) = weights {
                pic.fd.set_weights(w);
            }
            pic.deblock = deblock;
            pic.filter_offset_a = filter_offset_a;
            pic.filter_offset_b = filter_offset_b;
            pic.idr_long_term |= idr_long_term;
            pic.mmco_ops.extend(mmco_ops);
        }
        let pic = self.cur.as_mut().expect("pending picture set above");
        pic.fd.set_deblock_params(
            deblock && !abl_deblock(),
            filter_offset_a,
            filter_offset_b,
            deblock_idc2,
        );
        let first = first_mb_in_slice.min(pic.total_mb);
        pic.route_bits += r.data().len() as u64;
        let next = if cabac {
            r.align_to_byte().map_err(|_| DecodeError::Truncated)?;
            let (data, start) = (r.data(), r.bit_pos() / 8);
            pic.fd
                .decode_slice_data_cabac(data, start, slice_qp, cabac_init_idc, is_i, is_p, first)
        } else {
            pic.fd.decode_slice_data(&mut r, is_p, first)
        }
        .map_err(|e| match e {
            mb16::MbError::Truncated => DecodeError::Truncated,
            mb16::MbError::Unsupported(s) => DecodeError::Unsupported(s),
        })?;
        pic.next_mb = next;
        pic.slice_count += 1;
        if crate::h264::decoder::mb16::dump_mb_on() {
            eprintln!(
                "  slice decoded {}/{} MBs{}",
                next,
                pic.total_mb,
                if next < pic.total_mb {
                    "   <-- INCOMPLETE"
                } else {
                    ""
                }
            );
        }
        if pic.next_mb < pic.total_mb {
            return Ok(None);
        }
        let pic = self.cur.take().expect("pending picture");
        {
            let (skips, coded) = pic.fd.route_counters();
            let mbs = pic.total_mb.max(1) as f64;
            let bits_per_mb = pic.route_bits as f64 * 8.0 / mbs;
            let (skip_frac, coded_frac) = (skips as f64 / mbs, coded as f64 / mbs);
            let (eb, es, ec) = match self.route_ema {
                None => (bits_per_mb, skip_frac, coded_frac),
                Some((pb, ps, pc)) => (
                    pb + (bits_per_mb - pb) / 8.0,
                    ps + (skip_frac - ps) / 8.0,
                    pc + (coded_frac - pc) / 8.0,
                ),
            };
            self.route_ema = Some((eb, es, ec));
            let route = route_for(pic.route_cabac, pic.route_t8x8, eb, es, ec);
            self.last_route = Some(route);
            if crate::h264::decoder::mb16::route_dump_on() {
                eprintln!(
                    "ROUTE cabac={} t8x8={} bits_per_mb={bits_per_mb:.2} skip_frac={skip_frac:.4} coded_frac={coded_frac:.4} ema=({eb:.2},{es:.4},{ec:.4}) -> {route:?}",
                    pic.route_cabac, pic.route_t8x8
                );
            }
        }
        let PendingPic {
            mut fd,
            frame_num,
            poc,
            deblock,
            filter_offset_a,
            filter_offset_b,
            crop_r,
            crop_b,
            max_refs,
            log2_max_frame_num,
            is_reference,
            idr_long_term,
            mut mmco_ops,
            ..
        } = pic;
        self.last_poc = poc;
        if deblock && !abl_deblock() {
            fd.deblock(filter_offset_a, filter_offset_b);
        }
        let reference = if is_reference {
            let _dg = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::DpbClone);
            Some(fd.as_reference_pooled(&mut self.plane_pool))
        } else {
            None
        };
        let _fg = crate::h264::common::prof::scope(crate::h264::common::prof::Stage::Finalize);
        if let Some(mut reference) = reference {
            reference.frame_num = frame_num;
            reference.poc = poc;
            if crate::h264::decoder::mb16::dump_mb_on() {
                eprintln!("DPB-ADD fn={frame_num} poc={poc}");
            }
            if idr_long_term {
                reference.long_term = true;
                reference.long_term_idx = 0;
            }
            let reference = reference;
            if self.detach_dpb {
                self.detached_mmco = core::mem::take(&mut mmco_ops);
                self.detached_frame_num = frame_num;
                self.detached_log2_max_frame_num = log2_max_frame_num;
                self.detached_max_refs = max_refs;
                self.detached_idr_long_term = idr_long_term;
                if let Some(slot) = self.progress_slot.take() {
                    Self::fill_progress_slot(&slot, reference);
                    slot.mark_fully_ready();
                    self.detached_ref = Some(slot);
                } else {
                    let arc = crate::h264::decoder::sync::Arc::new(reference);
                    arc.mark_fully_ready();
                    self.detached_ref = Some(arc);
                }
            } else {
                reference.mark_fully_ready();
                self.prev_ref_frame_num = self.apply_ref_marking(
                    reference,
                    &mmco_ops,
                    frame_num,
                    log2_max_frame_num,
                    max_refs,
                );
            }
        }
        self.sc_mmco = mmco_ops;
        let (frame, pool) = fd.into_frame_recycle(crop_r, crop_b);
        self.grid_pool = pool;
        self.reclaim_retired();
        Ok(Some(frame))
    }

    fn fill_progress_slot(slot: &Ref, mut finished: crate::h264::decoder::RefFrame) {
        let planes = FrozenPlanes {
            py: core::mem::take(&mut finished.py),
            pu: core::mem::take(&mut finished.pu),
            pv: core::mem::take(&mut finished.pv),
        };
        if let Some(live) = &slot.live {
            let _w = live.wait.lock().unwrap();
            {
                let mut m = live.meta.write().unwrap();
                m.frame_num = finished.frame_num;
                m.poc = finished.poc;
                m.long_term = finished.long_term;
                m.long_term_idx = finished.long_term_idx;
                m.mv = core::mem::take(&mut finished.mv);
                m.ref_idx = core::mem::take(&mut finished.ref_idx);
                m.mv1 = core::mem::take(&mut finished.mv1);
                m.ref_idx1 = core::mem::take(&mut finished.ref_idx1);
                m.ref_poc = core::mem::take(&mut finished.ref_poc);
                m.w4 = finished.w4;
                m.motion_ready = true;
            }
            let _ = slot.frozen.set(planes);
            slot.ready_rows
                .store(slot.ch, core::sync::atomic::Ordering::Release);
            live.cv.notify_all();
        } else {
            let _ = slot.frozen.set(planes);
            slot.mark_fully_ready();
        }
    }
    fn reclaim_retired(&mut self) {
        for arc in self.retired.drain(..) {
            if let Ok(mut rf) = crate::h264::decoder::sync::Arc::try_unwrap(arc) {
                if let Some(f) = rf.frozen.take() {
                    if !f.py.is_empty() {
                        self.plane_pool.push(f.py);
                        self.plane_pool.push(f.pu);
                        self.plane_pool.push(f.pv);
                    }
                } else if !rf.py.is_empty() {
                    self.plane_pool.push(rf.py);
                    self.plane_pool.push(rf.pu);
                    self.plane_pool.push(rf.pv);
                }
            }
        }
        self.plane_pool.truncate(18);
    }
    fn insert_frame_num_gaps(
        &mut self,
        frame_num: u32,
        max_fn: u32,
        max_refs: usize,
        w: usize,
        h: usize,
    ) {
        if max_fn == 0 {
            return;
        }
        let start = (self.prev_ref_frame_num + 1) % max_fn;
        let gap = (frame_num + max_fn - start) % max_fn;
        if gap == 0 {
            return;
        }
        let cap = max_refs.max(1);
        let n = (gap as usize).min(cap);
        let (cw, ch) = (w, h);
        let mut expected = (frame_num + max_fn - n as u32) % max_fn;
        for _ in 0..n {
            self.refs.insert(
                0,
                crate::h264::decoder::sync::Arc::new(RefFrame {
                    py: vec![128; (cw + 2 * LPAD) * (ch + 2 * LPAD)],
                    pu: vec![128; (cw / 2 + 2 * CPAD) * (ch / 2 + 2 * CPAD)],
                    pv: vec![128; (cw / 2 + 2 * CPAD) * (ch / 2 + 2 * CPAD)],
                    cw,
                    ch,
                    ready_rows: core::sync::atomic::AtomicUsize::new(ch),
                    live: None,
                    frozen: crate::h264::decoder::sync::Once::new(),
                    frame_num: expected,
                    poc: 0,
                    mv: Vec::new(),
                    ref_idx: Vec::new(),
                    mv1: Vec::new(),
                    ref_idx1: Vec::new(),
                    ref_poc: Vec::new(),
                    w4: 0,
                    long_term: false,
                    long_term_idx: 0,
                }),
            );
            self.refs.truncate(cap);
            expected = (expected + 1) % max_fn;
        }
        self.prev_ref_frame_num = (frame_num + max_fn - 1) % max_fn;
    }

    fn compute_poc(
        &mut self,
        sps: &Sps,
        is_idr: bool,
        nal_ref_idc: u8,
        frame_num: u32,
        poc_lsb: u32,
        delta_bottom: i32,
    ) -> i32 {
        self.poc
            .compute_poc(sps, is_idr, nal_ref_idc, frame_num, poc_lsb, delta_bottom)
    }
    fn apply_ref_marking(
        &mut self,
        mut reference: RefFrame,
        ops: &[Mmco],
        frame_num: u32,
        log2_max_frame_num: u32,
        max_refs: usize,
    ) -> u32 {
        let max = 1i64 << log2_max_frame_num;
        let curr = frame_num as i64;
        let pic_num = |rf: &RefFrame| -> i64 {
            if (rf.frame_num as i64) > curr {
                rf.frame_num as i64 - max
            } else {
                rf.frame_num as i64
            }
        };
        if ops.is_empty() {
            let out_fn = reference.frame_num;
            self.refs
                .insert(0, crate::h264::decoder::sync::Arc::new(reference));
            while self.refs.len() > max_refs {
                match self.refs.iter().rposition(|r| !r.long_term) {
                    Some(pos) => {
                        let evicted = self.refs.remove(pos);
                        self.retired.push(evicted);
                    }
                    None => break,
                }
            }
            return out_fn;
        }
        for &op in ops {
            match op {
                Mmco::Unref(diff) => {
                    let target = curr - (diff as i64 + 1);
                    self.refs.retain(|r| r.long_term || pic_num(r) != target);
                }
                Mmco::UnrefLong(ltpn) => {
                    self.refs
                        .retain(|r| !(r.long_term && r.long_term_idx == ltpn));
                }
                Mmco::AssignLong(diff, idx) => {
                    let target = curr - (diff as i64 + 1);
                    self.refs
                        .retain(|r| !(r.long_term && r.long_term_idx == idx));
                    for r in self.refs.iter_mut() {
                        if !r.long_term && pic_num(r) == target {
                            let r = crate::h264::decoder::sync::Arc::make_mut(r);
                            r.long_term = true;
                            r.long_term_idx = idx;
                        }
                    }
                }
                Mmco::MaxLong(max_plus1) => {
                    self.refs
                        .retain(|r| !(r.long_term && r.long_term_idx + 1 > max_plus1));
                }
                Mmco::Reset => {
                    self.refs.clear();
                    reference.frame_num = 0;
                }
                Mmco::CurrentLong(idx) => {
                    self.refs
                        .retain(|r| !(r.long_term && r.long_term_idx == idx));
                    reference.long_term = true;
                    reference.long_term_idx = idx;
                }
            }
        }
        let out_fn = reference.frame_num;
        self.refs
            .insert(0, crate::h264::decoder::sync::Arc::new(reference));
        let cap = max_refs.max(16);
        if self.refs.len() > cap {
            self.refs.truncate(cap);
        }
        out_fn
    }
    fn apply_ref_marking_arc(
        &mut self,
        mut reference: Ref,
        ops: &[Mmco],
        frame_num: u32,
        log2_max_frame_num: u32,
        max_refs: usize,
    ) -> u32 {
        let max = 1i64 << log2_max_frame_num;
        let curr = frame_num as i64;
        let pic_num = |rf: &RefFrame| -> i64 {
            let f = rf.fn_num() as i64;
            if f > curr { f - max } else { f }
        };
        if ops.is_empty() {
            let out_fn = reference.fn_num();
            self.refs.insert(0, reference);
            while self.refs.len() > max_refs {
                match self.refs.iter().rposition(|r| !r.is_long_term()) {
                    Some(pos) => {
                        let evicted = self.refs.remove(pos);
                        self.retired.push(evicted);
                    }
                    None => break,
                }
            }
            return out_fn;
        }
        for &op in ops {
            match op {
                Mmco::Unref(diff) => {
                    let target = curr - (diff as i64 + 1);
                    self.refs
                        .retain(|r| r.is_long_term() || pic_num(r) != target);
                }
                Mmco::UnrefLong(ltpn) => {
                    self.refs
                        .retain(|r| !(r.is_long_term() && r.lt_idx() == ltpn));
                }
                Mmco::AssignLong(diff, idx) => {
                    let target = curr - (diff as i64 + 1);
                    self.refs
                        .retain(|r| !(r.is_long_term() && r.lt_idx() == idx));
                    for r in self.refs.iter_mut() {
                        if !r.is_long_term() && pic_num(r) == target {
                            if r.live.is_some() {
                                r.set_long_term_marks(true, idx);
                            } else {
                                let r = crate::h264::decoder::sync::Arc::make_mut(r);
                                r.long_term = true;
                                r.long_term_idx = idx;
                            }
                        }
                    }
                }
                Mmco::MaxLong(max_plus1) => {
                    self.refs
                        .retain(|r| !(r.is_long_term() && r.lt_idx() + 1 > max_plus1));
                }
                Mmco::Reset => {
                    self.refs.clear();
                    reference.set_frame_num_live(0);
                    if let Some(r) = crate::h264::decoder::sync::Arc::get_mut(&mut reference) {
                        r.frame_num = 0;
                    }
                }
                Mmco::CurrentLong(idx) => {
                    self.refs
                        .retain(|r| !(r.is_long_term() && r.lt_idx() == idx));
                    reference.set_long_term_marks(true, idx);
                    if let Some(r) = crate::h264::decoder::sync::Arc::get_mut(&mut reference) {
                        r.long_term = true;
                        r.long_term_idx = idx;
                    }
                }
            }
        }
        let out_fn = reference.fn_num();
        self.refs.insert(0, reference);
        let cap = max_refs.max(16);
        if self.refs.len() > cap {
            self.refs.truncate(cap);
        }
        out_fn
    }
}

fn parse_pred_weight_table(
    r: &mut BitReader,
    num_l0: usize,
    num_l1: usize,
    is_b: bool,
) -> Result<WeightTable, DecodeError> {
    let luma_log2_denom = r.read_ue()? as i32;
    let chroma_log2_denom = r.read_ue()? as i32;
    if !(0..=7).contains(&luma_log2_denom) || !(0..=7).contains(&chroma_log2_denom) {
        return Err(DecodeError::Unsupported("invalid weight denom"));
    }
    let mut wt = WeightTable {
        luma_log2_denom,
        chroma_log2_denom,
        ..Default::default()
    };
    let lists: &[(usize, usize)] = if is_b {
        &[(0, num_l0), (1, num_l1)]
    } else {
        &[(0, num_l0)]
    };
    for &(list, n) in lists {
        let mut luma = Vec::with_capacity(n);
        let mut chroma = Vec::with_capacity(n);
        for _ in 0..n {
            let (mut lw, mut lo) = (1 << luma_log2_denom, 0);
            if r.read_bit()? {
                lw = r.read_se()?;
                lo = r.read_se()?;
            }
            luma.push((lw, lo));
            let mut ch = [(1 << chroma_log2_denom, 0); 2];
            if r.read_bit()? {
                for slot in ch.iter_mut() {
                    *slot = (r.read_se()?, r.read_se()?);
                }
            }
            chroma.push(ch);
        }
        wt.luma[list & 1] = luma;
        wt.chroma[list & 1] = chroma;
    }
    Ok(wt)
}
fn resolve_scaling(sps: &Sps, pps: &Pps) -> ([[i32; 16]; 6], [[i32; 64]; 2]) {
    use crate::h264::decoder::params::{
        DEFAULT_4X4_INTER, DEFAULT_4X4_INTRA, DEFAULT_8X8_INTER, DEFAULT_8X8_INTRA,
    };
    const ZZ4: [usize; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];
    const ZZ8: [usize; 64] = [
        0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27,
        20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51,
        58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
    ];
    let mut z4 = [[16u8; 16]; 6];
    for i in 0..6 {
        z4[i] = if pps.pic_scaling_matrix_present {
            if pps.scaling_present_4x4[i] {
                pps.scaling_4x4[i]
            } else {
                match i {
                    0 if sps.has_scaling => sps.scaling_4x4[0],
                    0 => DEFAULT_4X4_INTRA,
                    3 if sps.has_scaling => sps.scaling_4x4[3],
                    3 => DEFAULT_4X4_INTER,
                    _ => z4[i - 1],
                }
            }
        } else {
            sps.scaling_4x4[i]
        };
    }
    let mut z8 = [[16u8; 64]; 2];
    for (i, list) in z8.iter_mut().enumerate() {
        *list = if pps.pic_scaling_matrix_present {
            if pps.scaling_present_8x8[i] {
                pps.scaling_8x8[i]
            } else if sps.has_scaling {
                sps.scaling_8x8[i]
            } else if i == 0 {
                DEFAULT_8X8_INTRA
            } else {
                DEFAULT_8X8_INTER
            }
        } else {
            sps.scaling_8x8[i]
        };
    }
    let mut out4 = [[16i32; 16]; 6];
    for (li, list) in out4.iter_mut().enumerate() {
        for k in 0..16 {
            list[ZZ4[k]] = z4[li][k] as i32;
        }
    }
    let mut out8 = [[16i32; 64]; 2];
    for (li, list) in out8.iter_mut().enumerate() {
        for k in 0..64 {
            list[ZZ8[k]] = z8[li][k] as i32;
        }
    }
    (out4, out8)
}
fn parse_ref_pic_list_modification(
    r: &mut BitReader,
    out: &mut Vec<(u32, u32)>,
) -> Result<(), DecodeError> {
    loop {
        let idc = r.read_ue()?;
        if idc == 3 {
            break;
        }
        if idc > 3 {
            return Err(DecodeError::Unsupported(
                "invalid ref_pic_list_modification",
            ));
        }
        let val = r.read_ue()?;
        out.push((idc, val));
        if out.len() > 64 {
            return Err(DecodeError::Truncated);
        }
    }
    Ok(())
}
fn build_ref_list_p(
    dpb: &[Ref],
    curr_frame_num: u32,
    max_frame_num: u32,
    num_active: usize,
    mods: &[(u32, u32)],
) -> Result<Vec<Ref>, DecodeError> {
    let curr = curr_frame_num as i64;
    let max = max_frame_num as i64;
    let pic_num = |fnum: u32| -> i64 {
        let f = fnum as i64;
        if f > curr { f - max } else { f }
    };
    let mut init: Vec<Ref> = dpb.iter().filter(|r| !r.is_long_term()).cloned().collect();
    init.sort_by_key(|rf| core::cmp::Reverse(pic_num(rf.fn_num())));
    let mut long: Vec<Ref> = dpb.iter().filter(|r| r.is_long_term()).cloned().collect();
    long.sort_by_key(|rf| rf.lt_idx());
    init.extend(long);
    apply_list_modification(init, curr_frame_num, max_frame_num, num_active, mods)
}
fn build_ref_list_b(
    dpb: &[Ref],
    curr_poc: i32,
    curr_frame_num: u32,
    max_frame_num: u32,
    num0: usize,
    num1: usize,
    mods0: &[(u32, u32)],
    mods1: &[(u32, u32)],
) -> Result<(Vec<Ref>, Vec<Ref>), DecodeError> {
    let mut less: Vec<Ref> = dpb
        .iter()
        .filter(|r| !r.is_long_term() && r.pic_poc() < curr_poc)
        .cloned()
        .collect();
    let mut greater: Vec<Ref> = dpb
        .iter()
        .filter(|r| !r.is_long_term() && r.pic_poc() > curr_poc)
        .cloned()
        .collect();
    let mut long: Vec<Ref> = dpb.iter().filter(|r| r.is_long_term()).cloned().collect();
    less.sort_by_key(|r| core::cmp::Reverse(r.pic_poc()));
    greater.sort_by_key(|r| r.pic_poc());
    long.sort_by_key(|r| r.lt_idx());
    let mut init0 = less.clone();
    init0.extend(greater.clone());
    init0.extend(long.clone());
    let mut init1 = greater;
    init1.extend(less);
    init1.extend(long);
    let eq_len = num0.min(num1).min(init0.len()).min(init1.len());
    if num1 > 1
        && init1.len() > 1
        && (0..eq_len).all(|i| same_picture(&init0[i], &init1[i]))
        && eq_len == num1.min(init1.len())
        && eq_len == num0.min(init0.len())
    {
        if let [a, b, ..] = &mut init1[..] {
            core::mem::swap(a, b);
        }
    }
    let list0 = apply_list_modification(init0, curr_frame_num, max_frame_num, num0, mods0)?;
    let list1 = apply_list_modification(init1, curr_frame_num, max_frame_num, num1, mods1)?;
    Ok((list0, list1))
}
fn same_picture(a: &RefFrame, b: &RefFrame) -> bool {
    a.is_long_term() == b.is_long_term()
        && if a.is_long_term() {
            a.lt_idx() == b.lt_idx()
        } else {
            a.pic_poc() == b.pic_poc()
        }
}
fn apply_list_modification(
    init: Vec<Ref>,
    curr_frame_num: u32,
    max_frame_num: u32,
    num_active: usize,
    mods: &[(u32, u32)],
) -> Result<Vec<Ref>, DecodeError> {
    if mods.is_empty() {
        let mut init = init;
        init.truncate(num_active.max(1));
        return Ok(init);
    }
    let curr = curr_frame_num as i64;
    let max = max_frame_num as i64;
    let mut list = init.clone();
    let mut pic_num_pred = curr;
    let mut refidx = 0usize;
    for &(idc, val) in mods {
        let matches: Box<dyn Fn(&RefFrame) -> bool> = if idc == 2 {
            Box::new(move |r: &RefFrame| r.is_long_term() && r.lt_idx() == val)
        } else {
            let abs_diff = (val as i64) + 1;
            let no_wrap = if idc == 0 {
                let x = pic_num_pred - abs_diff;
                if x < 0 { x + max } else { x }
            } else {
                let x = pic_num_pred + abs_diff;
                if x >= max { x - max } else { x }
            };
            pic_num_pred = no_wrap;
            let target = if no_wrap > curr {
                no_wrap - max
            } else {
                no_wrap
            };
            Box::new(move |r: &RefFrame| {
                let f = r.fn_num() as i64;
                let pn = if f > curr { f - max } else { f };
                !r.is_long_term() && pn == target
            })
        };
        let found = init.iter().find(|r| matches(r)).cloned();
        let Some(found) = found else {
            if crate::h264::decoder::mb16::dump_mb_on() {
                let cand: Vec<String> = init
                    .iter()
                    .map(|r| {
                        let f = r.fn_num() as i64;
                        let pn = if f > curr { f - max } else { f };
                        format!(
                            "(fn={} poc={} lt={} picnum={})",
                            r.fn_num(),
                            r.pic_poc(),
                            r.is_long_term(),
                            pn
                        )
                    })
                    .collect();
                eprintln!(
                    "MODFAIL idc={idc} val={val}  curr_frame_num={curr} max={max}  init={}",
                    cand.join(" ")
                );
            }
            return Err(DecodeError::Truncated);
        };
        if refidx > list.len() {
            break;
        }
        list.insert(refidx, found);
        if let Some(dup) = list
            .iter()
            .enumerate()
            .skip(refidx + 1)
            .find(|(_, r)| matches(r))
            .map(|(i, _)| i)
        {
            list.remove(dup);
        }
        refidx += 1;
        if refidx >= num_active {
            break;
        }
    }
    list.truncate(num_active.max(1));
    Ok(list)
}
