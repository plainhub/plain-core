use super::*;
pub fn factory<'a>() -> NoUnusedFragments<'a> {
    NoUnusedFragments::default()
}
#[test]
fn all_fragment_names_are_used() {
    expect_passes_rule!(
        factory,
        r#"
          {
            human(id: 4) {
              ...HumanFields1
              ... on Human {
                ...HumanFields2
              }
            }
          }
          fragment HumanFields1 on Human {
            name
            ...HumanFields3
          }
          fragment HumanFields2 on Human {
            name
          }
          fragment HumanFields3 on Human {
            name
          }
        "#,
    );
}
#[test]
fn all_fragment_names_are_used_by_multiple_operations() {
    expect_passes_rule!(
        factory,
        r#"
          query Foo {
            human(id: 4) {
              ...HumanFields1
            }
          }
          query Bar {
            human(id: 4) {
              ...HumanFields2
            }
          }
          fragment HumanFields1 on Human {
            name
            ...HumanFields3
          }
          fragment HumanFields2 on Human {
            name
          }
          fragment HumanFields3 on Human {
            name
          }
        "#,
    );
}
#[test]
fn contains_unknown_fragments() {
    expect_fails_rule!(
        factory,
        r#"
          query Foo {
            human(id: 4) {
              ...HumanFields1
            }
          }
          query Bar {
            human(id: 4) {
              ...HumanFields2
            }
          }
          fragment HumanFields1 on Human {
            name
            ...HumanFields3
          }
          fragment HumanFields2 on Human {
            name
          }
          fragment HumanFields3 on Human {
            name
          }
          fragment Unused1 on Human {
            name
          }
          fragment Unused2 on Human {
            name
          }
        "#,
    );
}
#[test]
fn contains_unknown_fragments_with_ref_cycle() {
    expect_fails_rule!(
        factory,
        r#"
          query Foo {
            human(id: 4) {
              ...HumanFields1
            }
          }
          query Bar {
            human(id: 4) {
              ...HumanFields2
            }
          }
          fragment HumanFields1 on Human {
            name
            ...HumanFields3
          }
          fragment HumanFields2 on Human {
            name
          }
          fragment HumanFields3 on Human {
            name
          }
          fragment Unused1 on Human {
            name
            ...Unused2
          }
          fragment Unused2 on Human {
            name
            ...Unused1
          }
        "#,
    );
}
#[test]
fn contains_unknown_and_undef_fragments() {
    expect_fails_rule!(
        factory,
        r#"
          query Foo {
            human(id: 4) {
              ...bar
            }
          }
          fragment foo on Human {
            name
          }
        "#,
    );
}
