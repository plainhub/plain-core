use super::super::onnx::proto::{NodeProto, ValueInfoProto};
use super::*;

fn node(inputs: &[&str], outputs: &[&str]) -> NodeProto {
    NodeProto {
        input: inputs.iter().map(|s| s.to_string()).collect(),
        output: outputs.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

#[test]
fn evaluates_requested_output_in_dependency_order_and_releases_intermediates() {
    let graph = GraphProto {
        input: vec![ValueInfoProto {
            name: "in".into(),
            ..Default::default()
        }],
        output: vec![ValueInfoProto {
            name: "value".into(),
            ..Default::default()
        }],
        node: vec![
            node(&["left", "right"], &["value"]),
            node(&["in"], &["shared", "extra"]),
            node(&["shared"], &["left"]),
            node(&["shared"], &["right"]),
            node(&["extra"], &["unselected"]),
        ],
        ..Default::default()
    };
    let plan = Plan::for_outputs(&graph, &[0]).unwrap();
    assert_eq!(plan.order, [1, 2, 3, 0]);
    assert!(plan.release[2].contains(&plan.names["shared"].slot));
    assert!(plan.release[0].contains(&plan.names["extra"].slot));
    assert!(
        !plan
            .release
            .iter()
            .flatten()
            .any(|slot| *slot == plan.outputs[0])
    );
}

#[test]
fn a_cycle_or_missing_producer_fails_during_planning() {
    for nodes in [
        vec![node(&["x"], &["value"]), node(&["value"], &["x"])],
        vec![node(&["x"], &["value"])],
    ] {
        let graph = GraphProto {
            output: vec![ValueInfoProto {
                name: "value".into(),
                ..Default::default()
            }],
            node: nodes,
            ..Default::default()
        };
        assert!(Plan::for_outputs(&graph, &[0]).is_err());
    }
}
