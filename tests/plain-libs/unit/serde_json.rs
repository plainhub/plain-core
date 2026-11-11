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

#[test]
fn number_scanning_preserves_all_input_modes_and_integer_boundaries() {
    let input = "[0,-0,1,-1,0.25,-0.25,1e2,-1E-2,18446744073709551615,-9223372036854775808,18446744073709551616]";
    let slice: Value = from_slice(input.as_bytes()).unwrap();
    let text: Value = from_str(input).unwrap();
    let reader: Value = from_reader(std::io::Cursor::new(input)).unwrap();
    assert_eq!(slice, text);
    assert_eq!(slice, reader);
    assert_eq!(slice[1].as_f64().unwrap().to_bits(), (-0.0_f64).to_bits());
    assert_eq!(slice[4].as_f64(), Some(0.25));
    assert_eq!(slice[5].as_f64(), Some(-0.25));
    assert_eq!(slice[6].as_f64(), Some(100.0));
    assert_eq!(slice[7].as_f64(), Some(-0.01));
    assert_eq!(slice[8].as_u64(), Some(u64::MAX));
    assert_eq!(slice[9].as_i64(), Some(i64::MIN));
    assert_eq!(slice[10].as_f64(), Some(18446744073709551616.0));
    let values: Vec<Value> = Deserializer::from_slice(b"0 1 22 333 -4 0.25 1e2")
        .into_iter()
        .collect::<Result<_>>()
        .unwrap();
    assert_eq!(
        values,
        vec![
            json!(0),
            json!(1),
            json!(22),
            json!(333),
            json!(-4),
            json!(0.25),
            json!(100.0)
        ]
    );
}

#[test]
fn number_scanning_preserves_errors_and_numeric_object_keys() {
    for input in [
        "01", "-01", "-", "1.", "1.e1", "1e", "1e+", "1e--2", "1e9999", "[1 2]", "[1,]", "1_0",
        "0x1", "1\n0",
    ] {
        let slice = from_slice::<Value>(input.as_bytes()).unwrap_err();
        let text = from_str::<Value>(input).unwrap_err();
        let reader = from_reader::<_, Value>(std::io::Cursor::new(input)).unwrap_err();
        assert_eq!(slice.to_string(), text.to_string(), "{input}");
        assert_eq!(slice.to_string(), reader.to_string(), "{input}");
        assert!(
            matches!(
                slice.classify(),
                error::Category::Syntax | error::Category::Eof
            ),
            "{input}"
        );
    }
    let map: std::collections::BTreeMap<i64, String> =
        from_str(r#"{"1":"one","-2":"two"}"#).unwrap();
    assert_eq!(map[&1], "one");
    assert_eq!(map[&-2], "two");
    for input in [
        r#"{"01":"bad"}"#,
        r#"{"1.5":"bad"}"#,
        r#"{"9223372036854775808":"bad"}"#,
    ] {
        assert!(from_str::<std::collections::BTreeMap<i64, String>>(input).is_err());
    }
}
