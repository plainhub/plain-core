use super::*;
pub fn factory<'a>() -> VariableInAllowedPosition<'a> {
    VariableInAllowedPosition::default()
}
#[test]
fn boolean_into_boolean() {
    expect_passes_rule!(
        factory,
        r#"
          query Query($booleanArg: Boolean)
          {
            complicatedArgs {
              booleanArgField(booleanArg: $booleanArg)
            }
          }
        "#,
    );
}
#[test]
fn boolean_into_boolean_within_fragment() {
    expect_passes_rule!(
        factory,
        r#"
          fragment booleanArgFrag on ComplicatedArgs {
            booleanArgField(booleanArg: $booleanArg)
          }
          query Query($booleanArg: Boolean)
          {
            complicatedArgs {
              ...booleanArgFrag
            }
          }
        "#,
    );
    expect_passes_rule!(
        factory,
        r#"
          query Query($booleanArg: Boolean)
          {
            complicatedArgs {
              ...booleanArgFrag
            }
          }
          fragment booleanArgFrag on ComplicatedArgs {
            booleanArgField(booleanArg: $booleanArg)
          }
        "#,
    );
}
#[test]
fn non_null_boolean_into_boolean() {
    expect_passes_rule!(
        factory,
        r#"
          query Query($nonNullBooleanArg: Boolean!)
          {
            complicatedArgs {
              booleanArgField(booleanArg: $nonNullBooleanArg)
            }
          }
        "#,
    );
}
#[test]
fn non_null_boolean_into_boolean_within_fragment() {
    expect_passes_rule!(
        factory,
        r#"
          fragment booleanArgFrag on ComplicatedArgs {
            booleanArgField(booleanArg: $nonNullBooleanArg)
          }
          query Query($nonNullBooleanArg: Boolean!)
          {
            complicatedArgs {
              ...booleanArgFrag
            }
          }
        "#,
    );
}
#[test]
fn int_into_non_null_int_with_default() {
    expect_passes_rule!(
        factory,
        r#"
          query Query($intArg: Int = 1)
          {
            complicatedArgs {
              nonNullIntArgField(nonNullIntArg: $intArg)
            }
          }
        "#,
    );
}
#[test]
fn string_list_into_string_list() {
    expect_passes_rule!(
        factory,
        r#"
          query Query($stringListVar: [String])
          {
            complicatedArgs {
              stringListArgField(stringListArg: $stringListVar)
            }
          }
        "#,
    );
}
#[test]
fn non_null_string_list_into_string_list() {
    expect_passes_rule!(
        factory,
        r#"
          query Query($stringListVar: [String!])
          {
            complicatedArgs {
              stringListArgField(stringListArg: $stringListVar)
            }
          }
        "#,
    );
}
#[test]
fn string_into_string_list_in_item_position() {
    expect_passes_rule!(
        factory,
        r#"
          query Query($stringVar: String)
          {
            complicatedArgs {
              stringListArgField(stringListArg: [$stringVar])
            }
          }
        "#,
    );
}
#[test]
fn non_null_string_into_string_list_in_item_position() {
    expect_passes_rule!(
        factory,
        r#"
          query Query($stringVar: String!)
          {
            complicatedArgs {
              stringListArgField(stringListArg: [$stringVar])
            }
          }
        "#,
    );
}
#[test]
fn complex_input_into_complex_input() {
    expect_passes_rule!(
        factory,
        r#"
          query Query($complexVar: ComplexInput)
          {
            complicatedArgs {
              complexArgField(complexArg: $complexVar)
            }
          }
        "#,
    );
}
#[test]
fn complex_input_into_complex_input_in_field_position() {
    expect_passes_rule!(
        factory,
        r#"
          query Query($boolVar: Boolean = false)
          {
            complicatedArgs {
              complexArgField(complexArg: {requiredArg: $boolVar})
            }
          }
        "#,
    );
}
#[test]
fn non_null_boolean_into_non_null_boolean_in_directive() {
    expect_passes_rule!(
        factory,
        r#"
          query Query($boolVar: Boolean!)
          {
            dog @include(if: $boolVar)
          }
        "#,
    );
}
#[test]
fn boolean_in_non_null_in_directive_with_default() {
    expect_passes_rule!(
        factory,
        r#"
          query Query($boolVar: Boolean = false)
          {
            dog @include(if: $boolVar)
          }
        "#,
    );
}
#[test]
fn int_into_non_null_int() {
    expect_fails_rule!(
        factory,
        r#"
          query Query($intArg: Int) {
            complicatedArgs {
              nonNullIntArgField(nonNullIntArg: $intArg)
            }
          }
        "#,
    );
}
#[test]
fn int_into_non_null_int_within_fragment() {
    expect_fails_rule!(
        factory,
        r#"
          fragment nonNullIntArgFieldFrag on ComplicatedArgs {
            nonNullIntArgField(nonNullIntArg: $intArg)
          }
          query Query($intArg: Int) {
            complicatedArgs {
              ...nonNullIntArgFieldFrag
            }
          }
        "#,
    );
}
#[test]
fn int_into_non_null_int_within_nested_fragment() {
    expect_fails_rule!(
        factory,
        r#"
          fragment outerFrag on ComplicatedArgs {
            ...nonNullIntArgFieldFrag
          }
          fragment nonNullIntArgFieldFrag on ComplicatedArgs {
            nonNullIntArgField(nonNullIntArg: $intArg)
          }
          query Query($intArg: Int) {
            complicatedArgs {
              ...outerFrag
            }
          }
        "#,
    );
}
#[test]
fn string_over_boolean() {
    expect_fails_rule!(
        factory,
        r#"
          query Query($stringVar: String) {
            complicatedArgs {
              booleanArgField(booleanArg: $stringVar)
            }
          }
        "#,
    );
}
#[test]
fn string_into_string_list() {
    expect_fails_rule!(
        factory,
        r#"
          query Query($stringVar: String) {
            complicatedArgs {
              stringListArgField(stringListArg: $stringVar)
            }
          }
        "#,
    );
}
#[test]
fn boolean_into_non_null_boolean_in_directive() {
    expect_fails_rule!(
        factory,
        r#"
          query Query($boolVar: Boolean) {
            dog @include(if: $boolVar)
          }
        "#,
    );
}
#[test]
fn string_into_non_null_boolean_in_directive() {
    expect_fails_rule!(
        factory,
        r#"
          query Query($stringVar: String) {
            dog @include(if: $stringVar)
          }
        "#,
    );
}
