use std::collections::TryReserveError;
use std::iter::repeat;
#[inline(always)]
pub(crate) fn expand_packed<F>(buf: &mut [u8], channels: usize, bit_depth: u8, mut func: F)
where
    F: FnMut(u8, &mut [u8]),
{
    let pixels = buf.len() / channels * bit_depth as usize;
    let extra = pixels % 8;
    let entries = pixels / 8
        + match extra {
            0 => 0,
            _ => 1,
        };
    let mask = ((1u16 << bit_depth) - 1) as u8;
    let i = (0..entries)
        .rev()
        .flat_map(|idx| (0..8 / bit_depth).map(|i| i * bit_depth).zip(repeat(idx)))
        .skip(extra);
    let buf_len = buf.len();
    let j_inv = (channels..buf_len).step_by(channels);
    for ((shift, i), j_inv) in i.zip(j_inv) {
        let j = buf_len - j_inv;
        let pixel = (buf[i] & (mask << shift)) >> shift;
        func(pixel, &mut buf[j..(j + channels)]);
    }
}

#[inline(always)]
fn interleave_planes_inner<const PLANES: usize, const N: usize>(
    out: &mut [[u8; N]],
    planes: [&[[u8; N]]; PLANES],
) {
    let mut iters = planes.map(|plane| plane.iter().copied());
    for out in out.as_chunks_mut::<PLANES>().0 {
        let vals = iters.each_mut().map(Iterator::next);
        for i in 0..PLANES {
            out[i] = vals[i].unwrap_or(out[i]);
        }
    }
}

#[inline]
pub(crate) fn clamp<N>(a: N, min: N, max: N) -> N
where
    N: PartialOrd,
{
    if a < min {
        min
    } else if a > max {
        max
    } else {
        a
    }
}
#[inline]
pub(crate) fn vec_try_with_capacity<T>(capacity: usize) -> Result<Vec<T>, TryReserveError> {
    let mut vec = Vec::new();
    vec.try_reserve_exact(capacity)?;
    Ok(vec)
}
