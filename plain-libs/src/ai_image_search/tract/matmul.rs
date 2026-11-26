use super::{
    math::multi_broadcast,
    tensor::{Array, Tensor},
};
use crate::binary16::{from_f32, to_f32};
use ndarray::{ArrayD, ArrayView2, ArrayViewD, Axis, Dimension, Ix2, IxDyn, LinalgScalar};

fn matrix<'a, T>(
    mut array: ArrayViewD<'a, T>,
    prefix: &[usize],
) -> Result<ArrayView2<'a, T>, String> {
    for &index in prefix {
        let i = if array.shape()[0] == 1 { 0 } else { index };
        array = array.index_axis_move(Axis(0), i);
    }
    array
        .into_dimensionality::<Ix2>()
        .map_err(|e| e.to_string())
}

fn product<T: LinalgScalar>(
    mut a: ArrayViewD<'_, T>,
    mut b: ArrayViewD<'_, T>,
    a_trans: bool,
    b_trans: bool,
    alpha: T,
) -> Result<Array<T>, String> {
    if a.ndim() == 0 || b.ndim() == 0 {
        return Err("MatMul requires inputs of rank one or higher".into());
    }
    let implicit_m = a.ndim() == 1;
    let implicit_n = b.ndim() == 1;
    if implicit_m {
        a = a.insert_axis(Axis(usize::from(a_trans)));
    }
    if implicit_n {
        b = b.insert_axis(Axis(usize::from(!b_trans)));
    }
    while a.ndim() < b.ndim() {
        a = a.insert_axis(Axis(0));
    }
    while b.ndim() < a.ndim() {
        b = b.insert_axis(Axis(0));
    }
    let row = a.ndim() - 2;
    let col = row + 1;
    if a_trans {
        a.swap_axes(row, col);
    }
    if b_trans {
        b.swap_axes(row, col);
    }
    if a.shape()[col] != b.shape()[row] {
        return Err(format!(
            "Inconsistent MatMul {:?} x {:?}",
            a.shape(),
            b.shape()
        ));
    }
    let batch = multi_broadcast(&[&a.shape()[..row], &b.shape()[..row]])?;
    let (m, n) = (a.shape()[row], b.shape()[col]);
    let mut shape = batch.clone();
    shape.extend([m, n]);
    let count = shape
        .iter()
        .try_fold(1usize, |v, &n| v.checked_mul(n))
        .ok_or("MatMul shape overflow")?;
    let mut data = Vec::new();
    data.try_reserve_exact(count).map_err(|e| e.to_string())?;
    data.resize(count, T::zero());
    let mut output = ArrayD::from_shape_vec(IxDyn(&shape), data).map_err(|e| e.to_string())?;
    for prefix in ndarray::indices(IxDyn(&batch)) {
        let ma = matrix(a.view(), prefix.slice())?;
        let mb = matrix(b.view(), prefix.slice())?;
        let mut mc = output.view_mut();
        for &index in prefix.slice() {
            mc = mc.index_axis_move(Axis(0), index);
        }
        let mut mc = mc.into_dimensionality::<Ix2>().map_err(|e| e.to_string())?;
        ndarray::linalg::general_mat_mul(alpha, &ma, &mb, T::zero(), &mut mc);
    }
    let mut shape = batch;
    if !implicit_m {
        shape.push(m);
    }
    if !implicit_n {
        shape.push(n);
    }
    output
        .into_shape_with_order(IxDyn(&shape))
        .map(|a| a.into_shared())
        .map_err(|e| e.to_string())
}

pub(super) fn matmul(
    a: &Tensor,
    b: &Tensor,
    a_trans: bool,
    b_trans: bool,
    alpha: f32,
) -> Result<Tensor, String> {
    Ok(match (a, b) {
        (Tensor::F16(a), Tensor::F16(b)) => Tensor::F16(
            product(
                a.mapv(to_f32).view(),
                b.mapv(to_f32).view(),
                a_trans,
                b_trans,
                alpha,
            )?
            .mapv(from_f32)
            .into_shared(),
        ),
        (Tensor::F32(a), Tensor::F32(b)) => {
            Tensor::F32(product(a.view(), b.view(), a_trans, b_trans, alpha)?)
        }
        (Tensor::F64(a), Tensor::F64(b)) => Tensor::F64(product(
            a.view(),
            b.view(),
            a_trans,
            b_trans,
            f64::from(alpha),
        )?),
        (Tensor::I32(a), Tensor::I32(b)) => {
            Tensor::I32(product(a.view(), b.view(), a_trans, b_trans, alpha as i32)?)
        }
        (Tensor::I64(a), Tensor::I64(b)) => {
            Tensor::I64(product(a.view(), b.view(), a_trans, b_trans, alpha as i64)?)
        }
        (Tensor::U32(a), Tensor::U32(b)) => {
            Tensor::U32(product(a.view(), b.view(), a_trans, b_trans, alpha as u32)?)
        }
        (Tensor::U64(a), Tensor::U64(b)) => {
            Tensor::U64(product(a.view(), b.view(), a_trans, b_trans, alpha as u64)?)
        }
        _ => return Err("Incompatible MatMul types".into()),
    })
}
