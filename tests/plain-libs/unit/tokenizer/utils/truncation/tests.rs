use super::*;
use crate::tokenizer::tokenizer::Encoding;
use std::collections::HashMap;

fn get_empty() -> Encoding {
    Encoding::new(
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        HashMap::new(),
    )
}

fn get_short() -> Encoding {
    Encoding::new(
        vec![1, 2],
        vec![0, 0],
        vec![String::from("a"), String::from("b")],
        vec![Some(0), Some(1)],
        vec![(0, 1), (1, 2)],
        vec![0, 0],
        vec![1, 1],
        vec![],
        HashMap::new(),
    )
}

fn get_medium() -> Encoding {
    Encoding::new(
        vec![3, 4, 5, 6],
        vec![0, 0, 0, 0],
        vec![
            String::from("d"),
            String::from("e"),
            String::from("f"),
            String::from("g"),
        ],
        vec![Some(0), Some(1), Some(2), Some(3)],
        vec![(0, 1), (1, 2), (2, 3), (3, 4)],
        vec![0, 0, 0, 0],
        vec![1, 1, 1, 1],
        vec![],
        HashMap::new(),
    )
}

fn get_long() -> Encoding {
    Encoding::new(
        vec![7, 8, 9, 10, 11, 12, 13, 14],
        vec![0, 0, 0, 0, 0, 0, 0, 0],
        vec![
            String::from("h"),
            String::from("i"),
            String::from("j"),
            String::from("k"),
            String::from("l"),
            String::from("m"),
            String::from("n"),
            String::from("o"),
        ],
        vec![
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(7),
        ],
        vec![
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 6),
            (6, 7),
            (6, 8),
        ],
        vec![0, 0, 0, 0, 0, 0, 0, 0],
        vec![1, 1, 1, 1, 1, 1, 1, 1],
        vec![],
        HashMap::new(),
    )
}

fn truncate_and_assert(
    encoding1: Encoding,
    encoding2: Encoding,
    params: &TruncationParams,
    n1: usize,
    n2: usize,
) {
    match truncate_encodings(encoding1, Some(encoding2), params) {
        Ok((e1, Some(e2))) => {
            assert!(e1.get_ids().len() == n1);
            assert!(e2.get_ids().len() == n2);
        }
        _ => panic!(),
    };
}

#[test]
fn truncate_encodings_longest_first() {
    let params = TruncationParams {
        max_length: 7,
        strategy: TruncationStrategy::LongestFirst,
        stride: 0,
        direction: TruncationDirection::Right,
    };

    truncate_and_assert(get_empty(), get_empty(), &params, 0, 0);
    truncate_and_assert(get_empty(), get_short(), &params, 0, 2);
    truncate_and_assert(get_empty(), get_medium(), &params, 0, 4);
    truncate_and_assert(get_empty(), get_long(), &params, 0, 7);

    truncate_and_assert(get_short(), get_empty(), &params, 2, 0);
    truncate_and_assert(get_short(), get_short(), &params, 2, 2);
    truncate_and_assert(get_short(), get_medium(), &params, 2, 4);
    truncate_and_assert(get_short(), get_long(), &params, 2, 5);

    truncate_and_assert(get_medium(), get_empty(), &params, 4, 0);
    truncate_and_assert(get_medium(), get_short(), &params, 4, 2);
    truncate_and_assert(get_medium(), get_medium(), &params, 3, 4);
    truncate_and_assert(get_medium(), get_long(), &params, 3, 4);

    truncate_and_assert(get_long(), get_empty(), &params, 7, 0);
    truncate_and_assert(get_long(), get_short(), &params, 5, 2);
    truncate_and_assert(get_long(), get_medium(), &params, 4, 3);
    truncate_and_assert(get_long(), get_long(), &params, 3, 4);
}

#[test]
fn truncate_encodings_empty() {
    let params = TruncationParams {
        max_length: 0,
        strategy: TruncationStrategy::LongestFirst,
        stride: 0,
        direction: TruncationDirection::Right,
    };

    truncate_and_assert(get_empty(), get_short(), &params, 0, 0);
    truncate_and_assert(get_medium(), get_medium(), &params, 0, 0);
    truncate_and_assert(get_long(), get_long(), &params, 0, 0);
}

#[test]
fn test_deserialize_defaults() {
    let old_truncation_params = r#"{"max_length":256,"strategy":"LongestFirst","stride":0}"#;

    let params: TruncationParams = serde_json::from_str(old_truncation_params).unwrap();

    assert_eq!(params.direction, TruncationDirection::Right);
}
