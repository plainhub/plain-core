//! The bounding rectangle of a block's non-zero coefficients.
//!
//! Residual coding sends a last-significant-coefficient position and nothing
//! beyond it, so for anything but the flattest content most of a 16x16 or
//! 32x32 block is zero. The scaling of clause 8.6.3 maps a zero level to a
//! zero coefficient, and both 1-D stages of clause 8.6.4.2 sum over an index
//! that those zeros index directly, so bounding either by this rectangle
//! removes work rather than approximating it: the result is bit-identical.

/// Widest block any transform accepts.
const MAX_N: usize = 32;

/// The smallest `(rows, cols)` rectangle at the top-left corner of the
/// row-major `n`x`n` block `b` that holds every non-zero entry.
///
/// Only the first `n * n` entries are read; `(0, 0)` means the block is all
/// zero. An `n` outside `1..=32` reports the whole block, which is always
/// safe for a caller to sum over.
///
/// Each row is OR-folded into a per-column mask instead of being searched,
/// which keeps the pass a straight vectorisable sweep with no data-dependent
/// branch inside the inner loop.
pub(super) fn extent(b: &[i32], n: usize) -> (usize, usize) {
    if n == 0 || n > MAX_N {
        return (n, n);
    }
    let mut col_mask = [0i32; MAX_N];
    let cm = &mut col_mask[..n];
    let mut rows = 0usize;
    for (y, row) in b.chunks_exact(n).take(n).enumerate() {
        let mut any = 0i32;
        for (m, &v) in cm.iter_mut().zip(row) {
            *m |= v;
            any |= v;
        }
        if any != 0 {
            rows = y + 1;
        }
    }
    let cols = cm.iter().rposition(|&v| v != 0).map_or(0, |x| x + 1);
    (rows, cols)
}

#[cfg(test)]
#[path = "../../../tests/unit/src/hevc/transform/extent.rs"]
mod tests;
