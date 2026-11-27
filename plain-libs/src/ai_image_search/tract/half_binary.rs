use super::Binary;
use crate::binary16::{from_f32, to_f32};
use ndarray::{ArrayD, ArrayViewD, IxDyn, Zip};

pub(super) fn eval(
    op: Binary,
    a: ArrayViewD<'_, u16>,
    b: ArrayViewD<'_, u16>,
) -> Result<ArrayD<u16>, String> {
    #[cfg(target_arch = "aarch64")]
    if let Some(result) = neon::eval(op, a.view(), b.view()) {
        return result;
    }
    Ok(portable(op, a, b))
}

fn portable(op: Binary, a: ArrayViewD<'_, u16>, b: ArrayViewD<'_, u16>) -> ArrayD<u16> {
    Zip::from(a)
        .and(b)
        .map_collect(|&a, &b| from_f32(op.float(to_f32(a), to_f32(b))))
}

#[cfg(target_arch = "aarch64")]
mod neon {
    use super::*;
    use ndarray::{ArrayView1, Axis};
    use std::arch::aarch64::{
        uint16x8_t, vandq_u16, vdupq_n_u16, vld1q_u16, vmaxvq_u16, vst1q_u16,
    };

    pub(super) fn eval(
        op: Binary,
        a: ArrayViewD<'_, u16>,
        b: ArrayViewD<'_, u16>,
    ) -> Option<Result<ArrayD<u16>, String>> {
        let last = a.ndim().checked_sub(1)?;
        let width = a.shape()[last];
        let supported = |stride| stride == 0 || stride == 1;
        if width < 8
            || a.is_empty()
            || !supported(a.strides()[last])
            || !supported(b.strides()[last])
            || !matches!(op, Binary::Add | Binary::Sub | Binary::Mul)
            || !std::arch::is_aarch64_feature_detected!("fp16")
        {
            return None;
        }
        let fpcr: u64;
        // SAFETY: Reading the current thread's FP mode does not modify machine state.
        unsafe {
            std::arch::asm!("mrs {0}, fpcr", out(reg) fpcr, options(nomem, nostack, preserves_flags))
        };
        if fpcr & ((0x1f << 22) | (1 << 19) | 3) != 0 {
            return None;
        }
        let mut data = Vec::new();
        if let Err(error) = data.try_reserve_exact(a.len()) {
            return Some(Err(error.to_string()));
        }
        data.resize(a.len(), 0);
        // SAFETY: FP16 and the compatible FP mode were checked above.
        unsafe {
            match op {
                Binary::Add => run::<0>(a.view(), b, &mut data, last),
                Binary::Sub => run::<1>(a.view(), b, &mut data, last),
                Binary::Mul => run::<2>(a.view(), b, &mut data, last),
                _ => unreachable!(),
            }
        }
        Some(ArrayD::from_shape_vec(IxDyn(a.shape()), data).map_err(|error| error.to_string()))
    }

    #[target_feature(enable = "fp16")]
    unsafe fn run<const OP: u8>(
        a: ArrayViewD<'_, u16>,
        b: ArrayViewD<'_, u16>,
        output: &mut [u16],
        axis: usize,
    ) {
        let mask = vdupq_n_u16(0x7c00);
        let width = a.shape()[axis];
        for ((a, b), out) in a
            .lanes(Axis(axis))
            .into_iter()
            .zip(b.lanes(Axis(axis)))
            .zip(output.chunks_exact_mut(width))
        {
            let scalar = |i: usize| {
                let x = to_f32(a[i]);
                let y = to_f32(b[i]);
                from_f32(match OP {
                    0 => x + y,
                    1 => x - y,
                    _ => x * y,
                })
            };
            let mut blocks = out.chunks_exact_mut(8);
            for (block, out) in blocks.by_ref().enumerate() {
                let offset = block * 8;
                let (x, y) = unsafe { (load(&a, offset), load(&b, offset)) };
                if vmaxvq_u16(vandq_u16(x, mask)) == 0x7c00
                    || vmaxvq_u16(vandq_u16(y, mask)) == 0x7c00
                {
                    for (i, value) in out.iter_mut().enumerate() {
                        *value = scalar(offset + i);
                    }
                    continue;
                }
                let result: uint16x8_t;
                macro_rules! arithmetic {
                    ($instruction:literal) => {
                        std::arch::asm!(
                            "fcvtl v2.4s, {x:v}.4h",
                            "fcvtl2 v3.4s, {x:v}.8h",
                            "fcvtl v4.4s, {y:v}.4h",
                            "fcvtl2 v5.4s, {y:v}.8h",
                            concat!($instruction, " v2.4s, v2.4s, v4.4s"),
                            concat!($instruction, " v3.4s, v3.4s, v5.4s"),
                            "fcvtn {result:v}.4h, v2.4s",
                            "fcvtn2 {result:v}.8h, v3.4s",
                            x = in(vreg) x, y = in(vreg) y, result = lateout(vreg) result,
                            out("v2") _, out("v3") _, out("v4") _, out("v5") _,
                            options(nomem, nostack, preserves_flags)
                        )
                    };
                }
                // SAFETY: The vectors have all eight lanes, and the destination block has room for all eight.
                unsafe {
                    match OP {
                        0 => arithmetic!("fadd"),
                        1 => arithmetic!("fsub"),
                        _ => arithmetic!("fmul"),
                    }
                    vst1q_u16(out.as_mut_ptr(), result);
                }
            }
            let offset = width / 8 * 8;
            for (i, value) in blocks.into_remainder().iter_mut().enumerate() {
                *value = scalar(offset + i);
            }
        }
    }

    #[target_feature(enable = "neon")]
    unsafe fn load(lane: &ArrayView1<'_, u16>, offset: usize) -> uint16x8_t {
        if lane.strides()[0] == 0 {
            vdupq_n_u16(lane[0])
        } else {
            // SAFETY: The dispatcher requires unit stride and the caller only loads full blocks.
            unsafe { vld1q_u16(lane.as_ptr().add(offset)) }
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/ai_image_search/tract/half_binary.rs"]
mod tests;
