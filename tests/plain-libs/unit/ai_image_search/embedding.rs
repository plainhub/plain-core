use super::*;
use crate::ai_image_search::models::package as onnx_package;
use crate::ai_image_search::models::test_fixture;
use crate::ai_image_search::models::test_fixture::fixture;
use crate::ai_image_search::preprocess;

#[cfg(feature = "image-search-inference")]
fn image_package(directory: &Path, model: &[u8]) -> Manifest {
    let mut manifest = fixture(directory);
    std::fs::write(directory.join(&manifest.image.file), model).unwrap();
    let image = manifest
        .files
        .iter_mut()
        .find(|file| file.name == manifest.image.file)
        .unwrap();
    image.size = model.len() as u64;
    image.sha256 = format!("{:x}", Sha256::digest(model));
    manifest.preprocess.size = 16;
    manifest.preprocess.mean = [0.0; 3];
    manifest.preprocess.std = [1.0; 3];
    manifest
}

#[cfg(feature = "image-search-inference")]
fn import_image_encoder(model: &[u8], expected: [f32; 4]) {
    let directory = crate::temporary::tempdir().unwrap();
    let manifest = image_package(directory.path(), model);
    let engine = Engine::open(directory.path().into(), manifest).unwrap();
    let image = directory.path().join("gradient.png");
    crate::image::RgbImage::from_fn(96, 64, |x, y| {
        crate::image::Rgb([
            (x * 255 / 95) as u8,
            (y * 255 / 63) as u8,
            ((x + 2 * y) % 256) as u8,
        ])
    })
    .save(&image)
    .unwrap();
    let result = engine.image(&image).unwrap().unwrap();
    for (value, expected) in result.iter().zip(expected) {
        assert!((value - expected).abs() < 1e-5, "{result:?}");
    }
}

#[cfg(feature = "image-search-inference")]
#[test]
fn importing_a_package_requires_supported_ir_and_declared_opsets() {
    let valid = include_bytes!("../../../../plain-rs/testdata/image_search/image.onnx");
    let graph_and_opset = valid.strip_prefix(&[0x08, 9]).unwrap();
    let no_opset = valid.strip_suffix(&[0x42, 4, 0x0a, 0, 0x10, 17]).unwrap();
    let supported = [&[0x08, 13][..], graph_and_opset].concat();
    let directory = crate::temporary::tempdir().unwrap();
    let manifest = image_package(directory.path(), &supported);
    Engine::open(directory.path().into(), manifest).unwrap();
    let future = [&[0x08, 14][..], graph_and_opset].concat();
    for (model, reason) in [
        (future.as_slice(), "IR version"),
        (graph_and_opset, "IR version"),
        (no_opset, "opset"),
    ] {
        let directory = crate::temporary::tempdir().unwrap();
        let manifest = image_package(directory.path(), model);
        let error = Engine::open(directory.path().into(), manifest)
            .err()
            .expect("model import must fail");
        assert!(error.contains(reason), "{error}");
    }
}

#[cfg(feature = "image-search-inference")]
#[test]
fn imports_a_model_calling_nested_local_functions_with_a_default_attribute() {
    import_image_encoder(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/image_search/local_functions.onnx"
        )),
        [0.51519865, 0.5156901, 0.44847152, 0.51721114],
    );
}

#[cfg(feature = "image-search-inference")]
#[test]
fn imports_a_model_with_a_broadcast_mask_and_variadic_feature_reductions() {
    import_image_encoder(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/image_search/masked.onnx"
        )),
        [0.29799414, 0.298136, 0.14138956, 0.8957251],
    );
}

#[cfg(feature = "image-search-inference")]
#[test]
fn imports_a_layer_normalized_encoder_with_named_statistics() {
    import_image_encoder(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/image_search/layer_normalization.onnx"
        )),
        [0.17164025, 0.110129476, 0.048734095, 0.977771],
    );
}

