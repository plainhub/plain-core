use super::*;
pub fn factory<'a>() -> KnownArgumentNames<'a> {
    KnownArgumentNames::default()
}
#[test]
fn single_arg_is_known() {
    expect_passes_rule!(
        factory,
        r#"
          fragment argOnRequiredArg on Dog {
            doesKnowCommand(dogCommand: SIT)
          }
          { __typename }
        "#,
    );
}
#[test]
fn multiple_args_are_known() {
    expect_passes_rule!(
        factory,
        r#"
          fragment multipleArgs on ComplicatedArgs {
            multipleReqs(req1: 1, req2: 2)
          }
          { __typename }
        "#,
    );
}
#[test]
fn ignores_args_of_unknown_fields() {
    expect_passes_rule!(
        factory,
        r#"
          fragment argOnUnknownField on Dog {
            unknownField(unknownArg: SIT)
          }
          { __typename }
        "#,
    );
}
#[test]
fn multiple_args_in_reverse_order_are_known() {
    expect_passes_rule!(
        factory,
        r#"
          fragment multipleArgsReverseOrder on ComplicatedArgs {
            multipleReqs(req2: 2, req1: 1)
          }
          { __typename }
        "#,
    );
}
#[test]
fn no_args_on_optional_arg() {
    expect_passes_rule!(
        factory,
        r#"
          fragment noArgOnOptionalArg on Dog {
            isHousetrained
          }
          { __typename }
        "#,
    );
}
#[test]
fn args_are_known_deeply() {
    expect_passes_rule!(
        factory,
        r#"
          {
            dog {
              doesKnowCommand(dogCommand: SIT)
            }
            human {
              pet {
                ... on Dog {
                  doesKnowCommand(dogCommand: SIT)
                }
              }
            }
          }
        "#,
    );
}
#[test]
fn directive_args_are_known() {
    expect_passes_rule!(
        factory,
        r#"
          {
            dog @skip(if: true)
          }
        "#,
    );
}
#[test]
fn undirective_args_are_invalid() {
    expect_fails_rule!(
        factory,
        r#"
          {
            dog @skip(unless: true)
          }
        "#,
    );
}
#[test]
fn invalid_arg_name() {
    expect_fails_rule!(
        factory,
        r#"
          fragment invalidArgName on Dog {
            doesKnowCommand(unknown: true)
          }
          { __typename }
        "#,
    );
}
#[test]
fn unknown_args_amongst_known_args() {
    expect_fails_rule!(
        factory,
        r#"
          fragment oneGoodArgOneInvalidArg on Dog {
            doesKnowCommand(whoknows: 1, dogCommand: SIT, unknown: true)
          }
          { __typename }
        "#,
    );
}
#[test]
fn unknown_args_deeply() {
    expect_fails_rule!(
        factory,
        r#"
          {
            dog {
              doesKnowCommand(unknown: true)
            }
            human {
              pet {
                ... on Dog {
                  doesKnowCommand(unknown: true)
                }
              }
            }
          }
        "#,
    );
}
