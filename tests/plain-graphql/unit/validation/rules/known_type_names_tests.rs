use super::*;
pub fn factory() -> KnownTypeNames {
    KnownTypeNames
}
#[test]
fn known_type_names_are_valid() {
    expect_passes_rule!(
        factory,
        r#"
          query Foo($var: String, $required: [String!]!) {
            user(id: 4) {
              pets { ... on Pet { name }, ...PetFields, ... { name } }
            }
          }
          fragment PetFields on Pet {
            name
          }
        "#,
    );
}
#[test]
fn unknown_type_names_are_invalid() {
    expect_fails_rule!(
        factory,
        r#"
          query Foo($var: JumbledUpLetters) {
            user(id: 4) {
              name
              pets { ... on Badger { name }, ...PetFields }
            }
          }
          fragment PetFields on Peettt {
            name
          }
        "#,
    );
}
