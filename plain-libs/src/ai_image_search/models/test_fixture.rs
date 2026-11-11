//! Test-only fixture: writes the tiny paired ONNX encoders plus their
//! manifest into `directory` and returns the manifest describing them.
//!
//! Exposed through the `test-seams` feature so dependent crates' test
//! binaries (plain-server's image-models tests) can build the same engine
//! package; `#[cfg(test)]` in this crate cannot reach them.

use crate::ai_image_search::models::manifest;
use crate::ai_image_search::models::manifest::*;
use sha2::{Digest, Sha256};
use std::path::Path;

pub fn preprocess() -> Preprocess {
    Preprocess {
        size: 256,
        resize: Resize::Stretch,
        layout: Layout::Nchw,
        interpolation: Interpolation::Bilinear,
        mean: [0.5; 3],
        std: [0.5; 3],
    }
}

pub fn fixture(directory: &Path) -> Manifest {
    std::fs::create_dir_all(directory).unwrap();
    let mut manifest = Manifest {
        format_version: 1,
        id: "fixture".into(),
        name: "Fixture".into(),
        license: "Apache-2.0".into(),
        source: "local".into(),
        dimensions: 4,
        minimum_score: 0.0,
        files: Vec::new(),
        image: ImageEncoder {
            file: "image.onnx".into(),
            input: "pixels".into(),
            output: "embedding".into(),
        },
        text: TextEncoder {
            file: "text.onnx".into(),
            inputs: vec![TextInput {
                name: "ids".into(),
                role: TextRole::Ids,
            }],
            output: "embedding".into(),
        },
        tokenizer: Tokenizer {
            file: "tokenizer.json".into(),
            context_length: 4,
            pad_token: "[PAD]".into(),
        },
        preprocess: Preprocess {
            size: 16,
            ..preprocess()
        },
    };
    let data: [(&str, &[u8]); 3] = [
        (
            "image.onnx",
            include_bytes!("../../../../plain-rs/testdata/image_search/image.onnx"),
        ),
        (
            "text.onnx",
            include_bytes!("../../../../plain-rs/testdata/image_search/text.onnx"),
        ),
        (
            "tokenizer.json",
            include_bytes!("../../../../plain-rs/testdata/image_search/tokenizer.json"),
        ),
    ];
    manifest.files = data
        .into_iter()
        .map(|(name, bytes)| {
            std::fs::write(directory.join(name), bytes).unwrap();
            manifest::File {
                name: name.into(),
                size: bytes.len() as u64,
                sha256: format!("{:x}", Sha256::digest(bytes)),
            }
        })
        .collect();
    std::fs::write(
        directory.join("manifest.json"),
        crate::serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    manifest
}
