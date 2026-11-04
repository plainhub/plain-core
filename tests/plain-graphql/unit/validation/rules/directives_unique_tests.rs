use super::*;
pub fn factory() -> DirectivesUnique {
    DirectivesUnique
}
#[test]
fn skip_on_field() {
    expect_passes_rule!(
        factory,
        r#"
          {
            dog {
              name @skip(if: true)
            }
          }
        "#,
    );
}
#[test]
fn duplicate_skip_on_field() {
    expect_fails_rule!(
        factory,
        r#"
          {
            dog {
              name @skip(if: true) @skip(if: false)
            }
          }
        "#,
    );
}
#[test]
fn skip_on_fragment_spread() {
    expect_passes_rule!(
        factory,
        r#"
          fragment A on Dog {
            name
          }

          query {
            dog ... A @skip(if: true)
          }
        "#,
    );
}
#[test]
fn duplicate_skip_on_fragment_spread() {
    expect_fails_rule!(
        factory,
        r#"
          fragment A on Dog {
            name
          }

          query {
            dog ... A @skip(if: true) @skip(if: false)
          }
        "#,
    );
}
#[test]
fn skip_on_inline_fragment() {
    expect_passes_rule!(
        factory,
        r#"
          query {
            dog ... @skip(if: true) {
                name
            }
          }
        "#,
    );
}
#[test]
fn duplicate_skip_on_inline_fragment() {
    expect_fails_rule!(
        factory,
        r#"
          query {
            dog ... @skip(if: true) @skip(if: false) {
                name
            }
          }
        "#,
    );
}
