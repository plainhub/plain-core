use super::*;
use crate::tokenizer::tokenizer::Encoding;
use ahash::AHashMap;

#[test]
fn pad_to_multiple() {
    fn get_encodings() -> [Encoding; 2] {
        [
            Encoding::new(
                vec![0, 1, 2, 3, 4],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                AHashMap::new(),
            ),
            Encoding::new(
                vec![0, 1, 2],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                AHashMap::new(),
            ),
        ]
    }

    // Test fixed
    let mut encodings = get_encodings();
    let mut params = PaddingParams {
        strategy: PaddingStrategy::Fixed(7),
        direction: PaddingDirection::Right,
        pad_to_multiple_of: Some(8),
        pad_id: 0,
        pad_type_id: 0,
        pad_token: String::from("[PAD]"),
    };
    pad_encodings(&mut encodings, &params).unwrap();
    assert!(encodings.iter().all(|e| e.get_ids().len() == 8));

    // Test batch
    let mut encodings = get_encodings();
    params.strategy = PaddingStrategy::BatchLongest;
    params.pad_to_multiple_of = Some(6);
    pad_encodings(&mut encodings, &params).unwrap();
    assert!(encodings.iter().all(|e| e.get_ids().len() == 6));

    // Do not crash with 0
    params.pad_to_multiple_of = Some(0);
    pad_encodings(&mut encodings, &params).unwrap();
}
