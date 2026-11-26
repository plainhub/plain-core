use super::*;

#[test]
fn executes_region_encoder_through_its_named_embedding_output() {
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
    let mut inputs = [Input {
        name: "pixels",
        shape: &[1, 3, 16, 16],
        values: Values::Float(vec![0.; 768]),
    }];
    let model = Model::open(&path, &inputs, "embedding").unwrap();
    let Tensor::F32(out) = model.run(&mut inputs).unwrap() else {
        panic!("expected f32 output")
    };
    assert_eq!(out.shape(), [1, 4]);
    assert_eq!(out.as_slice().unwrap(), [0., 0., 0., 1.]);
    inputs[0].values = Values::Float((0..768).map(|v| v as f32).collect());
    let Tensor::F32(out) = model.run(&mut inputs).unwrap() else {
        panic!("expected f32 output")
    };
    for (&v, expected) in out.iter().zip([119., 375., 631., 1.]) {
        assert!((v - expected).abs() < 0.001, "{v} ({expected})");
    }
}

#[test]
fn runs_a_custom_pair_with_nhwc_and_permuted_named_text_inputs() {
    let dir = crate::temporary::tempdir().unwrap();
    let image_path = dir.path().join("image.onnx");
    let text_path = dir.path().join("text.onnx");
    std::fs::write(
        &image_path,
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/image_search/nhwc_mean.onnx"
        )),
    )
    .unwrap();
    std::fs::write(
        &text_path,
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/image_search/named_text.onnx"
        )),
    )
    .unwrap();
    let mut pixels = [Input {
        name: "pixels",
        shape: &[1, 24, 24, 3],
        values: Values::Float([1., 0., 0.].repeat(24 * 24)),
    }];
    let model = Model::open(&image_path, &pixels, "embedding").unwrap();
    let Tensor::F32(red) = model.run(&mut pixels).unwrap() else {
        panic!("expected float output")
    };
    assert_eq!(red.as_slice().unwrap(), [1., 0., 0., 1.]);
    drop(model);
    let mut tokens = [
        Input {
            name: "tokens",
            shape: &[1, 6],
            values: Values::Integer(vec![3, 0, 0, 0, 0, 0]),
        },
        Input {
            name: "segments",
            shape: &[1, 6],
            values: Values::Integer(vec![0; 6]),
        },
        Input {
            name: "attention",
            shape: &[1, 6],
            values: Values::Integer(vec![1, 0, 0, 0, 0, 0]),
        },
    ];
    let model = Model::open(&text_path, &tokens, "embedding").unwrap();
    let Tensor::F32(text) = model.run(&mut tokens).unwrap() else {
        panic!("expected float output")
    };
    assert_eq!(text.as_slice().unwrap(), [3., 1., 0., 1.]);
}

#[test]
#[ignore = "requires the original SigLIP2 files in PLAIN_TEST_SIGLIP2_PACKAGE"]
fn original_text_encoder_matches_ort() {
    let package = std::path::PathBuf::from(std::env::var_os("PLAIN_TEST_SIGLIP2_PACKAGE").unwrap());
    let mut ids = vec![0; 64];
    ids[..5].copy_from_slice(&[2, 103, 1284, 890, 43]);
    let mut inputs = [Input {
        name: "input_ids",
        shape: &[1, 64],
        values: Values::Integer(ids),
    }];
    let model = Model::open(&package.join("text.onnx"), &inputs, "pooler_output").unwrap();
    let Tensor::F32(output) = model.run(&mut inputs).unwrap() else {
        panic!("expected FP32 output")
    };
    assert_eq!(output.shape(), [1, 768]);
    let golden = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/image_search/siglip2_text.f32be"
    ));
    compare_embedding(&output, golden, "text");
}

#[test]
#[ignore = "requires the original SigLIP2 files in PLAIN_TEST_SIGLIP2_PACKAGE"]
fn original_vision_encoder_matches_ort() {
    let package = std::path::PathBuf::from(std::env::var_os("PLAIN_TEST_SIGLIP2_PACKAGE").unwrap());
    let values = (0..3)
        .flat_map(|c| {
            (0..256).flat_map(move |y| {
                (0..256).map(move |x| ((x * 3 + y * 7 + c * 37) % 256) as f32 / 127.5 - 1.0)
            })
        })
        .collect();
    let mut inputs = [Input {
        name: "pixel_values",
        shape: &[1, 3, 256, 256],
        values: Values::Float(values),
    }];
    let model = Model::open(&package.join("image.onnx"), &inputs, "pooler_output").unwrap();
    let Tensor::F32(output) = model.run(&mut inputs).unwrap() else {
        panic!("expected FP32 output")
    };
    assert_eq!(output.shape(), [1, 768]);
    let golden = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/image_search/siglip2_vision.f32be"
    ));
    compare_embedding(&output, golden, "vision");
}

fn compare_embedding(output: &super::super::tensor::Array<f32>, golden: &[u8], name: &str) {
    let mut dot = 0.;
    let mut old_norm = 0.;
    let mut new_norm = 0.;
    for (&v, bytes) in output.iter().zip(golden.chunks_exact(4)) {
        let reference = f64::from(f32::from_be_bytes(bytes.try_into().unwrap()));
        let current = f64::from(v);
        dot += current * reference;
        new_norm += current * current;
        old_norm += reference * reference;
    }
    let cosine = dot / (old_norm * new_norm).sqrt();
    eprintln!("{name} embedding cosine {cosine:.8}");
    assert!(cosine > 0.999, "{name} embedding cosine {cosine}");
}

