use super::*;
use crate::tokenizer::processors::{ByteLevel, PostProcessorWrapper};
use crate::tokenizer::tokenizer::{Encoding, PostProcessor};
use std::collections::HashMap;
use std::iter::FromIterator;

#[test]
fn process_chain() {
    let start = Encoding::new(
        vec![0; 5],
        vec![0; 5],
        vec![
            "Ġ".into(),
            "ĠĠĠĠHelloĠĠ".into(),
            "ĠĠHello".into(),
            "HelloĠĠ".into(),
            "ĠĠĠĠ".into(),
        ],
        vec![],
        vec![(0, 1), (0, 11), (11, 18), (18, 25), (25, 29)],
        vec![],
        vec![],
        vec![],
        HashMap::new(),
    );

    let bytelevel = ByteLevel::default().trim_offsets(true);
    let sequence = Sequence::new(vec![PostProcessorWrapper::ByteLevel(bytelevel)]);
    let expected = Encoding::new(
        vec![0; 5],
        vec![0; 5],
        vec![
            "Ġ".into(),
            "ĠĠĠĠHelloĠĠ".into(),
            "ĠĠHello".into(),
            "HelloĠĠ".into(),
            "ĠĠĠĠ".into(),
        ],
        vec![],
        vec![(0, 0), (4, 9), (13, 18), (18, 23), (29, 29)],
        vec![],
        vec![],
        vec![],
        HashMap::from_iter(vec![(0, 0..5)]),
    );

    assert_eq!(
        expected,
        bytelevel.process(start.clone(), None, false).unwrap()
    );
    assert_eq!(
        expected,
        sequence.process(start.clone(), None, false).unwrap()
    );

    let pair_expected = Encoding::new(
        vec![0; 10],
        vec![0, 0, 0, 0, 0, 1, 1, 1, 1, 1],
        vec![
            "Ġ".into(),
            "ĠĠĠĠHelloĠĠ".into(),
            "ĠĠHello".into(),
            "HelloĠĠ".into(),
            "ĠĠĠĠ".into(),
            "Ġ".into(),
            "ĠĠĠĠHelloĠĠ".into(),
            "ĠĠHello".into(),
            "HelloĠĠ".into(),
            "ĠĠĠĠ".into(),
        ],
        vec![],
        vec![
            (0, 0),
            (4, 9),
            (13, 18),
            (18, 23),
            (29, 29),
            (0, 0),
            (4, 9),
            (13, 18),
            (18, 23),
            (29, 29),
        ],
        vec![],
        vec![],
        vec![],
        HashMap::from_iter(vec![(0, 0..5), (1, 5..10)]),
    );
    assert_eq!(
        pair_expected,
        bytelevel
            .process(start.clone(), Some(start.clone()), false)
            .unwrap()
    );
    assert_eq!(
        pair_expected,
        sequence.process(start.clone(), Some(start), false).unwrap()
    );
}
