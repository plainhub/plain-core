use super::*;
use plain_rs::features::media::images::embeddings::{self, EmbeddingInput};
use plain_rs::features::services::images::{
    index::ImageIndex, models::Runtime, status::ImageSearchStatusType,
};
use std::{path::Path, time::Duration};

#[tokio::test]
async fn imported_named_encoders_search_existing_and_new_image_vectors() {
    let (dir, schema, db) = fixture_seeded(
        GRANTED,
        true,
        |_| {},
        |method, _| panic!("unexpected host call {method}"),
    );
    let index = schema.data::<Arc<ImageIndex>>().unwrap();
    index.use_native_catalog_for_test();
    let models = schema.data::<Arc<Runtime>>().unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../plain-rs/testdata/image_search");
    let incoming = dir.path().join("ai_models/incoming");
    std::fs::create_dir_all(&incoming).unwrap();
    for name in ["nhwc_mean.onnx", "named_text.onnx", "tokenizer.json"] {
        std::fs::copy(fixtures.join(name), incoming.join(name)).unwrap();
    }
    std::fs::copy(
        fixtures.join("named_inputs.json"),
        incoming.join("manifest.json"),
    )
    .unwrap();

    let black = dir.path().join("black.png");
    let red = dir.path().join("red.png");
    plain_libs::image::RgbImage::from_pixel(64, 64, plain_libs::image::Rgb([255, 0, 0]))
        .save(&red)
        .unwrap();
    let rows = db
        .record_media_files(&[
            media(DataType::Image, black.to_str().unwrap(), "black", 200, "b1"),
            media(DataType::Image, red.to_str().unwrap(), "red", 200, "b1"),
        ])
        .unwrap();

    // Frozen package fingerprint and black-image embedding from the previous runtime.
    db.key_values()
        .set_json(
            "image_index_model",
            "2c9ed73be2904e0c3e8b16b0cb8b5d8f83ec0cf1f35c067d369864a1bab22541",
        )
        .unwrap();
    embeddings::save(
        &db,
        &[EmbeddingInput {
            id: rows[0].id.clone(),
            path: rows[0].path.clone(),
            embedding_base64: "AAAAAAAAAAAAAAAAP4AAAA==".into(),
        }],
    )
    .unwrap();
    import_and_index(&schema).await;

    let hits = models.search("cat").await.unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].image_id, rows[1].id);
    assert!((hits[0].score - 4.0 / 22.0_f32.sqrt()).abs() < 1e-6);
    assert_eq!(hits[1].image_id, rows[0].id);
    assert!((hits[1].score - 1.0 / 11.0_f32.sqrt()).abs() < 1e-6);
    let response = schema.execute(r#"{images(query:"text:cat",offset:0,limit:2,sortBy:NAME_ASC){id} imageCount(query:"text:cat")}"#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap(),
        json!({
            "images": [{"id": rows[1].id}, {"id": rows[0].id}],
            "imageCount": 2
        })
    );
}

async fn import_and_index(schema: &PublicSchema) {
    let index = schema.data::<Arc<ImageIndex>>().unwrap();
    let models = schema.data::<Arc<Runtime>>().unwrap();
    let import = schema.execute("mutation { importImageSearchModel }").await;
    assert!(import.errors.is_empty(), "{:?}", import.errors);
    assert_eq!(
        import.data.into_json().unwrap()["importImageSearchModel"],
        true
    );
    tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            let status = models.snapshot().status;
            assert!(status.error_message.is_empty(), "{}", status.error_message);
            if status.status == ImageSearchStatusType::Ready {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        index.wait_until_idle().await;
    })
    .await
    .unwrap();
    assert!(
        index.status().error_message.is_empty(),
        "{}",
        index.status().error_message
    );
}

#[tokio::test]
#[ignore = "requires the original SigLIP2 files in PLAIN_TEST_SIGLIP2_PACKAGE"]
async fn siglip2_import_searches_an_ort_index_and_adds_a_matching_vector() {
    use plain_rs::features::services::images::default_model;
    let (dir, schema, db) = fixture_seeded(
        GRANTED,
        true,
        |_| {},
        |method, _| panic!("unexpected host call {method}"),
    );
    schema
        .data::<Arc<ImageIndex>>()
        .unwrap()
        .use_native_catalog_for_test();
    let source = std::path::PathBuf::from(std::env::var_os("PLAIN_TEST_SIGLIP2_PACKAGE").unwrap());
    let incoming = dir.path().join("ai_models/incoming");
    std::fs::create_dir_all(&incoming).unwrap();
    let manifest = default_model::manifest();
    for file in &manifest.files {
        std::fs::copy(source.join(&file.name), incoming.join(&file.name)).unwrap();
    }
    std::fs::write(
        incoming.join("manifest.json"),
        plain_libs::serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();

    let archived = dir.path().join("archived.png");
    let added = dir.path().join("added.png");
    plain_libs::image::RgbImage::from_fn(96, 64, |x, y| {
        plain_libs::image::Rgb([
            (x * 255 / 95) as u8,
            (y * 255 / 63) as u8,
            ((x + 2 * y) % 256) as u8,
        ])
    })
    .save(&added)
    .unwrap();
    let rows = db
        .record_media_files(&[
            media(
                DataType::Image,
                archived.to_str().unwrap(),
                "archived",
                200,
                "b1",
            ),
            media(DataType::Image, added.to_str().unwrap(), "added", 200, "b1"),
        ])
        .unwrap();
    let golden = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/image_search/siglip2_gradient.f32be"
    ));
    db.key_values()
        .set_json(
            "image_index_model",
            "4ee8b24283c89e923b3449ce2c1cc3d1dc6a18522bbd6ab9ef74cf38cbea2afb",
        )
        .unwrap();
    embeddings::save(
        &db,
        &[EmbeddingInput {
            id: rows[0].id.clone(),
            path: rows[0].path.clone(),
            embedding_base64: plain_libs::utils::base64::base64_encode(golden),
        }],
    )
    .unwrap();

    import_and_index(&schema).await;
    let vector = |id: &str| {
        db.with_conn(|conn| {
            conn.query_row(
                "SELECT embedding FROM image_embeddings WHERE file_id=?1",
                [id],
                |row| row.get::<_, Vec<u8>>(0),
            )
        })
        .unwrap()
    };
    assert_eq!(vector(&rows[0].id), golden);
    let new_vector = vector(&rows[1].id);
    assert_eq!(new_vector.len(), 768 * 4);
    let cosine: f64 = new_vector
        .chunks_exact(4)
        .zip(golden.chunks_exact(4))
        .map(|(a, b)| {
            f64::from(f32::from_be_bytes(a.try_into().unwrap()))
                * f64::from(f32::from_be_bytes(b.try_into().unwrap()))
        })
        .sum();
    assert!(cosine > 0.999, "image cosine {cosine}");
    let hits = schema
        .data::<Arc<Runtime>>()
        .unwrap()
        .search("a colorful gradient")
        .await
        .unwrap();
    assert_eq!(hits.len(), 2);
    for hit in hits {
        assert!(
            (hit.score - 0.15885683).abs() < 0.003,
            "query score {}",
            hit.score
        );
    }
    let response = schema
        .execute(r#"{imageCount(query:"text:\"a colorful gradient\"")}"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["imageCount"], 2);
}
