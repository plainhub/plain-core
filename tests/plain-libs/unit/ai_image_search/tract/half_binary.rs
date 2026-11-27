use super::{Binary, eval, portable};
use crate::ai_image_search::tract::tensor::array;
use ndarray::{Axis, IxDyn};

#[test]
fn binary_reads_strided_and_broadcast_views_in_logical_order() {
    let a = array(&[12, 3], (0..36).map(|i| 0x3400 + i * 17).collect())
        .unwrap()
        .reversed_axes();
    let b = array(&[3, 1], vec![0x3c00, 0x8400, 0x7e01]).unwrap();
    let dims = [3, 12];
    for op in [Binary::Add, Binary::Sub, Binary::Mul] {
        let lhs = a.broadcast(IxDyn(&dims)).unwrap();
        let rhs = b.broadcast(IxDyn(&dims)).unwrap();
        let actual = eval(op, lhs.clone(), rhs.clone()).unwrap();
        let expected = portable(op, lhs, rhs);
        assert_eq!(actual, expected);
    }
    assert_eq!(a.strides()[1], 3);
    assert_eq!(b.broadcast(IxDyn(&dims)).unwrap().stride_of(Axis(1)), 0);
}

#[test]
fn binary_preserves_half_rounding_and_special_values_across_vector_boundaries() {
    let edges = [
        0x0000, 0x8000, 0x0001, 0x83ff, 0x0400, 0x3c00, 0x1000, 0x7bff, 0xfbff, 0x0001, 0x0400,
        0x3555, 0x3c00, 0x7bff, 0x7c00, 0x7e11, 0xfc00, 0xfd01,
    ];
    for op in [
        Binary::Add,
        Binary::Sub,
        Binary::Mul,
        Binary::Div,
        Binary::Pow,
        Binary::Min,
        Binary::Max,
    ] {
        for rhs in edges {
            let a = array(&[edges.len()], edges.to_vec()).unwrap();
            let b = array(&[1], vec![rhs]).unwrap();
            let b = b.broadcast(IxDyn(&[edges.len()])).unwrap();
            assert_eq!(
                eval(op, a.view(), b.clone()).unwrap(),
                portable(op, a.view(), b)
            );
        }
    }
}

#[cfg(target_arch = "aarch64")]
#[test]
fn native_lanes_match_portable_half_outputs_with_broadcast_and_tails() {
    if !std::arch::is_aarch64_feature_detected!("fp16") {
        return;
    }
    let a = array(
        &[2, 4099],
        (0u32..8198).map(|i| i.wrapping_mul(39157) as u16).collect(),
    )
    .unwrap();
    let b = array(
        &[2, 4099],
        (0u32..8198)
            .map(|i| i.wrapping_mul(18719).wrapping_add(13577) as u16)
            .collect(),
    )
    .unwrap();
    let bias = array(&[2, 1], vec![0x3555, 0x8001]).unwrap();
    for op in [Binary::Add, Binary::Sub, Binary::Mul] {
        for rhs in [b.view(), bias.broadcast(IxDyn(&[2, 4099])).unwrap()] {
            let fast = super::neon::eval(op, a.view(), rhs.clone())
                .expect("native lane path")
                .unwrap();
            assert_eq!(fast, portable(op, a.view(), rhs.clone()));
            let reverse = super::neon::eval(op, rhs.clone(), a.view())
                .expect("native reverse lane path")
                .unwrap();
            assert_eq!(reverse, portable(op, rhs, a.view()));
        }
    }
}
