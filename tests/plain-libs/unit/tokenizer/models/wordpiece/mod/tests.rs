use super::*;

#[test]
fn test_error_display() {
    assert!(format!("{}", Error::MissingUnkToken).contains("Missing [UNK] token"));
}
