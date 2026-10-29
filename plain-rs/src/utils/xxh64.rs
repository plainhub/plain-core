const PRIME1: u64 = 0x9e3779b185ebca87;
const PRIME2: u64 = 0xc2b2ae3d27d4eb4f;
const PRIME3: u64 = 0x165667b19e3779f9;
const PRIME4: u64 = 0x85ebca77c2b2ae63;
const PRIME5: u64 = 0x27d4eb2f165667c5;

fn round(accumulator: u64, lane: u64) -> u64 {
    accumulator
        .wrapping_add(lane.wrapping_mul(PRIME2))
        .rotate_left(31)
        .wrapping_mul(PRIME1)
}

// XXH64, seed zero: https://github.com/Cyan4973/xxHash/blob/v0.8.3/doc/xxhash_spec.md#xxh64-algorithm-description
pub fn hash(bytes: &[u8]) -> u64 {
    let mut stripes = bytes.chunks_exact(32);
    let mut hash = if bytes.len() >= 32 {
        let mut accumulators = [
            PRIME1.wrapping_add(PRIME2),
            PRIME2,
            0,
            0u64.wrapping_sub(PRIME1),
        ];
        for stripe in stripes.by_ref() {
            for (accumulator, lane) in accumulators.iter_mut().zip(stripe.chunks_exact(8)) {
                *accumulator = round(*accumulator, u64::from_le_bytes(lane.try_into().unwrap()));
            }
        }
        let mut result = accumulators[0]
            .rotate_left(1)
            .wrapping_add(accumulators[1].rotate_left(7))
            .wrapping_add(accumulators[2].rotate_left(12))
            .wrapping_add(accumulators[3].rotate_left(18));
        for accumulator in accumulators {
            result = (result ^ round(0, accumulator))
                .wrapping_mul(PRIME1)
                .wrapping_add(PRIME4);
        }
        result
    } else {
        PRIME5
    };
    hash = hash.wrapping_add(bytes.len() as u64);
    let mut lanes = stripes.remainder().chunks_exact(8);
    for lane in lanes.by_ref() {
        hash = (hash ^ round(0, u64::from_le_bytes(lane.try_into().unwrap())))
            .rotate_left(27)
            .wrapping_mul(PRIME1)
            .wrapping_add(PRIME4);
    }
    let mut tail = lanes.remainder();
    if tail.len() >= 4 {
        hash ^= u64::from(u32::from_le_bytes(tail[..4].try_into().unwrap())).wrapping_mul(PRIME1);
        hash = hash
            .rotate_left(23)
            .wrapping_mul(PRIME2)
            .wrapping_add(PRIME3);
        tail = &tail[4..];
    }
    for byte in tail {
        hash ^= u64::from(*byte).wrapping_mul(PRIME5);
        hash = hash.rotate_left(11).wrapping_mul(PRIME1);
    }
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(PRIME2);
    hash ^= hash >> 29;
    hash = hash.wrapping_mul(PRIME3);
    hash ^ (hash >> 32)
}

#[cfg(test)]
#[path = "../../tests/unit/utils/xxh64.rs"]
mod tests;
