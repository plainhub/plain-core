use super::*;
pub fn factory() -> KnownDirectives {
    KnownDirectives::default()
}
#[test]
fn with_no_directives() {
    expect_passes_rule!(
        factory,
        r#"
          query Foo {
            name
            ...Frag
          }
          fragment Frag on Dog {
            name
          }
        "#,
    );
}
#[test]
fn with_known_directives() {
    expect_passes_rule!(
        factory,
        r#"
          {
            dog @include(if: true) {
              name
            }
            human @skip(if: false) {
              name
            }
          }
        "#,
    );
}
#[test]
fn with_unknown_directive() {
    expect_fails_rule!(
        factory,
        r#"
          {
            dog @unknown(directive: "value") {
              name
            }
          }
        "#,
    );
}
#[test]
fn with_many_unknown_directives() {
    expect_fails_rule!(
        factory,
        r#"
          {
            dog @unknown(directive: "value") {
              name
            }
            human @unknown(directive: "value") {
              name
              pets @unknown(directive: "value") {
                name
              }
            }
          }
        "#,
    );
}
#[test]
fn with_well_placed_directives() {
    expect_passes_rule!(
        factory,
        r#"
          query Foo {
            name @include(if: true)
            ...Frag @include(if: true)
            skippedField @skip(if: true)
            ...SkippedFrag @skip(if: true)
          }
          mutation Bar {
            someField
          }
        "#,
    );
}
#[test]
fn with_misplaced_directives() {
    expect_fails_rule!(
        factory,
        r#"
          query Foo @include(if: true) {
            name
            ...Frag
          }
          mutation Bar {
            someField
          }
        "#,
    );
}