#[test]
fn normalization_preserves_extra_broadcast_dimensions_in_a_custom_graph() {
    use super::onnx::proto::{
        AttributeProto, GraphProto, ModelProto, NodeProto, OperatorSetIdProto, TensorProto,
        TensorShapeProto, TypeProto, ValueInfoProto, tensor_shape_proto, type_proto,
    };
    use prost::Message;
    let value = |name: &str, dimensions: &[i64]| ValueInfoProto {
        name: name.into(),
        r#type: Some(TypeProto {
            value: Some(type_proto::Value::TensorType(type_proto::Tensor {
                elem_type: 1,
                shape: Some(TensorShapeProto {
                    dim: dimensions
                        .iter()
                        .map(|&n| tensor_shape_proto::Dimension {
                            value: Some(tensor_shape_proto::dimension::Value::DimValue(n)),
                            ..Default::default()
                        })
                        .collect(),
                }),
            })),
        }),
    };
    let node = |op: &str, inputs: &[&str], out: &str| NodeProto {
        op_type: op.into(),
        input: inputs.iter().map(|v| v.to_string()).collect(),
        output: vec![out.into()],
        attribute: if op == "ReduceMean" {
            vec![AttributeProto {
                name: "axes".into(),
                r#type: 7,
                ints: vec![-1],
                ..Default::default()
            }]
        } else {
            vec![]
        },
        ..Default::default()
    };
    let graph = GraphProto {
        input: vec![value("samples", &[3])],
        output: vec![value("result", &[1, 3])],
        initializer: vec![TensorProto {
            name: "epsilon".into(),
            data_type: 1,
            float_data: vec![1.0],
            dims: vec![1, 1],
            ..Default::default()
        }],
        node: vec![
            node("ReduceMean", &["samples"], "mean"),
            node("Sub", &["samples", "mean"], "diff"),
            node("Mul", &["diff", "diff"], "sq"),
            node("ReduceMean", &["sq"], "var"),
            node("Add", &["epsilon", "var"], "sum"),
            node("Sqrt", &["sum"], "den"),
            node("Div", &["diff", "den"], "result"),
        ],
        ..Default::default()
    };
    let model = ModelProto {
        ir_version: Some(8),
        opset_import: vec![OperatorSetIdProto {
            domain: String::new(),
            version: 16,
        }],
        graph: Some(graph),
        ..Default::default()
    };
    let dir = crate::temporary::tempdir().unwrap();
    let path = dir.path().join("encoder.onnx");
    std::fs::write(&path, model.encode_to_vec()).unwrap();
    let mut inputs = [Input {
        name: "samples",
        shape: &[3],
        values: Values::Float(vec![-1., 0., 1.]),
    }];
    let Tensor::F32(result) = Model::open(&path, &inputs, "result")
        .unwrap()
        .run(&mut inputs)
        .unwrap()
    else {
        panic!("expected float output")
    };
    assert_eq!(result.shape(), [1, 3]);
    assert!((result[[0, 2]] - (3_f32 / 5.).sqrt()).abs() < 1e-6);
}

#[test]
fn importing_two_text_inputs_binds_ids_and_attention_by_name() {
    use super::onnx::proto::{ModelProto, TensorProto};
    use crate::ai_image_search::{embedding::Engine, models::manifest::Manifest};
    use prost::Message;
    use sha2::{Digest, Sha256};
    let dir = crate::temporary::tempdir().unwrap();
    macro_rules! fixture {
        ($name:literal) => {
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../plain-rs/testdata/image_search/",
                $name
            ))
        };
    }
    let mut manifest = Manifest::parse(fixture!("named_inputs.json")).unwrap();
    std::fs::write(
        dir.path().join(&manifest.image.file),
        fixture!("nhwc_mean.onnx"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join(&manifest.tokenizer.file),
        fixture!("tokenizer.json"),
    )
    .unwrap();
    let mut model = ModelProto::decode(&fixture!("named_text.onnx")[..]).unwrap();
    let graph = model.graph.as_mut().unwrap();
    graph.input.retain(|v| v.name != "segments");
    graph.initializer.push(TensorProto {
        name: "segments".into(),
        dims: vec![1, 6],
        data_type: 7,
        int64_data: vec![0; 6],
        ..Default::default()
    });
    assert_eq!(
        graph
            .input
            .iter()
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>(),
        ["attention", "tokens"]
    );
    manifest.text.inputs.retain(|v| v.name != "segments");
    assert_eq!(
        manifest
            .text
            .inputs
            .iter()
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>(),
        ["tokens", "attention"]
    );
    let bytes = model.encode_to_vec();
    std::fs::write(dir.path().join(&manifest.text.file), &bytes).unwrap();
    let file = manifest
        .files
        .iter_mut()
        .find(|f| f.name == manifest.text.file)
        .unwrap();
    file.sha256 = format!("{:x}", Sha256::digest(&bytes));
    file.size = bytes.len() as u64;
    let engine = Engine::open(dir.path().into(), manifest).unwrap();
    let vector = engine.text("cat").unwrap();
    for (value, expected) in vector.iter().zip([3., 1., 0., 1.]) {
        assert!(
            (value - expected / 11_f32.sqrt()).abs() < 1e-6,
            "{vector:?}"
        );
    }
}
