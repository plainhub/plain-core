use super::*;
pub fn factory() -> FieldsOnCorrectType {
    FieldsOnCorrectType
}
#[test]
fn selection_on_object() {
    expect_passes_rule!(
        factory,
        r#"
          fragment objectFieldSelection on Dog {
            __typename
            name
          }
          { __typename }
        "#,
    );
}
#[test]
fn aliased_selection_on_object() {
    expect_passes_rule!(
        factory,
        r#"
          fragment aliasedObjectFieldSelection on Dog {
            tn : __typename
            otherName : name
          }
          { __typename }
        "#,
    );
}
#[test]
fn selection_on_interface() {
    expect_passes_rule!(
        factory,
        r#"
          fragment interfaceFieldSelection on Pet {
            __typename
            name
          }
          { __typename }
        "#,
    );
}
#[test]
fn aliased_selection_on_interface() {
    expect_passes_rule!(
        factory,
        r#"
          fragment interfaceFieldSelection on Pet {
            otherName : name
          }
          { __typename }
        "#,
    );
}
#[test]
fn lying_alias_selection() {
    expect_passes_rule!(
        factory,
        r#"
          fragment lyingAliasSelection on Dog {
            name : nickname
          }
          { __typename }
        "#,
    );
}
#[test]
fn ignores_unknown_type() {
    expect_passes_rule!(
        factory,
        r#"
          fragment unknownSelection on UnknownType {
            unknownField
          }
          { __typename }
        "#,
    );
}
#[test]
fn nested_unknown_fields() {
    expect_fails_rule!(
        factory,
        r#"
          fragment typeKnownAgain on Pet {
            unknown_pet_field {
              ... on Cat {
                unknown_cat_field
              }
            }
          }
          { __typename }
        "#,
    );
}
#[test]
fn unknown_field_on_fragment() {
    expect_fails_rule!(
        factory,
        r#"
          fragment fieldNotDefined on Dog {
            meowVolume
          }
          { __typename }
        "#,
    );
}
#[test]
fn ignores_deeply_unknown_field() {
    expect_fails_rule!(
        factory,
        r#"
          fragment deepFieldNotDefined on Dog {
            unknown_field {
              deeper_unknown_field
            }
          }
          { __typename }
        "#,
    );
}
#[test]
fn unknown_subfield() {
    expect_fails_rule!(
        factory,
        r#"
          fragment subFieldNotDefined on Human {
            pets {
              unknown_field
            }
          }
          { __typename }
        "#,
    );
}
#[test]
fn unknown_field_on_inline_fragment() {
    expect_fails_rule!(
        factory,
        r#"
          fragment fieldNotDefined on Pet {
            ... on Dog {
              meowVolume
            }
          }
          { __typename }
        "#,
    );
}
#[test]
fn unknown_aliased_target() {
    expect_fails_rule!(
        factory,
        r#"
          fragment aliasedFieldTargetNotDefined on Dog {
            volume : mooVolume
          }
          { __typename }
        "#,
    );
}
#[test]
fn unknown_aliased_lying_field_target() {
    expect_fails_rule!(
        factory,
        r#"
          fragment aliasedLyingFieldTargetNotDefined on Dog {
            barkVolume : kawVolume
          }
          { __typename }
        "#,
    );
}
#[test]
fn not_defined_on_interface() {
    expect_fails_rule!(
        factory,
        r#"
          fragment notDefinedOnInterface on Pet {
            tailLength
          }
          { __typename }
        "#,
    );
}
#[test]
fn defined_in_concrete_types_but_not_interface() {
    expect_fails_rule!(
        factory,
        r#"
          fragment definedOnImplementorsButNotInterface on Pet {
            nickname
          }
          { __typename }
        "#,
    );
}
#[test]
fn meta_field_on_union() {
    expect_passes_rule!(
        factory,
        r#"
          fragment definedOnImplementorsButNotInterface on Pet {
            __typename
          }
          { __typename }
        "#,
    );
}
#[test]
fn fields_on_union() {
    expect_fails_rule!(
        factory,
        r#"
          fragment definedOnImplementorsQueriedOnUnion on CatOrDog {
            name
          }
          { __typename }
        "#,
    );
}
#[test]
fn typename_on_union() {
    expect_passes_rule!(
        factory,
        r#"
          fragment objectFieldSelection on Pet {
            __typename
            ... on Dog {
              name
            }
            ... on Cat {
              name
            }
          }
          { __typename }
        "#,
    );
}
#[test]
fn valid_field_in_inline_fragment() {
    expect_passes_rule!(
        factory,
        r#"
          fragment objectFieldSelection on Pet {
            ... on Dog {
              name
            }
            ... {
              name
            }
          }
          { __typename }
        "#,
    );
}
#[test]
fn typename_in_subscription_root() {
    expect_fails_rule!(factory, "subscription { __typename }");
}
