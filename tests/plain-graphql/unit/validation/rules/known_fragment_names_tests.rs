use super::*;
pub fn factory() -> KnownFragmentNames {
    KnownFragmentNames
}
#[test]
fn known() {
    expect_passes_rule!(
        factory,
        r#"
          {
            human(id: 4) {
              ...HumanFields1
              ... on Human {
                ...HumanFields2
              }
              ... {
                name
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
fn unknown() {
    expect_fails_rule!(
        factory,
        r#"
          {
            human(id: 4) {
              ...UnknownFragment1
              ... on Human {
                ...UnknownFragment2
              }
            }
          }
          fragment HumanFields on Human {
            name
            ...UnknownFragment3
          }
        "#,
    );
}
