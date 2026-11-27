use super::*;

fn values(shape: &[usize], seed: usize) -> Array<u16> {
    let len = shape.iter().product();
    let data = (0..len)
        .map(|i| from_f32((((i * 71 + seed) % 193) as f32 - 96.0) / 73.0))
        .collect();
    ArrayD::from_shape_vec(IxDyn(shape), data)
        .unwrap()
        .into_shared()
}

fn compare(a: &[usize], b: &[usize], at: bool, bt: bool, shape: &[usize]) {
    let a = values(a, 7);
    let b = values(b, 61);
    let expected = product(
        a.mapv(to_f32).view(),
        b.mapv(to_f32).view(),
        at,
        bt,
        -0.6875,
    )
    .unwrap()
    .mapv(from_f32);
    let Tensor::F16(actual) = matmul(&Tensor::F16(a), &Tensor::F16(b), at, bt, -0.6875).unwrap()
    else {
        panic!("expected half precision");
    };
    assert_eq!(actual.shape(), shape);
    assert_eq!(actual, expected);
}

#[test]
fn half_products_match_single_precision_accumulation() {
    for at in [false, true] {
        for bt in [false, true] {
            let a = if at { [2, 1, 259, 9] } else { [2, 1, 9, 259] };
            let b = if bt {
                [1, 3, 137, 259]
            } else {
                [1, 3, 259, 137]
            };
            compare(&a, &b, at, bt, &[2, 3, 9, 137]);
        }
    }
    compare(&[5], &[5], false, false, &[]);
    compare(&[5], &[2, 5, 131], true, false, &[2, 131]);
    compare(&[3, 5], &[5], false, true, &[3]);
    compare(&[2, 0], &[0, 19], false, false, &[2, 19]);
    compare(&[0, 4], &[4, 7], false, false, &[0, 7]);
    compare(&[2, 4], &[4, 0], false, false, &[2, 0]);
}

#[test]
fn onnx_matmul_broadcasts_and_gemm_transposes_before_adding_bias() {
    use super::super::ops::Op;

    fn half(shape: &[usize], data: &[i16]) -> Tensor {
        let data = data.iter().map(|&x| from_f32(f32::from(x))).collect();
        Tensor::F16(
            ArrayD::from_shape_vec(IxDyn(shape), data)
                .unwrap()
                .into_shared(),
        )
    }

    let a = half(&[2, 1, 1, 2], &[1, 2, 3, 4]);
    let b = half(&[1, 3, 2, 1], &[1, 0, 0, 1, -2, 3]);
    let outputs = Op::MatMul.eval(&[Some(&a), Some(&b)]).unwrap();
    let [Tensor::F16(result)] = outputs.as_slice() else {
        panic!("expected one half tensor")
    };
    let Tensor::F16(expected) = half(&[2, 3, 1, 1], &[1, 2, 4, 3, 4, 6]) else {
        unreachable!()
    };
    assert_eq!(*result, expected);

    let a = half(&[3, 2], &[1, 4, 2, 5, 3, 6]);
    let b = half(&[2, 3], &[2, 0, -2, 0, 2, 0]);
    let bias = half(&[2], &[1, -1]);
    let gemm = Op::Gemm {
        trans_a: true,
        trans_b: true,
        alpha: 0.5,
        beta: 2.0,
    };
    let outputs = gemm.eval(&[Some(&a), Some(&b), Some(&bias)]).unwrap();
    let [Tensor::F16(result)] = outputs.as_slice() else {
        panic!("expected one half tensor")
    };
    let Tensor::F16(expected) = half(&[2, 2], &[0, 0, 0, 3]) else {
        unreachable!()
    };
    assert_eq!(*result, expected);
}

#[test]
fn half_products_reject_invalid_shapes() {
    for (a, b) in [
        (vec![], vec![1]),
        (vec![2, 3], vec![4, 2]),
        (vec![2, 1, 4], vec![3, 4, 1]),
    ] {
        assert!(
            matmul(
                &Tensor::F16(values(&a, 0)),
                &Tensor::F16(values(&b, 0)),
                false,
                false,
                1.0
            )
            .is_err()
        );
    }
}
