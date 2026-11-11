use num_traits::MulAdd;
use std::cmp::max;
use std::ops::{Add, Mul};
#[cfg(any(
    all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "fma"
    ),
    all(target_arch = "aarch64", target_feature = "neon")
))]
#[inline(always)]
pub(crate) fn multiply_accumulate<
    T: Copy + Mul<T, Output = T> + Add<T, Output = T> + MulAdd<T, Output = T>,
>(
    acc: T,
    a: T,
    b: T,
) -> T {
    MulAdd::mul_add(a, b, acc)
}
#[inline(always)]
#[cfg(not(any(
    all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "fma"
    ),
    all(target_arch = "aarch64", target_feature = "neon")
)))]
pub(crate) fn multiply_accumulate<
    T: Copy + Mul<T, Output = T> + Add<T, Output = T> + MulAdd<T, Output = T>,
>(
    acc: T,
    a: T,
    b: T,
) -> T {
    acc + a * b
}
pub(crate) fn resize_dimensions(
    width: u32,
    height: u32,
    nwidth: u32,
    nheight: u32,
    fill: bool,
) -> (u32, u32) {
    let wratio = f64::from(nwidth) / f64::from(width);
    let hratio = f64::from(nheight) / f64::from(height);
    let ratio = if fill {
        f64::max(wratio, hratio)
    } else {
        f64::min(wratio, hratio)
    };
    let nw = max((f64::from(width) * ratio).round() as u64, 1);
    let nh = max((f64::from(height) * ratio).round() as u64, 1);
    if nw > u64::from(u32::MAX) {
        let ratio = f64::from(u32::MAX) / f64::from(width);
        (u32::MAX, max((f64::from(height) * ratio).round() as u32, 1))
    } else if nh > u64::from(u32::MAX) {
        let ratio = f64::from(u32::MAX) / f64::from(height);
        (max((f64::from(width) * ratio).round() as u32, 1), u32::MAX)
    } else {
        (nw as u32, nh as u32)
    }
}
