use super::*;
pub fn factory<'a>() -> PossibleFragmentSpreads<'a> {
    PossibleFragmentSpreads::default()
}
#[test]
fn of_the_same_object() {
    expect_passes_rule!(
        factory,
        r#"
          fragment objectWithinObject on Dog { ...dogFragment }
          fragment dogFragment on Dog { barkVolume }
          { __typename }
        "#,
    );
}
#[test]
fn of_the_same_object_with_inline_fragment() {
    expect_passes_rule!(
        factory,
        r#"
          fragment objectWithinObjectAnon on Dog { ... on Dog { barkVolume } }
          { __typename }
        "#,
    );
}
#[test]
fn object_into_an_implemented_interface() {
    expect_passes_rule!(
        factory,
        r#"
          fragment objectWithinInterface on Pet { ...dogFragment }
          fragment dogFragment on Dog { barkVolume }
          { __typename }
        "#,
    );
}
#[test]
fn object_into_containing_union() {
    expect_passes_rule!(
        factory,
        r#"
          fragment objectWithinUnion on CatOrDog { ...dogFragment }
          fragment dogFragment on Dog { barkVolume }
          { __typename }
        "#,
    );
}
#[test]
fn union_into_contained_object() {
    expect_passes_rule!(
        factory,
        r#"
          fragment unionWithinObject on Dog { ...catOrDogFragment }
          fragment catOrDogFragment on CatOrDog { __typename }
          { __typename }
        "#,
    );
}
#[test]
fn union_into_overlapping_interface() {
    expect_passes_rule!(
        factory,
        r#"
          fragment unionWithinInterface on Pet { ...catOrDogFragment }
          fragment catOrDogFragment on CatOrDog { __typename }
          { __typename }
        "#,
    );
}
#[test]
fn union_into_overlapping_union() {
    expect_passes_rule!(
        factory,
        r#"
          fragment unionWithinUnion on DogOrHuman { ...catOrDogFragment }
          fragment catOrDogFragment on CatOrDog { __typename }
          { __typename }
        "#,
    );
}
#[test]
fn interface_into_implemented_object() {
    expect_passes_rule!(
        factory,
        r#"
          fragment interfaceWithinObject on Dog { ...petFragment }
          fragment petFragment on Pet { name }
          { __typename }
        "#,
    );
}
#[test]
fn interface_into_overlapping_interface() {
    expect_passes_rule!(
        factory,
        r#"
          fragment interfaceWithinInterface on Pet { ...beingFragment }
          fragment beingFragment on Being { name }
          { __typename }
        "#,
    );
}
#[test]
fn interface_into_overlapping_interface_in_inline_fragment() {
    expect_passes_rule!(
        factory,
        r#"
          fragment interfaceWithinInterface on Pet { ... on Being { name } }
          { __typename }
        "#,
    );
}
#[test]
fn interface_into_overlapping_union() {
    expect_passes_rule!(
        factory,
        r#"
          fragment interfaceWithinUnion on CatOrDog { ...petFragment }
          fragment petFragment on Pet { name }
          { __typename }
        "#,
    );
}
#[test]
fn different_object_into_object() {
    expect_fails_rule!(
        factory,
        r#"
          fragment invalidObjectWithinObject on Cat { ...dogFragment }
          fragment dogFragment on Dog { barkVolume }
          { __typename }
        "#,
    );
}
#[test]
fn different_object_into_object_in_inline_fragment() {
    expect_fails_rule!(
        factory,
        r#"
          fragment invalidObjectWithinObjectAnon on Cat {
            ... on Dog { barkVolume }
          }
          { __typename }
        "#,
    );
}
#[test]
fn object_into_not_implementing_interface() {
    expect_fails_rule!(
        factory,
        r#"
          fragment invalidObjectWithinInterface on Pet { ...humanFragment }
          fragment humanFragment on Human { pets { name } }
          { __typename }
        "#,
    );
}
#[test]
fn object_into_not_containing_union() {
    expect_fails_rule!(
        factory,
        r#"
          fragment invalidObjectWithinUnion on CatOrDog { ...humanFragment }
          fragment humanFragment on Human { pets { name } }
          { __typename }
        "#,
    );
}
#[test]
fn union_into_not_contained_object() {
    expect_fails_rule!(
        factory,
        r#"
          fragment invalidUnionWithinObject on Human { ...catOrDogFragment }
          fragment catOrDogFragment on CatOrDog { __typename }
          { __typename }
        "#,
    );
}
#[test]
fn union_into_non_overlapping_interface() {
    expect_fails_rule!(
        factory,
        r#"
          fragment invalidUnionWithinInterface on Pet { ...humanOrAlienFragment }
          fragment humanOrAlienFragment on HumanOrAlien { __typename }
          { __typename }
        "#,
    );
}
#[test]
fn union_into_non_overlapping_union() {
    expect_fails_rule!(
        factory,
        r#"
          fragment invalidUnionWithinUnion on CatOrDog { ...humanOrAlienFragment }
          fragment humanOrAlienFragment on HumanOrAlien { __typename }
          { __typename }
        "#,
    );
}
#[test]
fn interface_into_non_implementing_object() {
    expect_fails_rule!(
        factory,
        r#"
          fragment invalidInterfaceWithinObject on Cat { ...intelligentFragment }
          fragment intelligentFragment on Intelligent { iq }
          { __typename }
        "#,
    );
}
#[test]
fn interface_into_non_overlapping_interface() {
    expect_fails_rule!(
        factory,
        r#"
          fragment invalidInterfaceWithinInterface on Pet {
            ...intelligentFragment
          }
          fragment intelligentFragment on Intelligent { iq }
          { __typename }
        "#,
    );
}
#[test]
fn interface_into_non_overlapping_interface_in_inline_fragment() {
    expect_fails_rule!(
        factory,
        r#"
          fragment invalidInterfaceWithinInterfaceAnon on Pet {
            ...on Intelligent { iq }
          }
          { __typename }
        "#,
    );
}
#[test]
fn interface_into_non_overlapping_union() {
    expect_fails_rule!(
        factory,
        r#"
          fragment invalidInterfaceWithinUnion on HumanOrAlien { ...petFragment }
          fragment petFragment on Pet { name }
          { __typename }
        "#,
    );
}
