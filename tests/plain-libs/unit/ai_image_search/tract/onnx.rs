use super::*;

fn int<'a>(name: &'a str, shape: &'a [i64]) -> Input<'a> {
    Input {
        name,
        shape,
        values: Values::Integer(Vec::new()),
    }
}

#[test]
fn resolves_manifest_inputs_and_output_in_graph_order() {
    let dir = crate::temporary::tempdir().unwrap();
    let path = dir.path().join("encoder.onnx");
    std::fs::write(
        &path,
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/image_search/named_text.onnx"
        )),
    )
    .unwrap();
    let graph = read(&path).unwrap().graph.unwrap();
    let inputs = [
        int("tokens", &[1, 6]),
        int("segments", &[1, 6]),
        int("attention", &[1, 6]),
    ];
    let bindings = graph.bind(&inputs, "embedding").unwrap();
    assert_eq!(bindings.input_order, [1, 2, 0]);
    assert_eq!(bindings.output, 1);
    assert!(graph.bind(&inputs, "missing").is_err());
    assert!(graph.bind(&inputs[..2], "embedding").is_err());
    assert!(
        graph
            .bind(
                &[
                    int("tokens", &[1, 5]),
                    int("segments", &[1, 6]),
                    int("attention", &[1, 6])
                ],
                "embedding"
            )
            .is_err()
    );
}

#[test]
#[ignore = "requires the original SigLIP2 files in PLAIN_TEST_SIGLIP2_PACKAGE"]
fn resolves_all_siglip2_input_symbols_from_actual_shapes() {
    let dir = std::path::PathBuf::from(std::env::var_os("PLAIN_TEST_SIGLIP2_PACKAGE").unwrap());
    for (file, input, expected) in [
        (
            "image.onnx",
            Input {
                name: "pixel_values",
                shape: &[1, 3, 256, 256],
                values: Values::Float(Vec::new()),
            },
            vec![
                ("batch_size", 1),
                ("num_channels", 3),
                ("height", 256),
                ("width", 256),
            ],
        ),
        (
            "text.onnx",
            int("input_ids", &[1, 64]),
            vec![("batch_size", 1), ("sequence_length", 64)],
        ),
    ] {
        let graph = read(&dir.join(file)).unwrap().graph.unwrap();
        let binding = graph.bind(&[input], "pooler_output").unwrap();
        assert_eq!(binding.output, 1);
        assert_eq!(binding.input_order, [0]);
        assert_eq!(
            binding.symbols,
            expected
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect()
        );
    }
}
