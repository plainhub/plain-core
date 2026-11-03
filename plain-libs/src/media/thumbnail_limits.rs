pub fn workers(available_cpus: usize) -> usize {
    available_cpus
        .saturating_sub(available_cpus.div_ceil(4))
        .max(1)
}

pub fn memory_budget_mb(total_bytes: u64) -> u32 {
    (total_bytes / 8).div_euclid(1024 * 1024).clamp(1, 16_384) as u32
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/media/thumbnail_limits.rs"]
mod tests;
