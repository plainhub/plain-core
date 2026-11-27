use super::{
    embedding,
    tensor::{Tensor, array},
};

#[test]
fn normalizes_the_owned_contiguous_embedding_without_reallocating_it() {
    let output = array(&[1, 2], vec![3.0, 4.0]).unwrap();
    let address = output.as_ptr();
    let actual = embedding(Tensor::F32(output), 2).unwrap();
    assert_eq!(actual.as_ptr(), address);
    assert_eq!(actual, [0.6, 0.8]);
}

#[test]
fn never_mutates_a_shared_output_and_respects_strided_order() {
    let output = array(&[1, 4], vec![3.0, 99.0, 4.0, 99.0])
        .unwrap()
        .slice_move(ndarray::s![.., ..;2])
        .into_dyn();
    assert!(!output.is_standard_layout());
    let result = embedding(Tensor::F32(output.clone()), 2).unwrap();
    assert_eq!(result, [0.6, 0.8]);
    assert_eq!(output[[0, 0]], 3.0);
    assert_eq!(output[[0, 1]], 4.0);
}
