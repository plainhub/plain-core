use super::*;
pub fn factory() -> VariablesAreInputTypes {
    VariablesAreInputTypes
}
#[test]
fn input_types_are_valid() {
    expect_passes_rule!(
        factory,
        r#"
          query Foo($a: String, $b: [Boolean!]!, $c: ComplexInput) {
            field(a: $a, b: $b, c: $c)
          }
        "#,
    );
}
#[test]
fn output_types_are_invalid() {
    expect_fails_rule!(
        factory,
        r#"
          query Foo($a: Dog, $b: [[CatOrDog!]]!, $c: Pet) {
            field(a: $a, b: $b, c: $c)
          }
        "#,
    );
}
