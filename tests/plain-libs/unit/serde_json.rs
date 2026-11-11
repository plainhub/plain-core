use super::*;
use serde::Deserialize;
#[derive(Deserialize)]
struct Borrowed<'a> {
    text: &'a str,
    raw: &'a value::RawValue,
}
#[test]
fn slice_strings_and_raw_values_borrow_original_storage() {
    let input = br#"{"text":"hello","raw":{"n":123}}"#;
    let value: Borrowed<'_> = from_slice(input).unwrap();
    assert_eq!(value.text, "hello");
    assert_eq!(value.raw.get(), r#"{"n":123}"#);
    let range = input.as_ptr_range();
    assert!(range.contains(&value.text.as_ptr()));
    assert!(range.contains(&value.raw.get().as_ptr()));
}
#[test]
fn json_validation_and_exact_float_and_integer_bytes_are_preserved() {
    for bad in [
        "[1,]",
        "{\"a\":1} trailing",
        "\"\\uD800\"",
        "1e9999",
        "01",
        "NaN",
    ] {
        assert!(from_str::<Value>(bad).is_err(), "{bad}");
    }
    assert_eq!(
        to_string(&json!({"min":i64::MIN,"max":u64::MAX,"zero":-0.0})).unwrap(),
        r#"{"max":18446744073709551615,"min":-9223372036854775808,"zero":-0.0}"#
    );
    let error = from_str::<Value>("{\n\"a\":\n}").unwrap_err();
    assert_eq!(error.line(), 3);
}

#[test]
fn deeply_nested_input_retains_the_recursion_limit() {
    let input = "[".repeat(129) + "0" + &"]".repeat(129);
    let error = from_str::<Value>(&input).unwrap_err();
    assert!(error.to_string().contains("recursion limit exceeded"));
}
