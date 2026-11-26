use super::super::{
    model::Model,
    onnx::proto::{
        GraphProto, ModelProto, NodeProto, OperatorSetIdProto, TypeProto, ValueInfoProto,
        type_proto,
    },
    tensor::Tensor,
};
use crate::ai_image_search::{Input, Values};
use prost::Message;

#[test]
fn custom_graphs_execute_named_math_and_activation_outputs() {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("activation.onnx");
    let input = ValueInfoProto {
        name: "samples".into(),
        r#type: Some(TypeProto {
            value: Some(type_proto::Value::TensorType(type_proto::Tensor {
                elem_type: 1,
                shape: None,
            })),
        }),
    };
    for (op, x, y) in [
        ("Acos", 0.5, 1.0471976),
        ("Asin", 0.5, 0.5235988),
        ("Atan", 1., 0.7853982),
        ("Acosh", 2., 1.316958),
        ("Asinh", -2., -1.4436355),
        ("Atanh", 0.5, 0.54930615),
        ("Cosh", -1., 1.5430807),
        ("Sinh", -1., -1.1752012),
        ("Tan", 0.5, 0.5463025),
        ("Round", 2.5, 2.),
        ("Round", -3.5, -4.),
        ("Round", -0.5, -0.),
        ("Sign", -0.25, -1.),
        ("Erf", -0.5, -0.5204999),
        ("Erf", 2., 0.9953223),
        ("Elu", -1., -0.63212055),
        ("Celu", -1., -0.63212055),
        ("LeakyRelu", -1., -0.01),
        ("ThresholdedRelu", 1., 0.),
        ("HardSigmoid", -1., 0.3),
        ("HardSwish", -1., -0.33333334),
        ("Softplus", 100., 100.),
        ("Softsign", -3., -0.75),
        ("Mish", -1., -0.30340147),
        ("Selu", -1., -1.1113307),
        ("Gelu", -1., -0.15865526),
    ] {
        let graph = GraphProto {
            node: vec![NodeProto {
                op_type: op.to_owned(),
                input: vec!["samples".into()],
                output: vec!["encoded".into()],
                ..Default::default()
            }],
            input: vec![input.clone()],
            output: vec![ValueInfoProto {
                name: "encoded".into(),
                ..input.clone()
            }],
            ..Default::default()
        };
        let model = ModelProto {
            ir_version: Some(10),
            opset_import: vec![OperatorSetIdProto {
                domain: String::new(),
                version: 20,
            }],
            graph: Some(graph),
            ..Default::default()
        };
        std::fs::write(&path, model.encode_to_vec()).unwrap();
        let mut inputs = [Input {
            name: "samples",
            shape: &[1],
            values: Values::Float(vec![x]),
        }];
        let Tensor::F32(out) = Model::open(&path, &inputs, "encoded")
            .unwrap()
            .run(&mut inputs)
            .unwrap()
        else {
            panic!("expected float output")
        };
        let actual = out[[0]];
        assert!(
            (actual - y).abs() < 2e-6,
            "{op}({x}) = {actual}, expected {y}"
        );
        if op == "Round" && x == -0.5 {
            assert!(actual.is_sign_negative());
        }
    }
}
