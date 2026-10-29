pub(super) fn sample_at_or_before(stts: &[(u64, u64)], target: u64) -> u32 {
    let mut id = 1u32;
    let mut elapsed = 0u64;
    for &(count, delta) in stts {
        let delta = delta.max(1);
        let ticks = count.saturating_mul(delta);
        if target < elapsed.saturating_add(ticks) {
            return id
                .saturating_add(u32::try_from((target - elapsed) / delta).unwrap_or(u32::MAX));
        }
        id = id.saturating_add(u32::try_from(count).unwrap_or(u32::MAX));
        elapsed = elapsed.saturating_add(ticks);
    }
    id
}

pub(super) fn locate_keyframe_sample(
    stts: &[(u64, u64)],
    stss: Option<&[u32]>,
    target: u64,
) -> u32 {
    let candidate = sample_at_or_before(stts, target);
    let Some(syncs) = stss else {
        return candidate;
    };
    let position = syncs.partition_point(|sample| *sample <= candidate);
    syncs.get(position.saturating_sub(1)).copied().unwrap_or(1)
}
