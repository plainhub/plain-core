use super::*;
pub fn factory<'a>() -> NoFragmentCycles<'a> {
    NoFragmentCycles::default()
}
#[test]
fn single_reference_is_valid() {
    expect_passes_rule!(
        factory,
        r#"
          fragment fragA on Dog { ...fragB }
          fragment fragB on Dog { name }
          { __typename }
        "#,
    );
}
#[test]
fn spreading_twice_is_not_circular() {
    expect_passes_rule!(
        factory,
        r#"
          fragment fragA on Dog { ...fragB, ...fragB }
          fragment fragB on Dog { name }
          { __typename }
        "#,
    );
}
#[test]
fn spreading_twice_indirectly_is_not_circular() {
    expect_passes_rule!(
        factory,
        r#"
          fragment fragA on Dog { ...fragB, ...fragC }
          fragment fragB on Dog { ...fragC }
          fragment fragC on Dog { name }
          { __typename }
        "#,
    );
}
#[test]
fn double_spread_within_abstract_types() {
    expect_passes_rule!(
        factory,
        r#"
          fragment nameFragment on Pet {
            ... on Dog { name }
            ... on Cat { name }
          }
          fragment spreadsInAnon on Pet {
            ... on Dog { ...nameFragment }
            ... on Cat { ...nameFragment }
          }
          { __typename }
        "#,
    );
}
#[test]
fn does_not_false_positive_on_unknown_fragment() {
    expect_passes_rule!(
        factory,
        r#"
          fragment nameFragment on Pet {
            ...UnknownFragment
          }
          { __typename }
        "#,
    );
}
#[test]
fn spreading_recursively_within_field_fails() {
    expect_fails_rule!(
        factory,
        r#"
          fragment fragA on Human { relatives { ...fragA } },
          { __typename }
        "#,
    );
}
#[test]
fn no_spreading_itself_directly() {
    expect_fails_rule!(
        factory,
        r#"
          fragment fragA on Dog { ...fragA }
          { __typename }
        "#,
    );
}
#[test]
fn no_spreading_itself_directly_within_inline_fragment() {
    expect_fails_rule!(
        factory,
        r#"
          fragment fragA on Pet {
            ... on Dog {
              ...fragA
            }
          }
          { __typename }
        "#,
    );
}
#[test]
fn no_spreading_itself_indirectly() {
    expect_fails_rule!(
        factory,
        r#"
          fragment fragA on Dog { ...fragB }
          fragment fragB on Dog { ...fragA }
          { __typename }
        "#,
    );
}
#[test]
fn no_spreading_itself_indirectly_reports_opposite_order() {
    expect_fails_rule!(
        factory,
        r#"
          fragment fragB on Dog { ...fragA }
          fragment fragA on Dog { ...fragB }
          { __typename }
        "#,
    );
}
#[test]
fn no_spreading_itself_indirectly_within_inline_fragment() {
    expect_fails_rule!(
        factory,
        r#"
          fragment fragA on Pet {
            ... on Dog {
              ...fragB
            }
          }
          fragment fragB on Pet {
            ... on Dog {
              ...fragA
            }
          }
          { __typename }
        "#,
    );
}
#[test]
fn no_spreading_itself_deeply() {
    expect_fails_rule!(
        factory,
        r#"
          fragment fragA on Dog { ...fragB }
          fragment fragB on Dog { ...fragC }
          fragment fragC on Dog { ...fragO }
          fragment fragX on Dog { ...fragY }
          fragment fragY on Dog { ...fragZ }
          fragment fragZ on Dog { ...fragO }
          fragment fragO on Dog { ...fragP }
          fragment fragP on Dog { ...fragA, ...fragX }
          { __typename }
        "#,
    );
}
#[test]
fn no_spreading_itself_deeply_two_paths() {
    expect_fails_rule!(
        factory,
        r#"
          fragment fragA on Dog { ...fragB, ...fragC }
          fragment fragB on Dog { ...fragA }
          fragment fragC on Dog { ...fragA }
          { __typename }
        "#,
    );
}
#[test]
fn no_spreading_itself_deeply_two_paths_alt_traversal_order() {
    expect_fails_rule!(
        factory,
        r#"
          fragment fragA on Dog { ...fragC }
          fragment fragB on Dog { ...fragC }
          fragment fragC on Dog { ...fragA, ...fragB }
          { __typename }
        "#,
    );
}
#[test]
fn no_spreading_itself_deeply_and_immediately() {
    expect_fails_rule!(
        factory,
        r#"
          fragment fragA on Dog { ...fragB }
          fragment fragB on Dog { ...fragB, ...fragC }
          fragment fragC on Dog { ...fragA, ...fragB }
          { __typename }
        "#,
    );
}
