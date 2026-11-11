use super::*;
use crate::tokenizer::{OffsetReferential, OffsetType, PreTokenizer};
use SplitDelimiterBehavior::*;

#[test]
fn basic() {
    let tests = vec![
        (
            Removed,
            "How are you doing?",
            vec![
                ("How", (0, 3)),
                ("are", (4, 7)),
                ("you", (8, 11)),
                ("doing", (12, 17)),
                ("?", (17, 18)),
            ],
        ),
        (
            Isolated,
            "How are you doing?",
            vec![
                ("How", (0, 3)),
                (" ", (3, 4)),
                ("are", (4, 7)),
                (" ", (7, 8)),
                ("you", (8, 11)),
                (" ", (11, 12)),
                ("doing", (12, 17)),
                ("?", (17, 18)),
            ],
        ),
        (
            MergedWithPrevious,
            "How are you doing?",
            vec![
                ("How ", (0, 4)),
                ("are ", (4, 8)),
                ("you ", (8, 12)),
                ("doing", (12, 17)),
                ("?", (17, 18)),
            ],
        ),
        (
            MergedWithNext,
            "How are you doing?",
            vec![
                ("How", (0, 3)),
                (" are", (3, 7)),
                (" you", (7, 11)),
                (" doing", (11, 17)),
                ("?", (17, 18)),
            ],
        ),
        (
            Contiguous,
            "How are you doing?",
            vec![
                ("How", (0, 3)),
                (" ", (3, 4)),
                ("are", (4, 7)),
                (" ", (7, 8)),
                ("you", (8, 11)),
                (" ", (11, 12)),
                ("doing?", (12, 18)),
            ],
        ),
    ];

    // use whitespace regex
    let regex = SplitPattern::Regex(r"\w+|[^\w\s]+".into());

    for (behavior, s, res) in tests {
        let mut pretokenized = PreTokenizedString::from(s);
        let pretok = Split::new(regex.clone(), behavior, true).unwrap();
        pretok.pre_tokenize(&mut pretokenized).unwrap();
        assert_eq!(
            pretokenized
                .get_splits(OffsetReferential::Original, OffsetType::Byte)
                .into_iter()
                .map(|(s, o, _)| (s, o))
                .collect::<Vec<_>>(),
            res
        );
    }
}

#[test]
fn regex_string() {
    let mut pretok_str_for_regex = PreTokenizedString::from("Hey, man!");
    let mut pretok_str_for_string = pretok_str_for_regex.clone();

    // pre-tokenizer splits on " " - one from Regex, one from string
    let pretokenizer_regex = Split::new(
        SplitPattern::Regex(r"\s+".into()),
        SplitDelimiterBehavior::Removed,
        false,
    )
    .unwrap();
    let pretokenizer_string = Split::new(" ", SplitDelimiterBehavior::Removed, false).unwrap();

    pretokenizer_regex
        .pre_tokenize(&mut pretok_str_for_regex)
        .unwrap();
    pretokenizer_string
        .pre_tokenize(&mut pretok_str_for_string)
        .unwrap();

    assert_eq!(pretok_str_for_regex, pretok_str_for_string);
}

#[test]
fn invert() {
    let mut pretok_str = PreTokenizedString::from("Hello Hello Hello");
    let mut pretok_str_for_invert = pretok_str.clone();

    // one pre-tokenizer splits on " " - one splits inverted on "Hello"
    let pretokenizer = Split::new(" ", SplitDelimiterBehavior::Removed, false).unwrap();
    let pretokenizer_invert = Split::new("Hello", SplitDelimiterBehavior::Removed, true).unwrap();

    pretokenizer.pre_tokenize(&mut pretok_str).unwrap();
    pretokenizer_invert
        .pre_tokenize(&mut pretok_str_for_invert)
        .unwrap();

    assert_eq!(pretok_str, pretok_str_for_invert);
}

#[test]
fn serialization() {
    use SplitDelimiterBehavior::*;

    let split = Split::new("Hello", Removed, true).unwrap();
    let split_s =
        r#"{"type":"Split","pattern":{"String":"Hello"},"behavior":"Removed","invert":true}"#;
    assert_eq!(crate::serde_json::to_string(&split).unwrap(), split_s);
    assert_eq!(crate::serde_json::from_str::<Split>(split_s).unwrap(), split);

    let split = Split::new(SplitPattern::Regex(r"\s+".into()), Isolated, false).unwrap();
    let split_s =
        r#"{"type":"Split","pattern":{"Regex":"\\s+"},"behavior":"Isolated","invert":false}"#;
    assert_eq!(crate::serde_json::to_string(&split).unwrap(), split_s);
    assert_eq!(crate::serde_json::from_str::<Split>(split_s).unwrap(), split);
}
