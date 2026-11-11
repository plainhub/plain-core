use super::*;
#[path = "../../library/fixtures.rs"]
mod fixtures;
use fixtures::test_db;
fn encode(values: &[f32]) -> String {
    plain_libs::utils::base64::base64_encode(
        &values
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect::<Vec<_>>(),
    )
}
fn item(id: &str, values: &[f32]) -> EmbeddingInput {
    EmbeddingInput {
        id: id.into(),
        path: format!("/synthetic/{id}"),
        embedding_base64: encode(values),
    }
}
#[test]
fn ranked_search_is_bounded_and_persisted() {
    let db = test_db("embeddings_search");
    save(
        &db,
        &[
            item("c", &[0.1, 0.0]),
            item("b", &[0.9, 0.0]),
            item("a", &[0.9, 0.0]),
            item("best", &[1.0, 0.0]),
        ],
    )
    .unwrap();
    let results = search(&db, &encode(&[1.0, 0.0]), 2).unwrap();
    assert_eq!(
        results
            .iter()
            .map(|r| r.image_id.as_str())
            .collect::<Vec<_>>(),
        ["best", "a"]
    );
    assert_eq!(results[0].score, 1.0);
    assert_eq!(count(&db).unwrap(), 4);
    assert_eq!(delete(&db, &[]).unwrap(), 0);
    assert_eq!(delete(&db, &["best".into()]).unwrap(), 1);
    assert_eq!(search(&db, &encode(&[1.0, 0.0]), 10).unwrap().len(), 2);
    assert_eq!(clear(&db).unwrap(), 3);
}
#[test]
fn invalid_vectors_and_failed_batch_do_not_commit_partial_updates() {
    let db = test_db("embeddings_validation");
    save(&db, &[item("old", &[1.0, 0.0])]).unwrap();
    assert!(
        save(
            &db,
            &[item("new", &[1.0, 0.0]), item("bad", &[f32::NAN, 0.0])]
        )
        .is_err()
    );
    assert_eq!(ids(&db).unwrap(), ["old"]);
    db.with_conn(|c|c.execute_batch("CREATE TRIGGER fail_embedding BEFORE INSERT ON image_embeddings WHEN NEW.file_id='bad' BEGIN SELECT RAISE(ABORT,'fail'); END")).unwrap();
    assert!(save(&db, &[item("new", &[1.0, 0.0]), item("bad", &[1.0, 0.0])]).is_err());
    assert_eq!(ids(&db).unwrap(), ["old"]);
    assert!(search(&db, &encode(&[1.0]), 5).is_err());
    assert!(search(&db, &encode(&[f32::INFINITY, 0.0]), 5).is_err());
    assert!(search(&db, "invalid", 5).is_err());
    assert!(search(&db, &encode(&[1.0, 0.0]), 501).is_err());
}

#[test]
fn scored_search_returns_all_matches_in_stable_order() {
    let db = test_db("embeddings_all_matches");
    for batch in (0..620).collect::<Vec<_>>().chunks(100) {
        let items = batch
            .iter()
            .map(|i| item(&format!("image-{i:04}"), &[0.75, 0.0]))
            .collect::<Vec<_>>();
        save(&db, &items).unwrap();
    }
    let results = search_with_score(&db, &encode(&[1.0, 0.0]), 0.75).unwrap();
    assert_eq!(results.len(), 620);
    assert_eq!(
        results
            .iter()
            .map(|r| r.image_id.clone())
            .collect::<Vec<_>>(),
        (0..620)
            .map(|i| format!("image-{i:04}"))
            .collect::<Vec<_>>()
    );
    assert!(
        search_with_score(&db, &encode(&[1.0, 0.0]), 0.76)
            .unwrap()
            .is_empty()
    );
    for minimum in [f32::NAN, f32::INFINITY, -1.1, 1.1] {
        assert!(search_with_score(&db, &encode(&[1.0, 0.0]), minimum).is_err());
    }
}
