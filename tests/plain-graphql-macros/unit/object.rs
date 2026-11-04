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
fn generic_objects_without_entities_do_not_emit_internal_entity_delegation() {
    let mut item = syn::parse_quote!(
        impl<T> Query<T> {
            fn value(&self) -> i32 {
                1
            }
        }
    );
    let args = args::Object {
        concretes: vec![args::ConcreteType {
            name: "IntQuery".into(),
            params: args::PathList(vec![syn::parse_quote!(i32)]),
            bounds: Default::default(),
        }],
        ..Default::default()
    };
    let generated = generate(&args, &mut item).unwrap().to_string();
    assert!(!generated.contains("find_entity"));
    assert!(generated.contains("__internal_resolve_field"));
}

#[test]
fn entity_resolvers_keep_validation_and_dispatch() {
    let mut item = syn::parse_quote! {
        impl Query { #[graphql(entity)] async fn item(&self, id: i32) -> Item { Item { id } } }
    };
    let generated = generate(&args::Object::default(), &mut item)
        .unwrap()
        .to_string();
    assert!(generated.contains("find_entity_params"));
    assert!(generated.contains("async fn find_entity"));
    assert!(generated.contains("self . item"));
}

#[test]
fn generic_entity_resolvers_keep_internal_validation_and_concrete_delegation() {
    let mut item = syn::parse_quote! {
        impl<T> Query<T> {
            #[graphql(entity)]
            async fn item(&self, id: T) -> Item<T> { Item { id } }
        }
    };
    let args = args::Object {
        concretes: vec![args::ConcreteType {
            name: "IntQuery".into(),
            params: args::PathList(vec![syn::parse_quote!(i32)]),
            bounds: Default::default(),
        }],
        ..Default::default()
    };
    let generated = generate(&args, &mut item).unwrap().to_string();
    assert!(generated.contains("find_entity_params"));
    assert!(generated.contains("async fn __internal_find_entity"));
    assert!(generated.contains("self . __internal_find_entity"));
}

#[test]
fn parameter_bindings_are_mutable_only_when_a_processor_needs_mutation() {
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
    let generated = generate(&args::Object::default(), &mut processed)
        .unwrap()
        .to_string();
    assert!(generated.contains("mut id"));
}
