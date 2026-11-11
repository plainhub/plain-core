pub mod fast;
mod seed;

const ARBITRARY1: u64 = 0x13198a2e03707344;
const ARBITRARY2: u64 = 0xa4093822299f31d0;
const ARBITRARY5: u64 = 0xbe5466cf34e90c6c;
#[inline(always)]
const fn folded_multiply(x: u64, y: u64) -> u64 {
    #[cfg(any(
        all(
            target_pointer_width = "64",
            not(any(target_arch = "sparc64", target_arch = "wasm64")),
        ),
        target_arch = "aarch64",
        target_arch = "x86_64",
        all(target_family = "wasm", target_feature = "wide-arithmetic"),
    ))]
    {
        let full = (x as u128).wrapping_mul(y as u128);
        let lo = full as u64;
        let hi = (full >> 64) as u64;
        lo ^ hi
    }
    #[cfg(not(any(
        all(
            target_pointer_width = "64",
            not(any(target_arch = "sparc64", target_arch = "wasm64")),
        ),
        target_arch = "aarch64",
        target_arch = "x86_64",
        all(target_family = "wasm", target_feature = "wide-arithmetic"),
    )))]
    {
        let lx = x as u32;
        let ly = y as u32;
        let hx = (x >> 32) as u32;
        let hy = (y >> 32) as u32;
        let ll = (lx as u64).wrapping_mul(ly as u64);
        let lh = (lx as u64).wrapping_mul(hy as u64);
        let hl = (hx as u64).wrapping_mul(ly as u64);
        let hh = (hx as u64).wrapping_mul(hy as u64);
        (hh ^ ll) ^ (hl ^ lh).rotate_right(32)
    }
}
#[inline(always)]
const fn rotate_right(x: u64, r: u32) -> u64 {
    #[cfg(any(
        target_pointer_width = "64",
        target_arch = "aarch64",
        target_arch = "x86_64",
        target_family = "wasm",
    ))]
    {
        x.rotate_right(r)
    }
    #[cfg(not(any(
        target_pointer_width = "64",
        target_arch = "aarch64",
        target_arch = "x86_64",
        target_family = "wasm",
    )))]
    {
        let lo = (x as u32).rotate_right(r);
        let hi = ((x >> 32) as u32).rotate_right(r);
        ((hi as u64) << 32) | lo as u64
    }
}
#[cold]
fn cold_path() {}
#[inline(always)]
fn hash_bytes_short(bytes: &[u8], accumulator: u64, seeds: &[u64; 6]) -> u64 {
    let len = bytes.len();
    let mut s0 = accumulator;
    let mut s1 = seeds[1];
    if len >= 8 {
        s0 ^= u64::from_ne_bytes(bytes[0..8].try_into().unwrap());
        s1 ^= u64::from_ne_bytes(bytes[len - 8..].try_into().unwrap());
    } else if len >= 4 {
        s0 ^= u32::from_ne_bytes(bytes[0..4].try_into().unwrap()) as u64;
        s1 ^= u32::from_ne_bytes(bytes[len - 4..].try_into().unwrap()) as u64;
    } else if len > 0 {
        let lo = bytes[0];
        let mid = bytes[len / 2];
        let hi = bytes[len - 1];
        s0 ^= lo as u64;
        s1 ^= ((hi as u64) << 8) | mid as u64;
    }
    folded_multiply(s0, s1)
}
#[inline(always)]
unsafe fn load(bytes: &[u8], offset: usize) -> u64 {
    unsafe { unsafe { bytes.as_ptr().add(offset).cast::<u64>().read_unaligned() } }
}
#[cold]
#[inline(never)]
unsafe fn hash_bytes_long(mut v: &[u8], accumulator: u64, seeds: &[u64; 6]) -> u64 {
    unsafe {
        let mut s0 = accumulator;
        let mut s1 = s0.wrapping_add(seeds[1]);
        if v.len() > 128 {
            cold_path();
            let mut s2 = s0.wrapping_add(seeds[2]);
            let mut s3 = s0.wrapping_add(seeds[3]);
            if v.len() > 256 {
                cold_path();
                let mut s4 = s0.wrapping_add(seeds[4]);
                let mut s5 = s0.wrapping_add(seeds[5]);
                loop {
                    unsafe {
                        s0 = folded_multiply(load(v, 0) ^ s0, load(v, 48) ^ seeds[0]);
                        s1 = folded_multiply(load(v, 8) ^ s1, load(v, 56) ^ seeds[0]);
                        s2 = folded_multiply(load(v, 16) ^ s2, load(v, 64) ^ seeds[0]);
                        s3 = folded_multiply(load(v, 24) ^ s3, load(v, 72) ^ seeds[0]);
                        s4 = folded_multiply(load(v, 32) ^ s4, load(v, 80) ^ seeds[0]);
                        s5 = folded_multiply(load(v, 40) ^ s5, load(v, 88) ^ seeds[0]);
                    }
                    v = &v[96..];
                    if v.len() <= 256 {
                        break;
                    }
                }
                s0 ^= s4;
                s1 ^= s5;
            }
            loop {
                unsafe {
                    s0 = folded_multiply(load(v, 0) ^ s0, load(v, 32) ^ seeds[0]);
                    s1 = folded_multiply(load(v, 8) ^ s1, load(v, 40) ^ seeds[0]);
                    s2 = folded_multiply(load(v, 16) ^ s2, load(v, 48) ^ seeds[0]);
                    s3 = folded_multiply(load(v, 24) ^ s3, load(v, 56) ^ seeds[0]);
                }
                v = &v[64..];
                if v.len() <= 128 {
                    break;
                }
            }
            s0 ^= s2;
            s1 ^= s3;
        }
        let len = v.len();
        unsafe {
            s0 = folded_multiply(load(v, 0) ^ s0, load(v, len - 16) ^ seeds[0]);
            s1 = folded_multiply(load(v, 8) ^ s1, load(v, len - 8) ^ seeds[0]);
            if len >= 32 {
                s0 = folded_multiply(load(v, 16) ^ s0, load(v, len - 32) ^ seeds[0]);
                s1 = folded_multiply(load(v, 24) ^ s1, load(v, len - 24) ^ seeds[0]);
                if len >= 64 {
                    s0 = folded_multiply(load(v, 32) ^ s0, load(v, len - 48) ^ seeds[0]);
                    s1 = folded_multiply(load(v, 40) ^ s1, load(v, len - 40) ^ seeds[0]);
                    if len >= 96 {
                        s0 = folded_multiply(load(v, 48) ^ s0, load(v, len - 64) ^ seeds[0]);
                        s1 = folded_multiply(load(v, 56) ^ s1, load(v, len - 56) ^ seeds[0]);
                    }
                }
            }
        }
        s0 ^ s1
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/foldhash.rs"]
mod tests;
