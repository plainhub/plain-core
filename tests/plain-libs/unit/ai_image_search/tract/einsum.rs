use super::*;

fn ints(shape: &[usize], data: &[i32]) -> Tensor {
    Tensor::I32(array(shape, data.to_vec()).unwrap())
}

fn evaluate(equation: &str, inputs: &[Tensor]) -> Result<Array<i32>, String> {
    let inputs = inputs.iter().map(Some).collect::<Vec<_>>();
    match EinSum::parse(equation)?.eval(&inputs)? {
        Tensor::I32(values) => Ok(values),
        _ => panic!("Expected int32"),
    }
}

#[test]
fn sums_diagonals_and_permutes_implicit_axes() {
    let a = ints(&[2, 2], &[1, 2, 3, 4]);
    assert_eq!(evaluate("ii->", std::slice::from_ref(&a)).unwrap()[[]], 5);
    assert_eq!(
        evaluate("ji", &[a]).unwrap(),
        array(&[2, 2], vec![1, 3, 2, 4]).unwrap()
    );
}

#[test]
fn broadcasts_ellipses_and_contracts_three_operands() {
    let a = ints(&[2, 1, 2], &[1, 2, 3, 4]);
    let b = ints(&[2, 2], &[2, 1, 0, 3]);
    let c = ints(&[], &[2]);
    assert_eq!(
        evaluate("...ij,...jk, -> ...ik", &[a, b, c]).unwrap(),
        array(&[2, 1, 2], vec![4, 14, 12, 30]).unwrap()
    );
    let a = ints(&[2, 1], &[2, 3]);
    let b = ints(&[2, 3, 2], &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    assert_eq!(
        evaluate("i...,i...", &[a, b]).unwrap(),
        array(&[3, 2], vec![23, 28, 33, 38, 43, 48]).unwrap()
    );
}

#[test]
fn handles_scalar_and_empty_reductions() {
    assert_eq!(
        evaluate(",->", &[ints(&[], &[2]), ints(&[], &[3])]).unwrap()[[]],
        6
    );
    assert_eq!(
        evaluate("ik,kj->ij", &[ints(&[2, 0], &[]), ints(&[0, 2], &[])]).unwrap(),
        array(&[2, 2], vec![0; 4]).unwrap()
    );
    assert_eq!(
        evaluate("i,i->i", &[ints(&[0], &[]), ints(&[1], &[1])])
            .unwrap()
            .shape(),
        &[0]
    );
}

#[test]
fn rejects_invalid_subscripts_and_dimensions() {
    for eq in ["i->j", "i->ii", "......", "i.."] {
        assert!(EinSum::parse(eq).is_err());
    }
    assert!(evaluate("ii", &[ints(&[2, 1], &[1, 2])]).is_err());
    assert!(evaluate("i,i", &[ints(&[2], &[1, 2]), ints(&[3], &[1, 2, 3])]).is_err());
}

#[test]
fn extracts_batch_diagonals_and_multiplies_matrices() {
    let a = ints(&[2, 2, 2], &[1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(
        evaluate("...ii->...i", std::slice::from_ref(&a)).unwrap(),
        array(&[2, 2], vec![1, 4, 5, 8]).unwrap()
    );
    let b = ints(&[2, 2, 1], &[2, 3, 4, 5]);
    assert_eq!(
        evaluate("bij,bjk->bik", &[a, b]).unwrap(),
        array(&[2, 2, 1], vec![8, 18, 50, 68]).unwrap()
    );
    assert_eq!(
        evaluate("i,i", &[ints(&[2], &[2, 3]), ints(&[2], &[5, 7])]).unwrap()[[]],
        31
    );
}
