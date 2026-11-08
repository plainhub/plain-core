use super::*;

#[test]
fn ordinary_objects_use_the_default_no_entity_path() {
    let mut item = syn::parse_quote!(impl Query { fn value(&self) -> i32 { 1 } });
    let generated = generate(&args::Object::default(), &mut item)
        .unwrap()
        .to_string();
    assert!(!generated.contains("find_entity"));
    assert!(generated.contains("resolve_field"));
}

#[test]
fn unused_entity_and_concrete_options_are_rejected() {
    use crate::meta::FromMeta;
    assert!(
        args::Object::from_list(
            crate::meta::parse_list(quote::quote!(concrete(name = "X", params(i32)))).unwrap()
        )
        .is_err()
    );
    let mut item = syn::parse_quote! { impl Query { #[graphql(entity)] async fn item(&self, id: i32) -> i32 { id } } };
    assert!(generate(&args::Object::default(), &mut item).is_err());
}

#[test]
fn parameters_are_immutable_and_unused_processor_options_are_rejected() {
    let mut plain = syn::parse_quote! {
        impl Query { fn value(&self, id: i32) -> i32 { id } }
    };
    let generated = generate(&args::Object::default(), &mut plain)
        .unwrap()
        .to_string();
    assert!(!generated.contains("mut id"));
    let mut processed = syn::parse_quote! {
        impl Query {
            fn value(&self, #[graphql(process_with = "normalize")] id: i32) -> i32 { id }
        }
    };
    assert!(generate(&args::Object::default(), &mut processed).is_err());
}

#[test]
fn unused_generation_options_are_rejected() {
    use crate::meta::FromMeta;
    for option in [
        "name_type",
        "serial",
        "use_type_description",
        "getters",
        "visible = false",
        r#"rename_fields = "snake_case""#,
        r#"rename_args = "snake_case""#,
    ] {
        let tokens = option.parse().unwrap();
        let items = crate::meta::parse_list(tokens).unwrap();
        assert!(args::Object::from_list(items).is_err(), "{option}");
    }
    for option in [
        "flatten",
        "derived(name = \"extra\", into = \"i32\")",
        "complexity = 3",
    ] {
        let tokens = option.parse().unwrap();
        let items = crate::meta::parse_list(tokens).unwrap();
        assert!(args::ObjectField::from_list(items).is_err(), "{option}");
    }
}

#[test]
fn unused_argument_secrecy_metadata_is_rejected() {
    use crate::meta::FromMeta;
    let items = crate::meta::parse_list(quote::quote!(secret)).unwrap();
    assert!(args::Argument::from_list(items).is_err());
}
