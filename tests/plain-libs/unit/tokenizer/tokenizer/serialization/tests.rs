use crate::tokenizer::tokenizer::Tokenizer;
use std::str::FromStr;

#[test]
fn test_deserialization_serialization_invariant() {
    let tok_json = r#"{
  "version": "1.0",
  "truncation": null,
  "padding": null,
  "added_tokens": [
    {
      "id": 0,
      "content": "[SPECIAL_0]",
      "single_word": false,
      "lstrip": false,
      "rstrip": false,
      "normalized": false,
      "special": true
    },
    {
      "id": 1,
      "content": "[SPECIAL_1]",
      "single_word": false,
      "lstrip": false,
      "rstrip": false,
      "normalized": true,
      "special": false
    },
    {
      "id": 2,
      "content": "[SPECIAL_2]",
      "single_word": false,
      "lstrip": false,
      "rstrip": false,
      "normalized": false,
      "special": true
    }
  ],
  "normalizer": null,
  "pre_tokenizer": null,
  "post_processor": null,
  "decoder": null,
  "model": {
    "type": "WordPiece",
    "unk_token": "[UNK]",
    "continuing_subword_prefix": "",
    "max_input_chars_per_word": 100,
    "vocab": {}
  }
}"#;
    let tokenizer = Tokenizer::from_str(tok_json).unwrap();

    let tok_str = serde_json::to_string_pretty(&tokenizer).unwrap();
    // It should be exactly the same as above
    assert_eq!(tok_str, tok_json);
}
