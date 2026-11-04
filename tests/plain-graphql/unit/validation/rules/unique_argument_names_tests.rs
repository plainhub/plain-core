use super::*;
pub fn factory<'a>() -> UniqueArgumentNames<'a> {
    UniqueArgumentNames::default()
}
#[test]
fn no_arguments_on_field() {
    expect_passes_rule!(
        factory,
        r#"
          {
            field
          }
        "#,
    );
}
#[test]
fn no_arguments_on_directive() {
    expect_passes_rule!(
        factory,
        r#"
          {
            dog @directive
          }
        "#,
    );
}
#[test]
fn argument_on_field() {
    expect_passes_rule!(
        factory,
        r#"
          {
            field(arg: "value")
          }
        "#,
    );
}
#[test]
fn argument_on_directive() {
    expect_passes_rule!(
        factory,
        r#"
          {
            dog @directive(arg: "value")
          }
        "#,
    );
}
#[test]
fn same_argument_on_two_fields() {
    expect_passes_rule!(
        factory,
        r#"
          {
            one: field(arg: "value")
            two: field(arg: "value")
          }
        "#,
    );
}
#[test]
fn same_argument_on_field_and_directive() {
    expect_passes_rule!(
        factory,
        r#"
          {
            field(arg: "value") @directive(arg: "value")
          }
        "#,
    );
}
#[test]
fn same_argument_on_two_directives() {
    expect_passes_rule!(
        factory,
        r#"
          {
            field @directive1(arg: "value") @directive2(arg: "value")
          }
        "#,
    );
}
#[test]
fn multiple_field_arguments() {
    expect_passes_rule!(
        factory,
        r#"
          {
            field(arg1: "value", arg2: "value", arg3: "value")
          }
        "#,
    );
}
#[test]
fn multiple_directive_arguments() {
    expect_passes_rule!(
        factory,
        r#"
          {
            field @directive(arg1: "value", arg2: "value", arg3: "value")
          }
        "#,
    );
}
#[test]
fn duplicate_field_arguments() {
    expect_fails_rule!(
        factory,
        r#"
          {
            field(arg1: "value", arg1: "value")
          }
        "#,
    );
}
#[test]
fn many_duplicate_field_arguments() {
    expect_fails_rule!(
        factory,
        r#"
          {
            field(arg1: "value", arg1: "value", arg1: "value")
          }
        "#,
    );
}
#[test]
fn duplicate_directive_arguments() {
    expect_fails_rule!(
        factory,
        r#"
          {
            field @directive(arg1: "value", arg1: "value")
          }
        "#,
    );
}
#[test]
fn many_duplicate_directive_arguments() {
    expect_fails_rule!(
        factory,
        r#"
          {
            field @directive(arg1: "value", arg1: "value", arg1: "value")
          }
        "#,
    );
}
