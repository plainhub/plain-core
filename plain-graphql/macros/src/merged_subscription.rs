use crate::{
    args::{self, RenameTarget},
    utils::{GeneratorResult, get_crate_path, get_rustdoc},
};
use crate::meta::Data;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Error, LitInt};
pub fn generate(object_args: &args::MergedSubscription) -> GeneratorResult<TokenStream> {
    let crate_name = get_crate_path(&object_args.crate_path, object_args.internal);
    let ident = &object_args.ident;
    let (impl_generics, ty_generics, where_clause) = object_args.generics.split_for_impl();
    let gql_typename = {
        let name = object_args
            .name
            .clone()
            .unwrap_or_else(|| RenameTarget::Type.rename(ident.to_string()));
        quote ! (::std::borrow::Cow::Borrowed(#name))
    };
    let gql_typename_string = {
        let name = object_args
            .name
            .clone()
            .unwrap_or_else(|| RenameTarget::Type.rename(ident.to_string()));
        quote ! (::std::string::ToString::to_string(#name))
    };
    let desc = get_rustdoc(&object_args.attrs)?
        .map(|s| quote! { ::std::option::Option::Some(::std::string::ToString::to_string(#s)) })
        .unwrap_or_else(|| quote! {::std::option::Option::None});
    let s = match &object_args.data {
        Data::Struct(e) => e,
        _ => {
            return Err(Error::new_spanned(
                ident,
                "MergedSubscription can only be applied to an struct.",
            )
            .into());
        }
    };
    let types: Vec<_> = s.fields.iter().map(|field| &field.ty).collect();
    let create_field_stream: proc_macro2::TokenStream = (0..types.len())
        .map(|i| {
            let n = LitInt::new(&i.to_string(), Span::call_site());
            quote ! (.or_else(|| #crate_name::SubscriptionType::create_field_stream(&self.#n, ctx)))
        })
        .collect();
    let expanded = quote! {
        impl #impl_generics #crate_name::SubscriptionType for #ident #ty_generics #where_clause {
            fn type_name() -> ::std::borrow::Cow<'static, str> { #gql_typename }
            fn create_type_info(registry: &mut #crate_name::registry::Registry) -> String {
                registry.create_subscription_type::<Self, _>(|registry| {
                    let mut fields = #crate_name::indexmap::IndexMap::new();
                    #(
                        if let #crate_name::registry::MetaType::Object { fields: member_fields, .. } = registry.create_fake_subscription_type::<#types>() {
                            fields.extend(member_fields);
                        }
                    )*
                    #crate_name::registry::ObjectBuilder::new(#gql_typename_string, fields)
                        .description(#desc).is_subscription(true)
                        .rust_typename(::std::any::type_name::<Self>()).build()
                })
            }
            fn create_field_stream<'a>(&'a self, ctx: &'a #crate_name::Context<'_>) -> Option<::std::pin::Pin<Box<dyn #crate_name::futures_util::stream::Stream<Item = #crate_name::Response> + Send + 'a>>> {
                None #create_field_stream
            }
        }
    };
    Ok(expanded)
}
#[cfg(test)]
#[path = "../../../tests/plain-graphql-macros/unit/merged_subscription.rs"]
mod tests;
