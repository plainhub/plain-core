use super::*;

fn float(shape: &[usize], data: &[f32]) -> Tensor {
    Tensor::F32(array(shape, data.to_vec()).unwrap())
}

fn attention(causal: bool) -> QAttention {
    QAttention {
        heads: 2,
        causal,
        scale: 0.,
        mask_value: -10000.,
        present: true,
        shared_past: false,
    }
}

fn run(op: &QAttention, values: &[Option<Tensor>]) -> Result<Vec<Tensor>, String> {
    op.eval(&values.iter().map(Option::as_ref).collect::<Vec<_>>())
}

fn inputs() -> Vec<Option<Tensor>> {
    vec![
        Some(Tensor::U8(array(&[1, 2, 1], vec![6, 8]).unwrap())),
        Some(Tensor::I8(array(&[1, 6], vec![1, 2, 3, 4, -2, 5]).unwrap())),
        Some(float(&[6], &[0., 0., 0., 0., 1., -1.])),
        Some(float(&[], &[0.5])),
        Some(float(&[6], &[2., 2., 2., 2., 0.25, 2.])),
        None,
        Some(Tensor::U8(array(&[1], vec![4]).unwrap())),
        Some(Tensor::I8(array(&[6], vec![1, 2, 3, 4, 2, 3]).unwrap())),
    ]
}

#[test]
fn handles_per_column_signed_projection_and_causal_multihead_attention() {
    let outputs = run(&attention(true), &inputs()).unwrap();
    let Tensor::F32(output) = &outputs[0] else {
        panic!()
    };
    assert_eq!(*output, array(&[1, 2, 2], vec![0., 3., -0.5, 5.]).unwrap());
    let Tensor::F32(present) = &outputs[1] else {
        panic!()
    };
    assert_eq!(
        *present,
        array(&[2, 1, 2, 2, 1], vec![0., 0., 0., 0., 0., -1., 3., 7.]).unwrap()
    );
}

#[test]
fn applies_key_ranges_and_query_masks() {
    for (shape, mask, want) in [
        (vec![2], vec![2, 1], vec![-1., 7., -1., 7.]),
        (vec![1, 2], vec![1, 0], vec![0., 3., 0., 3.]),
        (vec![1, 2, 2], vec![0, 1, 1, 0], vec![-1., 7., 0., 3.]),
    ] {
        let mut x = inputs();
        x[5] = Some(Tensor::I32(array(&shape, mask).unwrap()));
        let result = run(&attention(false), &x).unwrap();
        let Tensor::F32(output) = &result[0] else {
            panic!()
        };
        assert_eq!(*output, array(&[1, 2, 2], want).unwrap());
    }
}

#[test]
fn attends_to_past_keys_and_appends_values_by_head() {
    let mut x = inputs();
    x.push(Some(float(&[2, 1, 2, 1, 1], &[1., 2., 4., 5.])));
    let outputs = run(&attention(true), &x).unwrap();
    let Tensor::F32(out) = &outputs[0] else {
        panic!()
    };
    assert_eq!(*out, array(&[1, 2, 2], vec![2., 4., 1., 5.]).unwrap());
    let Tensor::F32(state) = &outputs[1] else {
        panic!()
    };
    assert_eq!(
        *state,
        array(
            &[2, 1, 2, 3, 1],
            vec![1., 0., 0., 2., 0., 0., 4., 0., -1., 5., 3., 7.]
        )
        .unwrap()
    );
}

#[test]
fn rejects_mismatched_quantization_and_past_parameters() {
    let mut x = inputs();
    x[4] = Some(float(&[2], &[1., 1.]));
    assert!(run(&attention(false), &x).is_err());
    let mut x = inputs();
    x.push(Some(float(&[2, 1, 1, 1, 1], &[0., 1.])));
    assert!(run(&attention(false), &x).is_err());
}
