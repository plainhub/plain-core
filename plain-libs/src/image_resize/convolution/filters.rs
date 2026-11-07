use std::f64::consts::PI;

type FilterFn = fn(f64) -> f64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    Bilinear,
    Lanczos3,
}

pub(crate) fn get_filter_func(filter_type: FilterType) -> (FilterFn, f64) {
    match filter_type {
        FilterType::Bilinear => (bilinear_filter, 1.0),
        FilterType::Lanczos3 => (lanczos_filter, 3.0),
    }
}

#[inline]
fn bilinear_filter(mut x: f64) -> f64 {
    x = x.abs();
    if x < 1.0 { 1.0 - x } else { 0.0 }
}

#[inline]
fn sinc_filter(mut x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else {
        x *= PI;
        x.sin() / x
    }
}

#[inline]
fn lanczos_filter(x: f64) -> f64 {
    // truncated sinc
    if (-3.0..3.0).contains(&x) {
        sinc_filter(x) * sinc_filter(x / 3.)
    } else {
        0.0
    }
}
