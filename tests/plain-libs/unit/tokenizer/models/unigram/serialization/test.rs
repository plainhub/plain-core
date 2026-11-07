use super::*;

#[test]
fn test_serialization() {
    let vocab = vec![("<unk>".to_string(), 0.0), ("a".to_string(), -0.5)];
    let model = Unigram::from(vocab, Some(0), false).unwrap();

    let data = serde_json::to_string(&model).unwrap();
    let reconstructed = serde_json::from_str(&data).unwrap();

    assert_eq!(model, reconstructed);
}

#[test]
fn test_serialization_unk_id_not_zero() {
    let vocab = vec![("a".to_string(), -0.5), ("<unk>".to_string(), 0.0)];
    let model = Unigram::from(vocab, Some(1), false).unwrap();

    let data = serde_json::to_string(&model).unwrap();
    let reconstructed = serde_json::from_str(&data).unwrap();

    assert_eq!(model, reconstructed);
}

#[test]
fn test_serialization_no_unk_id() {
    let vocab = vec![("a".to_string(), -0.5)];
    let model = Unigram::from(vocab, None, false).unwrap();

    let data = serde_json::to_string(&model).unwrap();
    let reconstructed = serde_json::from_str(&data).unwrap();

    assert_eq!(model, reconstructed);
}
