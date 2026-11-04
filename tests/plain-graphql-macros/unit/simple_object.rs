use super::*;
use darling::FromDeriveInput;

fn getters(input: syn::DeriveInput) -> Vec<String> {
    let args = args::SimpleObject::from_derive_input(&input).unwrap();
    syn::parse2::<syn::File>(generate(&args).unwrap())
        .unwrap()
        .items
        .into_iter()
        .filter_map(|item| match item {
            syn::Item::Impl(item) if item.trait_.is_none() => Some(item),
            _ => None,
        })
        .flat_map(|item| {
            item.items.into_iter().filter_map(|item| match item {
                syn::ImplItem::Fn(item) => Some(item.sig.ident.to_string()),
                _ => None,
            })
        })
        .collect()
}

#[test]
fn existing_objects_keep_getters_by_default() {
    assert_eq!(
        getters(syn::parse_quote!(
            struct Item {
                value: i32,
            }
        )),
        ["value"]
    );
}

#[test]
fn disabled_getters_keep_only_explicit_interface_methods() {
    assert_eq!(
        getters(syn::parse_quote! {
            #[graphql(getters = false)]
            struct Item { hidden_method: i32, #[graphql(getter = true)] id: i32 }
        }),
        ["id"]
    );
}

#[test]
fn field_opt_out_preserves_other_getters() {
    assert_eq!(
        getters(syn::parse_quote! {
            struct Item { #[graphql(getter = false)] hidden_method: i32, id: i32 }
        }),
        ["id"]
    );
}

#[test]
fn disabled_getters_do_not_emit_an_empty_inherent_impl() {
    assert!(
        getters(syn::parse_quote! {
            #[graphql(getters = false)] struct Item { value: i32 }
        })
        .is_empty()
    );
    let args = args::SimpleObject::from_derive_input(&syn::parse_quote! {
        #[graphql(getters = false)] struct Item { value: i32 }
    })
    .unwrap();
    let generated = syn::parse2::<syn::File>(generate(&args).unwrap()).unwrap();
    assert!(
        !generated
            .items
            .iter()
            .any(|item| matches!(item, syn::Item::Impl(item) if item.trait_.is_none()))
    );
}
