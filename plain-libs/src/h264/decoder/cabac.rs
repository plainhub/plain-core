use crate::h264::common::cabac_tables::{CTX_INIT, RANGE_LPS, STATE_TRANS};
use std::borrow::ToOwned;
use std::boxed::Box;
use std::format;
use std::string::{String, ToString};
use std::vec;
use std::vec::Vec;
const fn build_fused() -> [u32; 4 * 256] {
    let mut t = [0u32; 4 * 256];
    let mut q = 0;
    while q < 4 {
        let mut s = 0;
        while s < 128 {
            let lps = RANGE_LPS[s >> 1][q] as u32;
            let tm = {
                let mps = s as u8 & 1;
                ((STATE_TRANS[s >> 1][1] << 1) | mps) as u32
            };
            let tl = {
                let mps = s as u8 & 1;
                let new_mps = if s >> 1 == 0 { 1 - mps } else { mps };
                ((STATE_TRANS[s >> 1][0] << 1) | new_mps) as u32
            };
            t[q * 256 + s] = lps | (tm << 8) | (tl << 16);
            s += 1;
        }
        q += 1;
    }
    t
}
static FUSED: [u32; 4 * 256] = build_fused();
const OFF: u32 = 41;
const REFILL_AT: i32 = 8;
#[inline(always)]
fn tr(kind: &str, eng: &Engine) {
    let _ = (kind, eng);
}
#[cold]
#[inline(never)]
fn refill_tail(data: &[u8], p: usize) -> u32 {
    let b = |i: usize| data.get(p + i).copied().unwrap_or(0) as u32;
    (b(0) << 24) | (b(1) << 16) | (b(2) << 8) | b(3)
}
#[derive(Clone, Copy, Debug)]
pub struct Engine {
    byte_pos: usize,
    low: u64,
    cnt: i32,
    range: u32,
}
pub type Ctx = [u8; 512];
impl Engine {
    #[inline(always)]
    fn refill(&mut self, data: &[u8]) {
        let v = match data.get(self.byte_pos..self.byte_pos + 4) {
            Some(c) => u32::from_be_bytes([c[0], c[1], c[2], c[3]]),
            None => refill_tail(data, self.byte_pos),
        };
        self.low |= (v as u64) << ((OFF as i32 - 32 - self.cnt) as u32);
        self.byte_pos += 4;
        self.cnt += 32;
    }
    #[inline(always)]
    fn renorm(&mut self, data: &[u8]) {
        let n = self.range.leading_zeros() - 23;
        self.range <<= n;
        self.low <<= n;
        self.cnt -= n as i32;
        if self.cnt < REFILL_AT {
            self.refill(data);
        }
    }
    #[inline(always)]
    pub fn decode_decision(&mut self, data: &[u8], ctx: &mut Ctx, ctx_idx: usize) -> u32 {
        tr("D", self);
        let ctx_idx = ctx_idx & 511;
        let s = ctx[ctx_idx] as usize;
        let q = ((self.range >> 6) & 3) as usize;
        let e = FUSED[(q << 8) | s];
        let lps = e & 0xFF;
        debug_assert!(
            self.range >= 256,
            "renorm invariant broken: range={}",
            self.range
        );
        self.range -= lps;
        let scaled = (self.range as u64) << OFF;
        let mask64 = ((scaled as i64 - self.low as i64 - 1) >> 63) as u64;
        let mask = mask64 as u32;
        self.low -= scaled & mask64;
        self.range = self.range.wrapping_add(lps.wrapping_sub(self.range) & mask);
        ctx[ctx_idx] = ((e >> (8 + (mask & 8))) & 0xFF) as u8;
        let bin = (s as u32 ^ mask) & 1;
        self.renorm(data);
        bin
    }
    #[inline(always)]
    pub fn decode_bypass(&mut self, data: &[u8]) -> u32 {
        tr("B", self);
        self.low <<= 1;
        self.cnt -= 1;
        if self.cnt < REFILL_AT {
            self.refill(data);
        }
        let scaled = (self.range as u64) << OFF;
        let mask = ((scaled as i64 - self.low as i64 - 1) >> 63) as u64;
        self.low -= scaled & mask;
        (mask & 1) as u32
    }
    #[inline(always)]
    pub fn decode_bypass_sign(&mut self) -> u32 {
        tr("B", self);
        debug_assert!(self.cnt >= 1, "sign bypass needs a buffered bit");
        self.low <<= 1;
        self.cnt -= 1;
        let scaled = (self.range as u64) << OFF;
        let mask = ((scaled as i64 - self.low as i64 - 1) >> 63) as u64;
        self.low -= scaled & mask;
        (mask & 1) as u32
    }
    #[inline(always)]
    pub fn decode_terminate(&mut self, data: &[u8]) -> bool {
        tr("T", self);
        self.range -= 2;
        if self.low >= (self.range as u64) << OFF {
            true
        } else {
            self.renorm(data);
            false
        }
    }
}
pub struct Cabac<'a> {
    data: &'a [u8],
    eng: Engine,
    ctx: Ctx,
}
impl<'a> Cabac<'a> {
    pub fn new(data: &'a [u8], start_byte: usize, qp: i32, init_idc: u32, is_i: bool) -> Self {
        let model = if is_i {
            0
        } else {
            ((init_idc + 1) as usize).min(3)
        };
        let q = qp.clamp(0, 51);
        let mut ctx = [0u8; 512];
        for (i, c) in ctx.iter_mut().take(460).enumerate() {
            let (m, n) = CTX_INIT[i][model];
            let pre = (((m as i32 * q) >> 4) + n as i32).clamp(1, 126);
            *c = if pre <= 63 {
                ((63 - pre) as u8) << 1
            } else {
                (((pre - 64) as u8) << 1) | 1
            };
        }
        let mut e = Cabac {
            data,
            eng: Engine {
                byte_pos: start_byte,
                low: 0,
                cnt: 0,
                range: 510,
            },
            ctx,
        };
        e.eng.refill(data);
        e.eng.low <<= 9;
        e.eng.cnt -= 9;
        e
    }
    pub fn dbg_state(&self) -> (u32, u32) {
        (self.eng.range, (self.eng.low >> OFF) as u32)
    }
    pub fn pcm_start_byte(&self) -> usize {
        let consumed = self.eng.byte_pos as isize * 8 - self.eng.cnt as isize;
        ((consumed + 7) >> 3) as usize
    }
    pub fn reinit_at(&mut self, byte: usize) {
        self.eng = Engine {
            byte_pos: byte,
            low: 0,
            cnt: 0,
            range: 510,
        };
        self.eng.refill(self.data);
        self.eng.low <<= 9;
        self.eng.cnt -= 9;
    }
    #[inline(always)]
    pub fn view(&mut self) -> (&'a [u8], Engine, &mut Ctx) {
        (self.data, self.eng, &mut self.ctx)
    }
    #[inline(always)]
    pub fn commit(&mut self, eng: Engine) {
        self.eng = eng;
    }
    #[inline(always)]
    pub fn decode_decision(&mut self, ctx_idx: usize) -> u32 {
        let mut e = self.eng;
        let b = e.decode_decision(self.data, &mut self.ctx, ctx_idx);
        self.eng = e;
        b
    }
    #[inline(always)]
    pub fn decode_bypass(&mut self) -> u32 {
        let mut e = self.eng;
        let b = e.decode_bypass(self.data);
        self.eng = e;
        b
    }
    #[inline(always)]
    pub fn decode_terminate(&mut self) -> bool {
        let mut e = self.eng;
        let b = e.decode_terminate(self.data);
        self.eng = e;
        b
    }
}
