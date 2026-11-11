#[inline]
fn c(val: i32) -> i32 {
    val.clamp(-128, 127)
}
#[inline]
fn u2s(val: u8) -> i32 {
    i32::from(val) - 128
}
#[inline]
fn s2u(val: i32) -> u8 {
    (c(val) + 128) as u8
}
#[inline]
const fn diff(val1: u8, val2: u8) -> u8 {
    u8::abs_diff(val1, val2)
}
fn common_adjust_vertical(
    use_outer_taps: bool,
    pixels: &mut [u8],
    point: usize,
    stride: usize,
) -> i32 {
    let p1 = u2s(pixels[point - 2 * stride]);
    let p0 = u2s(pixels[point - stride]);
    let q0 = u2s(pixels[point]);
    let q1 = u2s(pixels[point + stride]);
    let outer = if use_outer_taps { c(p1 - q1) } else { 0 };
    let a = c(outer + 3 * (q0 - p0));
    let b = (c(a + 3)) >> 3;
    let a = (c(a + 4)) >> 3;
    pixels[point] = s2u(q0 - a);
    pixels[point - stride] = s2u(p0 + b);
    a
}
fn common_adjust_horizontal(use_outer_taps: bool, pixels: &mut [u8]) -> i32 {
    let p1 = u2s(pixels[2]);
    let p0 = u2s(pixels[3]);
    let q0 = u2s(pixels[4]);
    let q1 = u2s(pixels[5]);
    let outer = if use_outer_taps { c(p1 - q1) } else { 0 };
    let a = c(outer + 3 * (q0 - p0));
    let b = (c(a + 3)) >> 3;
    let a = (c(a + 4)) >> 3;
    pixels[4] = s2u(q0 - a);
    pixels[3] = s2u(p0 + b);
    a
}
#[inline]
fn simple_threshold_vertical(
    filter_limit: i32,
    pixels: &[u8],
    point: usize,
    stride: usize,
) -> bool {
    i32::from(diff(pixels[point - stride], pixels[point])) * 2
        + i32::from(diff(pixels[point - 2 * stride], pixels[point + stride])) / 2
        <= filter_limit
}
#[inline]
fn simple_threshold_horizontal(filter_limit: i32, pixels: &[u8]) -> bool {
    assert!(pixels.len() >= 6);
    i32::from(diff(pixels[3], pixels[4])) * 2 + i32::from(diff(pixels[2], pixels[5])) / 2
        <= filter_limit
}
fn should_filter_vertical(
    interior_limit: u8,
    edge_limit: u8,
    pixels: &[u8],
    point: usize,
    stride: usize,
) -> bool {
    simple_threshold_vertical(i32::from(edge_limit), pixels, point, stride)
        && diff(pixels[point - 4 * stride], pixels[point - 3 * stride]) <= interior_limit
        && diff(pixels[point - 3 * stride], pixels[point - 2 * stride]) <= interior_limit
        && diff(pixels[point - 2 * stride], pixels[point - stride]) <= interior_limit
        && diff(pixels[point + 3 * stride], pixels[point + 2 * stride]) <= interior_limit
        && diff(pixels[point + 2 * stride], pixels[point + stride]) <= interior_limit
        && diff(pixels[point + stride], pixels[point]) <= interior_limit
}
fn should_filter_horizontal(interior_limit: u8, edge_limit: u8, pixels: &[u8]) -> bool {
    assert!(pixels.len() >= 8);
    simple_threshold_horizontal(i32::from(edge_limit), pixels)
        && diff(pixels[0], pixels[1]) <= interior_limit
        && diff(pixels[1], pixels[2]) <= interior_limit
        && diff(pixels[2], pixels[3]) <= interior_limit
        && diff(pixels[7], pixels[6]) <= interior_limit
        && diff(pixels[6], pixels[5]) <= interior_limit
        && diff(pixels[5], pixels[4]) <= interior_limit
}
#[inline]
fn high_edge_variance_vertical(threshold: u8, pixels: &[u8], point: usize, stride: usize) -> bool {
    diff(pixels[point - 2 * stride], pixels[point - stride]) > threshold
        || diff(pixels[point + stride], pixels[point]) > threshold
}
#[inline]
fn high_edge_variance_horizontal(threshold: u8, pixels: &[u8]) -> bool {
    diff(pixels[2], pixels[3]) > threshold || diff(pixels[5], pixels[4]) > threshold
}
pub(crate) fn simple_segment_vertical(
    edge_limit: u8,
    pixels: &mut [u8],
    point: usize,
    stride: usize,
) {
    if simple_threshold_vertical(i32::from(edge_limit), pixels, point, stride) {
        common_adjust_vertical(true, pixels, point, stride);
    }
}
pub(crate) fn simple_segment_horizontal(edge_limit: u8, pixels: &mut [u8]) {
    if simple_threshold_horizontal(i32::from(edge_limit), pixels) {
        common_adjust_horizontal(true, pixels);
    }
}
pub(crate) fn subblock_filter_vertical(
    hev_threshold: u8,
    interior_limit: u8,
    edge_limit: u8,
    pixels: &mut [u8],
    point: usize,
    stride: usize,
) {
    if should_filter_vertical(interior_limit, edge_limit, pixels, point, stride) {
        let hv = high_edge_variance_vertical(hev_threshold, pixels, point, stride);
        let a = (common_adjust_vertical(hv, pixels, point, stride) + 1) >> 1;
        if !hv {
            pixels[point + stride] = s2u(u2s(pixels[point + stride]) - a);
            pixels[point - 2 * stride] = s2u(u2s(pixels[point - 2 * stride]) + a);
        }
    }
}
pub(crate) fn subblock_filter_horizontal(
    hev_threshold: u8,
    interior_limit: u8,
    edge_limit: u8,
    pixels: &mut [u8],
) {
    if should_filter_horizontal(interior_limit, edge_limit, pixels) {
        let hv = high_edge_variance_horizontal(hev_threshold, pixels);
        let a = (common_adjust_horizontal(hv, pixels) + 1) >> 1;
        if !hv {
            pixels[5] = s2u(u2s(pixels[5]) - a);
            pixels[2] = s2u(u2s(pixels[2]) + a);
        }
    }
}
pub(crate) fn macroblock_filter_vertical(
    hev_threshold: u8,
    interior_limit: u8,
    edge_limit: u8,
    pixels: &mut [u8],
    point: usize,
    stride: usize,
) {
    if should_filter_vertical(interior_limit, edge_limit, pixels, point, stride) {
        if !high_edge_variance_vertical(hev_threshold, pixels, point, stride) {
            let p2 = u2s(pixels[point - 3 * stride]);
            let p1 = u2s(pixels[point - 2 * stride]);
            let p0 = u2s(pixels[point - stride]);
            let q0 = u2s(pixels[point]);
            let q1 = u2s(pixels[point + stride]);
            let q2 = u2s(pixels[point + 2 * stride]);
            let w = c(c(p1 - q1) + 3 * (q0 - p0));
            let a = c((27 * w + 63) >> 7);
            pixels[point] = s2u(q0 - a);
            pixels[point - stride] = s2u(p0 + a);
            let a = c((18 * w + 63) >> 7);
            pixels[point + stride] = s2u(q1 - a);
            pixels[point - 2 * stride] = s2u(p1 + a);
            let a = c((9 * w + 63) >> 7);
            pixels[point + 2 * stride] = s2u(q2 - a);
            pixels[point - 3 * stride] = s2u(p2 + a);
        } else {
            common_adjust_vertical(true, pixels, point, stride);
        }
    }
}
pub(crate) fn macroblock_filter_horizontal(
    hev_threshold: u8,
    interior_limit: u8,
    edge_limit: u8,
    pixels: &mut [u8],
) {
    assert!(pixels.len() >= 8);
    if should_filter_horizontal(interior_limit, edge_limit, pixels) {
        if !high_edge_variance_horizontal(hev_threshold, pixels) {
            let p2 = u2s(pixels[1]);
            let p1 = u2s(pixels[2]);
            let p0 = u2s(pixels[3]);
            let q0 = u2s(pixels[4]);
            let q1 = u2s(pixels[5]);
            let q2 = u2s(pixels[6]);
            let w = c(c(p1 - q1) + 3 * (q0 - p0));
            let a = c((27 * w + 63) >> 7);
            pixels[4] = s2u(q0 - a);
            pixels[3] = s2u(p0 + a);
            let a = c((18 * w + 63) >> 7);
            pixels[5] = s2u(q1 - a);
            pixels[2] = s2u(p1 + a);
            let a = c((9 * w + 63) >> 7);
            pixels[6] = s2u(q2 - a);
            pixels[1] = s2u(p2 + a);
        } else {
            common_adjust_horizontal(true, pixels);
        }
    }
}
