use super::*;
pub fn factory<'a>() -> NoUnusedVariables<'a> {
    NoUnusedVariables::default()
}
#[test]
fn uses_all_variables() {
    expect_passes_rule!(
        factory,
        r#"
          query ($a: String, $b: String, $c: String) {
            field(a: $a, b: $b, c: $c)
          }
        "#,
    );
}
#[test]
fn uses_all_variables_deeply() {
    expect_passes_rule!(
        factory,
        r#"
          query Foo($a: String, $b: String, $c: String) {
            field(a: $a) {
              field(b: $b) {
                field(c: $c)
              }
            }
          }
        "#,
    );
}
#[test]
fn uses_all_variables_deeply_in_inline_fragments() {
    expect_passes_rule!(
        factory,
        r#"
          query Foo($a: String, $b: String, $c: String) {
            ... on Type {
              field(a: $a) {
                field(b: $b) {
                  ... on Type {
                    field(c: $c)
                  }
                }
              }
            }
          }
        "#,
    );
}
#[test]
fn uses_all_variables_in_fragments() {
    expect_passes_rule!(
        factory,
        r#"
          query Foo($a: String, $b: String, $c: String) {
            ...FragA
          }
          fragment FragA on Type {
            field(a: $a) {
              ...FragB
            }
          }
          fragment FragB on Type {
            field(b: $b) {
              ...FragC
            }
          }
          fragment FragC on Type {
            field(c: $c)
          }
        "#,
    );
}
#[test]
fn variable_used_by_fragment_in_multiple_operations() {
    expect_passes_rule!(
        factory,
        r#"
          query Foo($a: String) {
            ...FragA
          }
          query Bar($b: String) {
            ...FragB
          }
          fragment FragA on Type {
            field(a: $a)
          }
          fragment FragB on Type {
            field(b: $b)
          }
        "#,
    );
}
#[test]
fn variable_used_by_recursive_fragment() {
    expect_passes_rule!(
        factory,
        r#"
          query Foo($a: String) {
            ...FragA
          }
          fragment FragA on Type {
            field(a: $a) {
              ...FragA
            }
          }
        "#,
    );
}
#[test]
fn variable_used_by_inline_fragment() {
    expect_passes_rule!(
        factory,
        r#"
          query Foo($a: String) {
            ... {
                field(a: $a) {
                  ...FragA
                }
            }
          }
        "#,
    );
}
#[test]
fn variable_not_used() {
    expect_fails_rule!(
        factory,
        r#"
          query ($a: String, $b: String, $c: String) {
            field(a: $a, b: $b)
          }
        "#,
    );
}
#[test]
fn multiple_variables_not_used_1() {
    expect_fails_rule!(
        factory,
        r#"
          query Foo($a: String, $b: String, $c: String) {
            field(b: $b)
          }
        "#,
    );
}
#[test]
fn variable_not_used_in_fragment() {
    expect_fails_rule!(
        factory,
        r#"
          query Foo($a: String, $b: String, $c: String) {
            ...FragA
          }
          fragment FragA on Type {
            field(a: $a) {
              ...FragB
            }
          }
          fragment FragB on Type {
            field(b: $b) {
              ...FragC
            }
          }
          fragment FragC on Type {
            field
          }
        "#,
    );
}
#[test]
fn multiple_variables_not_used_2() {
    expect_fails_rule!(
        factory,
        r#"
          query Foo($a: String, $b: String, $c: String) {
            ...FragA
          }
          fragment FragA on Type {
            field {
              ...FragB
            }
          }
          fragment FragB on Type {
            field(b: $b) {
              ...FragC
            }
          }
          fragment FragC on Type {
            field
          }
        "#,
    );
}
#[test]
fn variable_not_used_by_unreferenced_fragment() {
    expect_fails_rule!(
        factory,
        r#"
          query Foo($b: String) {
            ...FragA
          }
          fragment FragA on Type {
            field(a: $a)
          }
          fragment FragB on Type {
            field(b: $b)
          }
        "#,
    );
}
#[test]
fn variable_not_used_by_fragment_used_by_other_operation() {
    expect_fails_rule!(
        factory,
        r#"
          query Foo($b: String) {
            ...FragA
          }
          query Bar($a: String) {
            ...FragB
          }
          fragment FragA on Type {
            field(a: $a)
          }
          fragment FragB on Type {
            field(b: $b)
          }
        "#,
    );
}
