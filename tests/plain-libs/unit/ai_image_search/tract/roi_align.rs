use super::*;
use ndarray::{array, s};

fn roi() -> RoiAlign {
    RoiAlign {
        output_height: 1,
        output_width: 1,
        sampling_ratio: 1,
        spatial_scale: 1.0,
        half_pixel: true,
        pooling: Pooling::Average,
    }
}

#[test]
fn pools_fractional_coordinates_from_the_selected_batch() {
    let image = Array4::from_shape_fn((2, 2, 4, 4), |(n, c, y, x)| {
        (100 * n + 20 * c + 4 * y + x) as f32
    });
    let boxes = array![[0.5, 0.5, 2.5, 2.5], [1., 1., 3., 3.]];
    let batches = array![1, 0];
    let pooled = RoiAlign {
        output_width: 2,
        output_height: 2,
        sampling_ratio: 0,
        ..roi()
    }
    .eval(image.view(), boxes.view(), batches.view())
    .unwrap();
    assert_eq!(
        pooled.slice(s![0, 0, .., ..]),
        array![[102.5, 103.5], [106.5, 107.5]]
    );
    assert_eq!(
        pooled.slice(s![1, 1, .., ..]),
        array![[25., 26.], [29., 30.]]
    );
}

#[test]
fn applies_coordinate_mode_scale_padding_and_weighted_maximum() {
    let image = array![[1.0_f64, 2.0], [3.0, 4.0]]
        .into_shape_with_order((1, 1, 2, 2))
        .unwrap();
    let boxes = array![[0., 0., 4., 4.], [-20., -20., -18., -18.]];
    let batch = array![0, 0];
    let run = |op: RoiAlign| op.eval(image.view(), boxes.view(), batch.view()).unwrap();
    let average = run(RoiAlign {
        spatial_scale: 0.5,
        ..roi()
    });
    assert_eq!(average.into_raw_vec_and_offset().0, [2.5, 0.0]);
    let legacy = run(RoiAlign {
        half_pixel: false,
        spatial_scale: 0.5,
        ..roi()
    });
    assert_eq!(legacy.into_raw_vec_and_offset().0, [4.0, 0.0]);
    let maximum = run(RoiAlign {
        pooling: Pooling::Maximum,
        spatial_scale: 0.5,
        ..roi()
    });
    assert_eq!(maximum.into_raw_vec_and_offset().0, [1.0, 0.0]);
}

#[test]
fn validates_region_shapes_coordinates_and_batch_indices() {
    let image = Array4::<f32>::zeros((1, 1, 2, 2));
    for (boxes, indices) in [
        (array![[0., 0., 1., 1.]], array![-1]),
        (array![[0., 0., 1., 1.]], array![1]),
        (array![[f32::NAN, 0., 1., 1.]], array![0]),
        (array![[0., 0., 1.]], array![0]),
        (array![[0., 0., 1., 1.]], array![0, 0]),
    ] {
        assert!(
            roi()
                .eval(image.view(), boxes.view(), indices.view())
                .is_err()
        );
    }
}

#[test]
fn loads_region_coordinates_and_batch_indices_from_an_onnx_encoder() {
    use super::super::onnx;
    use ndarray::Array4;
    use std::collections::HashMap;

    let dir = crate::temporary::tempdir().unwrap();
    let path = dir.path().join("encoder.onnx");
    std::fs::write(
        &path,
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/image_search/roi_align.onnx"
        )),
    )
    .unwrap();
    let model = onnx::read(&path).unwrap();
    let graph = model.graph.unwrap();
    let node = graph.node.iter().find(|n| n.op_type == "RoiAlign").unwrap();
    let op = RoiAlign::from_node(node, model.opset_import[0].version).unwrap();
    let initializers: HashMap<_, _> = graph
        .initializer
        .into_iter()
        .map(|t| (t.name.clone(), Tensor::from_proto(t).unwrap()))
        .collect();
    let image = Array4::from_shape_fn((1, 3, 16, 16), |(_, c, y, x)| (c * 256 + y * 16 + x) as f32);
    let result = op
        .evaluate(
            &Tensor::F32(image.into_dyn().into_shared()),
            &initializers[&node.input[1]],
            &initializers[&node.input[2]],
        )
        .unwrap();
    let Tensor::F32(result) = result else {
        panic!("unexpected output type")
    };
    assert_eq!(result.shape(), [1, 3, 1, 1]);
    for (&v, expected) in result.iter().zip([119., 375., 631.]) {
        assert!((v - expected).abs() < 0.001, "{v} ({expected})");
    }
}