#[cfg(feature = "image-search-inference")]
#[test]
fn real_runtime_executes_paired_encoders_and_reloads_released_sessions() {
    let directory = crate::temporary::tempdir().unwrap();
    let engine = Engine::open(directory.path().into(), fixture(directory.path())).unwrap();
    let path = directory.path().join("photo.png");
    crate::image::RgbImage::from_pixel(64, 64, crate::image::Rgb([128, 64, 32]))
        .save(&path)
        .unwrap();
    assert_eq!(engine.text("cat").unwrap(), vec![1.0, 0.0, 0.0, 0.0]);
    assert_eq!(
        engine.image(&path).unwrap().unwrap(),
        vec![1.0, 0.0, 0.0, 0.0]
    );
    let ids = engine.tokenizer.encode("cat", true).unwrap();
    assert_eq!(ids.get_ids(), &[3, 0, 0, 0]);
    assert_eq!(
        engine
            .tokenizer
            .encode("cat cat cat cat cat", true)
            .unwrap()
            .len(),
        4
    );
    engine.release_all();
    assert_eq!(engine.text("image").unwrap()[0], 1.0);
    assert_eq!(engine.image(&path).unwrap().unwrap()[0], 1.0);
}
#[cfg(feature = "image-search-inference")]
#[test]
fn drops_the_active_encoder_before_loading_the_other_graph() {
    let directory = crate::temporary::tempdir().unwrap();
    let manifest = fixture(directory.path());
    let engine = Engine::open(directory.path().into(), manifest.clone()).unwrap();
    assert_eq!(
        engine
            .session
            .lock()
            .unwrap()
            .as_ref()
            .map(|(image, _)| *image),
        Some(false)
    );
    let model = directory.path().join(&manifest.image.file);
    let moved = directory.path().join("saved.onnx");
    std::fs::rename(&model, &moved).unwrap();
    assert!(
        engine
            .image_tensor(vec![0.; (3 * manifest.preprocess.size.pow(2)) as usize])
            .is_err()
    );
    assert!(engine.session.lock().unwrap().is_none());
    std::fs::rename(moved, model).unwrap();
    assert_eq!(
        engine
            .image_tensor(vec![0.; (3 * manifest.preprocess.size.pow(2)) as usize])
            .unwrap(),
        [1., 0., 0., 0.]
    );
    assert_eq!(
        engine
            .session
            .lock()
            .unwrap()
            .as_ref()
            .map(|(image, _)| *image),
        Some(true)
    );
    assert_eq!(engine.text("cat").unwrap(), [1., 0., 0., 0.]);
}
#[cfg(feature = "image-search-inference")]
#[test]
fn packages_reject_corruption_wrong_shapes_and_path_traversal() {
    let directory = crate::temporary::tempdir().unwrap();
    let mut manifest = fixture(directory.path());
    manifest.files[0].name = "../image.onnx".into();
    assert!(manifest.validate().is_err());
    manifest = fixture(directory.path());
    manifest.dimensions = 5;
    assert!(
        Engine::open(directory.path().into(), manifest)
            .err()
            .unwrap()
            .contains("embedding")
    );
    manifest = fixture(directory.path());
    std::fs::write(
        directory.path().join("image.onnx"),
        vec![0; manifest.files[0].size as usize],
    )
    .unwrap();
    assert!(
        Engine::open(directory.path().into(), manifest)
            .err()
            .unwrap()
            .contains("checksum")
    );
}
#[test]
fn external_tensor_files_are_rejected_before_loading() {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("external.onnx");
    std::fs::write(&path, [0x3a, 4, 0x2a, 2, 0x6a, 0]).unwrap();
    assert!(
        onnx_package::validate(&path)
            .unwrap_err()
            .contains("External")
    );
    std::fs::write(&path, [0x3a, 255]).unwrap();
    assert!(onnx_package::validate(&path).is_err());
}
#[test]
fn importing_a_package_rejects_external_weights_in_a_function_default() {
    let directory = crate::temporary::tempdir().unwrap();
    let mut manifest = fixture(directory.path());
    let mut graph = std::fs::read(directory.path().join(&manifest.image.file)).unwrap();
    // Model.functions -> Function.attribute_proto -> Attribute.t -> Tensor.data_location.
    graph.extend_from_slice(&[0xca, 0x01, 0x06, 0x5a, 0x04, 0x2a, 0x02, 0x70, 0x01]);
    std::fs::write(directory.path().join(&manifest.image.file), &graph).unwrap();
    let image = manifest
        .files
        .iter_mut()
        .find(|v| v.name == manifest.image.file)
        .unwrap();
    image.size = graph.len() as u64;
    image.sha256 = format!("{:x}", Sha256::digest(&graph));
    let error = Engine::open(directory.path().into(), manifest)
        .err()
        .unwrap();
    assert!(error.contains("External ONNX weights"), "{error}");
}

#[test]
fn normalization_handles_extreme_floats_and_rejects_invalid_embeddings() {
    let mut vector = [f32::MAX, f32::MAX];
    normalize(&mut vector).unwrap();
    assert!((vector[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    for mut values in [vec![0.0], vec![f32::NAN], vec![f32::INFINITY], vec![]] {
        assert!(normalize(&mut values).is_err());
    }
}
#[test]
fn preprocessing_uses_manifest_normalization_and_skips_tiny_images() {
    let directory = crate::temporary::tempdir().unwrap();
    let config = test_fixture::preprocess();
    let path = directory.path().join("photo.png");
    crate::image::RgbImage::from_pixel(64, 64, crate::image::Rgb([255, 0, 128]))
        .save(&path)
        .unwrap();
    let values = preprocess::tensor(&path, &config).unwrap().unwrap();
    let plane = config.size.pow(2) as usize;
    assert_eq!(values[0], 1.0);
    assert_eq!(values[plane], -1.0);
    assert!((values[2 * plane] - 1.0 / 255.0).abs() < 1e-6);
    crate::image::RgbImage::new(32, 32).save(&path).unwrap();
    assert!(preprocess::tensor(&path, &config).unwrap().is_none());
}

#[test]
fn manifest_reader_bounds_input_before_parsing() {
    let directory = crate::temporary::tempdir().unwrap();
    fixture(directory.path());
    let path = directory.path().join("manifest.json");
    assert!(Manifest::read(&path).is_ok());
    std::fs::write(&path, vec![b' '; 65537]).unwrap();
    assert!(Manifest::read(&path).err().unwrap().contains("64 KiB"));
}
