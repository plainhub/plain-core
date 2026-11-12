use crate::tokenizer::{
    Model,
    models::{bpe, wordlevel::WordLevel, wordpiece::WordPiece},
};
use std::error::Error as _;

#[test]
fn tokenizer_entry_points_keep_missing_vocabulary_errors() {
    assert_eq!(
        WordPiece::default()
            .tokenize("sample")
            .unwrap_err()
            .to_string(),
        "WordPiece error: Missing [UNK] token from the vocabulary"
    );
    assert_eq!(
        WordLevel::default()
            .tokenize("sample")
            .unwrap_err()
            .to_string(),
        "WordLevel error: Missing [UNK] token from the vocabulary"
    );
}

#[test]
fn bpe_conversions_preserve_io_and_json_failure_identity() {
    let error = bpe::Error::from(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "generated denial",
    ));
    assert_eq!(error.to_string(), "IoError: generated denial");
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<std::io::Error>()
            .unwrap()
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
    let malformed = crate::serde_json::from_slice::<crate::serde_json::Value>(b"[").unwrap_err();
    let location = (malformed.line(), malformed.column());
    let error = bpe::Error::from(malformed);
    let source = error
        .source()
        .unwrap()
        .downcast_ref::<crate::serde_json::Error>()
        .unwrap();
    assert_eq!((source.line(), source.column()), location);
}
