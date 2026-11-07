use super::*;

#[test]
fn serde() {
    let wp = WordPiece::default();
    let wp_s = "{\
            \"type\":\"WordPiece\",\
            \"unk_token\":\"[UNK]\",\
            \"continuing_subword_prefix\":\"##\",\
            \"max_input_chars_per_word\":100,\
            \"vocab\":{}\
        }";

    assert_eq!(serde_json::to_string(&wp).unwrap(), wp_s);
    assert_eq!(serde_json::from_str::<WordPiece>(wp_s).unwrap(), wp);
}

#[test]
fn deserialization_should_fail() {
    let missing_unk = "{\
            \"type\":\"WordPiece\",\
            \"continuing_subword_prefix\":\"##\",\
            \"max_input_chars_per_word\":100,\
            \"vocab\":{}\
        }";
    assert!(
        serde_json::from_str::<WordPiece>(missing_unk)
            .unwrap_err()
            .to_string()
            .starts_with("missing field `unk_token`")
    );

    let wrong_type = "{\
            \"type\":\"WordLevel\",\
            \"unk_token\":\"[UNK]\",\
            \"vocab\":{}\
        }";
    assert!(
        serde_json::from_str::<WordPiece>(wrong_type)
            .unwrap_err()
            .to_string()
            .starts_with("invalid value: string \"WordLevel\", expected WordPiece")
    );
}
