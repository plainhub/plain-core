pub fn workers(available_cpus: usize) -> usize {
    available_cpus
        .saturating_sub(available_cpus.div_ceil(4))
        .max(1)
}

pub fn memory_budget_mb(total_bytes: u64) -> u32 {
    (total_bytes / 8).div_euclid(1024 * 1024).clamp(1, 16_384) as u32
}

/// Compute target dimensions preserving aspect ratio (no upscaling), based
/// on the *upright* source dimensions.
pub fn compute_target_size(src_w: u32, src_h: u32, w: u32, h: u32) -> (u32, u32) {
    let mut tw = src_w;
    let mut th = src_h;
    if w > 0 && h > 0 {
        let rw = f64::from(w) / f64::from(src_w);
        let rh = f64::from(h) / f64::from(src_h);
        let r = rw.min(rh);
        if r < 1.0 {
            tw = (f64::from(src_w) * r) as u32;
            th = (f64::from(src_h) * r) as u32;
        }
    } else if w > 0 && w < src_w {
        let r = f64::from(w) / f64::from(src_w);
        tw = w;
        th = (f64::from(src_h) * r) as u32;
    } else if h > 0 && h < src_h {
        let r = f64::from(h) / f64::from(src_h);
        th = h;
        tw = (f64::from(src_w) * r) as u32;
    }
    (tw.max(1), th.max(1))
}

pub fn short_edge_size(width: u32, height: u32, short_limit: u32, long_limit: u32) -> (u32, u32) {
    let short = u64::from(width.min(height));
    let long = u64::from(width.max(height));
    let short_limit = short.min(u64::from(short_limit));
    let long_limit = long.min(u64::from(long_limit));
    let (short, long) = if short_limit * long <= long_limit * short {
        (short_limit, short_limit * long / short.max(1))
    } else {
        (long_limit * short / long.max(1), long_limit)
    };
    let (short, long) = (short.max(1) as u32, long.max(1) as u32);
    if width <= height {
        (short, long)
    } else {
        (long, short)
    }
}

pub fn target_size(
    width: u32,
    height: u32,
    bounds: (u32, u32),
    short_edge: Option<u32>,
) -> (u32, u32) {
    match short_edge {
        Some(short) => short_edge_size(width, height, short, bounds.0),
        None => compute_target_size(width, height, bounds.0, bounds.1),
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/media/thumbnail_limits.rs"]
mod tests;
