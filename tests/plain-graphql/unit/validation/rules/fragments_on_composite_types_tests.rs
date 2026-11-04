use super::*;
fn factory() -> FragmentsOnCompositeTypes {
    FragmentsOnCompositeTypes
}
#[test]
fn on_object() {
    expect_passes_rule!(
        factory,
        r#"
          fragment validFragment on Dog {
            barks
          }
          { __typename }
        "#,
    );
}
#[test]
fn on_interface() {
    expect_passes_rule!(
        factory,
        r#"
          fragment validFragment on Pet {
            name
          }
          { __typename }
        "#,
    );
}
#[test]
fn on_object_inline() {
    expect_passes_rule!(
        factory,
        r#"
          fragment validFragment on Pet {
            ... on Dog {
              barks
            }
          }
          { __typename }
        "#,
    );
}
#[test]
fn on_inline_without_type_cond() {
    expect_passes_rule!(
        factory,
        r#"
          fragment validFragment on Pet {
            ... {
              name
            }
          }
          { __typename }
        "#,
    );
}
#[test]
fn on_union() {
    expect_passes_rule!(
        factory,
        r#"
          fragment validFragment on CatOrDog {
            __typename
          }
          { __typename }
        "#,
    );
}
#[test]
fn not_on_scalar() {
    expect_fails_rule!(
        factory,
        r#"
          fragment scalarFragment on Boolean {
            bad
          }
          { __typename }
        "#,
    );
}
#[test]
fn not_on_enum() {
    expect_fails_rule!(
        factory,
        r#"
          fragment scalarFragment on FurColor {
            bad
          }
          { __typename }
        "#,
    );
}
#[test]
fn not_on_input_object() {
    expect_fails_rule!(
        factory,
        r#"
          fragment inputFragment on ComplexInput {
            stringField
          }
          { __typename }
        "#,
    );
}
#[test]
fn not_on_scalar_inline() {
    expect_fails_rule!(
        factory,
        r#"
          fragment invalidFragment on Pet {
            ... on String {
              barks
            }
          }
          { __typename }
        "#,
    );
}
