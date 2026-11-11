#[cfg(not(target_has_atomic = "8"))]
const FIXED_GLOBAL_SEED: SharedSeed = SharedSeed {
    seeds: [
        0xc0ac29b7c97c50dd,
        0x3f84d5b5b5470917,
        0x9216d5d98979fb1b,
        0xd1310ba698dfb5ac,
        0x2ffd72dbd01adfb7,
        0xb8e1afed6a267e96,
    ],
};
use super::{ARBITRARY2, folded_multiply};
use crate::foldhash::{ARBITRARY1, ARBITRARY5};
pub(crate) fn gen_per_hasher_seed() -> u64 {
    let mut per_hasher_seed = 0;
    let stack_ptr = core::ptr::addr_of!(per_hasher_seed) as u64;
    per_hasher_seed = stack_ptr;
    {
        use core::sync::atomic::{AtomicUsize, Ordering};
        static PER_HASHER_NONDETERMINISM: AtomicUsize = AtomicUsize::new(0);
        let nondeterminism = PER_HASHER_NONDETERMINISM.load(Ordering::Relaxed) as u64;
        per_hasher_seed = folded_multiply(per_hasher_seed, ARBITRARY1 ^ nondeterminism);
        PER_HASHER_NONDETERMINISM.store(per_hasher_seed as usize, Ordering::Relaxed);
    }
    folded_multiply(per_hasher_seed, ARBITRARY2)
}
#[derive(Clone, Debug)]
pub struct SharedSeed {
    pub(crate) seeds: [u64; 6],
}
impl SharedSeed {
    pub const fn from_u64(seed: u64) -> Self {
        macro_rules! mix {
            ($x : expr) => {
                folded_multiply($x, ARBITRARY5)
            };
        }
        let seed_a = mix!(mix!(mix!(seed)));
        let seed_b = mix!(mix!(mix!(seed_a)));
        let seed_c = mix!(mix!(mix!(seed_b)));
        let seed_d = mix!(mix!(mix!(seed_c)));
        let seed_e = mix!(mix!(mix!(seed_d)));
        let seed_f = mix!(mix!(mix!(seed_e)));
        const FORCED_ONES: u64 = (1 << 63) | (1 << 31) | 1;
        Self {
            seeds: [
                seed_a | FORCED_ONES,
                seed_b | FORCED_ONES,
                seed_c | FORCED_ONES,
                seed_d | FORCED_ONES,
                seed_e | FORCED_ONES,
                seed_f | FORCED_ONES,
            ],
        }
    }
}
#[cfg(target_has_atomic = "8")]
mod global {
    use super::*;
    use core::cell::UnsafeCell;
    use core::sync::atomic::{AtomicU8, Ordering};
    fn generate_global_seed() -> SharedSeed {
        let mix = |seed: u64, x: u64| folded_multiply(seed ^ x, ARBITRARY5);
        let mut seed = 0;
        let stack_ptr = &seed as *const _;
        let func_ptr = generate_global_seed;
        let static_ptr = &GLOBAL_SEED_STORAGE as *const _;
        seed = mix(seed, stack_ptr as usize as u64);
        seed = mix(seed, func_ptr as usize as u64);
        seed = mix(seed, static_ptr as usize as u64);
        SharedSeed::from_u64(seed)
    }
    struct GlobalSeedStorage {
        state: AtomicU8,
        seed: UnsafeCell<SharedSeed>,
    }
    const UNINIT: u8 = 0;
    const LOCKED: u8 = 1;
    const INIT: u8 = 2;
    unsafe impl Sync for GlobalSeedStorage {}
    static GLOBAL_SEED_STORAGE: GlobalSeedStorage = GlobalSeedStorage {
        state: AtomicU8::new(UNINIT),
        seed: UnsafeCell::new(SharedSeed { seeds: [0; 6] }),
    };
    #[derive(Copy, Clone, Debug)]
    pub struct GlobalSeed {
        _no_accidental_unsafe_init: (),
    }
    impl GlobalSeed {
        #[inline(always)]
        pub fn new() -> Self {
            if GLOBAL_SEED_STORAGE.state.load(Ordering::Acquire) != INIT {
                Self::init_slow()
            }
            Self {
                _no_accidental_unsafe_init: (),
            }
        }
        #[cold]
        #[inline(never)]
        fn init_slow() {
            let seed = generate_global_seed();
            loop {
                match GLOBAL_SEED_STORAGE.state.compare_exchange_weak(
                    UNINIT,
                    LOCKED,
                    Ordering::Acquire,
                    Ordering::Acquire,
                ) {
                    Ok(_) => unsafe {
                        *GLOBAL_SEED_STORAGE.seed.get() = seed;
                        GLOBAL_SEED_STORAGE.state.store(INIT, Ordering::Release);
                        return;
                    },
                    Err(INIT) => return,
                    _ => core::hint::spin_loop(),
                }
            }
        }
        #[inline(always)]
        pub fn get(self) -> &'static SharedSeed {
            unsafe { &*GLOBAL_SEED_STORAGE.seed.get() }
        }
    }
}
#[cfg(not(target_has_atomic = "8"))]
mod global {
    use super::*;
    #[derive(Copy, Clone, Debug)]
    pub struct GlobalSeed {}
    impl GlobalSeed {
        #[inline(always)]
        pub fn new() -> Self {
            Self {}
        }
        #[inline(always)]
        pub fn get(self) -> &'static SharedSeed {
            &super::FIXED_GLOBAL_SEED
        }
    }
}
pub(crate) use global::GlobalSeed;
