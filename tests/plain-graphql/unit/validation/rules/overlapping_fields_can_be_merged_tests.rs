use super::*;
pub fn factory() -> OverlappingFieldsCanBeMerged {
    OverlappingFieldsCanBeMerged
}
#[test]
fn same_field_on_different_type() {
    expect_passes_rule!(
        factory,
        r#"
          {
           pet {
            ... on Dog {
                doesKnowCommand(dogCommand: SIT)
            }
            ... on Cat {
                doesKnowCommand(catCommand: JUMP)
            }
           }
          }
        "#,
    );
}
#[test]
fn same_field_on_same_type() {
    expect_fails_rule!(
        factory,
        r#"
          {
           pet {
            ... on Dog {
                doesKnowCommand(dogCommand: SIT)
            }
            ... on Dog {
                doesKnowCommand(dogCommand: Heel)
            }
           }
          }
        "#,
    );
}
#[test]
fn same_alias_on_different_type() {
    expect_passes_rule!(
        factory,
        r#"
          {
           pet {
            ... on Dog {
                volume: barkVolume
            }
            ... on Cat {
                volume: meowVolume
            }
           }
          }
        "#,
    );
}
