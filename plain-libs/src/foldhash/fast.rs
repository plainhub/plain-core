use crate::foldhash::seed::{GlobalSeed, SharedSeed, gen_per_hasher_seed};
use crate::foldhash::{folded_multiply, hash_bytes_long, hash_bytes_short, rotate_right};
use core::hash::{BuildHasher, Hasher};
#[derive(Clone)]
pub struct FoldHasher<'a> {
    accumulator: u64,
    sponge: u128,
    sponge_len: u8,
    seeds: &'a [u64; 6],
}
impl<'a> FoldHasher<'a> {
    #[inline]
    pub const fn with_seed(per_hasher_seed: u64, shared_seed: &'a SharedSeed) -> FoldHasher<'a> {
        FoldHasher {
            accumulator: per_hasher_seed,
            sponge: 0,
            sponge_len: 0,
            seeds: &shared_seed.seeds,
        }
    }
    #[inline(always)]
    fn write_num<T: Into<u128>>(&mut self, x: T) {
        let bits: usize = 8 * core::mem::size_of::<T>();
        if self.sponge_len as usize + bits > 128 {
            let lo = self.sponge as u64;
            let hi = (self.sponge >> 64) as u64;
            self.accumulator = folded_multiply(lo ^ self.accumulator, hi ^ self.seeds[0]);
            self.sponge = x.into();
            self.sponge_len = bits as u8;
        } else {
            self.sponge |= x.into() << self.sponge_len;
            self.sponge_len += bits as u8;
        }
    }
}
impl<'a> Hasher for FoldHasher<'a> {
    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) {
        let len = bytes.len();
        self.accumulator = rotate_right(self.accumulator, len as u32);
        if len <= 16 {
            self.accumulator = hash_bytes_short(bytes, self.accumulator, self.seeds);
        } else {
            unsafe {
                self.accumulator = hash_bytes_long(bytes, self.accumulator, self.seeds);
            }
        }
    }
    #[inline(always)]
    fn write_u8(&mut self, i: u8) {
        self.write_num(i);
    }
    #[inline(always)]
    fn write_u16(&mut self, i: u16) {
        self.write_num(i);
    }
    #[inline(always)]
    fn write_u32(&mut self, i: u32) {
        self.write_num(i);
    }
    #[inline(always)]
    fn write_u64(&mut self, i: u64) {
        self.write_num(i);
    }
    #[inline(always)]
    fn write_u128(&mut self, i: u128) {
        let lo = i as u64;
        let hi = (i >> 64) as u64;
        self.accumulator = folded_multiply(lo ^ self.accumulator, hi ^ self.seeds[0]);
    }
    #[inline(always)]
    fn write_usize(&mut self, i: usize) {
        #[cfg(target_pointer_width = "32")]
        self.write_num(i as u32);
        #[cfg(target_pointer_width = "64")]
        self.write_num(i as u64);
    }
    #[inline(always)]
    fn finish(&self) -> u64 {
        if self.sponge_len > 0 {
            let lo = self.sponge as u64;
            let hi = (self.sponge >> 64) as u64;
            folded_multiply(lo ^ self.accumulator, hi ^ self.seeds[0])
        } else {
            self.accumulator
        }
    }
}
#[derive(Clone, Debug)]
pub struct RandomState {
    per_hasher_seed: u64,
    global_seed: GlobalSeed,
}
impl Default for RandomState {
    #[inline(always)]
    fn default() -> Self {
        Self {
            per_hasher_seed: gen_per_hasher_seed(),
            global_seed: GlobalSeed::new(),
        }
    }
}
impl BuildHasher for RandomState {
    type Hasher = FoldHasher<'static>;
    #[inline(always)]
    fn build_hasher(&self) -> FoldHasher<'static> {
        FoldHasher::with_seed(self.per_hasher_seed, self.global_seed.get())
    }
}
