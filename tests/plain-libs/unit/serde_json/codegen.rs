use crate::codegen_test::{compile_probe, external_calls};

#[test]
#[ignore = "Requires PLAIN_LIBS_CODEGEN_RLIB and PLAIN_LIBS_CODEGEN_DEPS from a production library build"]
fn numeric_constructors_inline_across_the_library_boundary() {
    let ir = compile_probe(
        r#"
extern crate plain_libs;
use plain_libs::serde_json::{Number, Value};
#[unsafe(no_mangle)]
pub fn construct_unsigned(value: u64) -> Number { value.into() }
#[unsafe(no_mangle)]
pub fn construct_signed(value: i64) -> Number { value.into() }
#[unsafe(no_mangle)]
pub fn construct_float(value: f64) -> Option<Number> { Number::from_f64(value) }
#[unsafe(no_mangle)]
pub fn duration_seconds(value: &plain_libs::chrono::TimeDelta) -> i64 { value.num_seconds() }
#[unsafe(no_mangle)]
pub fn uuid_bytes(value: &plain_libs::utils::uuid::Uuid) -> &[u8; 16] { value.as_bytes() }
#[unsafe(no_mangle)]
pub fn method_safe(value: &plain_libs::http::Method) -> bool { value.is_safe() }
#[unsafe(no_mangle)]
pub fn parse_array(input: &[u8]) -> Value { plain_libs::serde_json::from_slice(input).unwrap() }
"#,
    );
    let suffix_calls: Vec<_> = ir
        .lines()
        .filter(|line| {
            !line.trim_start().starts_with(';')
                && (line.contains("call ") || line.contains("invoke "))
                && line.contains("serde_json")
                && (line.contains("parse_number")
                    || line.contains("parse_integer")
                    || line.contains("append_decimal_digit"))
        })
        .collect();
    assert!(
        suffix_calls.is_empty(),
        "numeric suffix dispatch must inline: {suffix_calls:?}"
    );

    for function in [
        "construct_unsigned",
        "construct_signed",
        "construct_float",
        "duration_seconds",
        "uuid_bytes",
        "method_safe",
    ] {
        let calls = external_calls(&ir, function);
        assert!(
            calls.is_empty(),
            "{function} must be inlined, got {calls:?}"
        );
    }
}
