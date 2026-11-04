use darling::ast::Data;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Error, LitInt};

use crate::{
    args::{self, RenameTarget, TypeDirectiveLocation},
    utils::{GeneratorResult, gen_directive_calls, get_crate_path, get_rustdoc, visible_fn},
};

pub fn generate(object_args: &args::MergedObject) -> GeneratorResult<TokenStream> {
    let crate_name = get_crate_path(&object_args.crate_path, object_args.internal);
    let ident = &object_args.ident;
    let (impl_generics, ty_generics, where_clause) = object_args.generics.split_for_impl();
    let extends = object_args.extends;
    let shareable = object_args.shareable;
    let inaccessible = object_args.inaccessible;
    let interface_object = object_args.interface_object;
    let tags = object_args
        .tags
        .iter()
        .map(|tag| quote!(::std::string::ToString::to_string(#tag)))
        .collect::<Vec<_>>();
    let gql_typename = if !object_args.name_type {
        let name = object_args
            .name
            .clone()
            .unwrap_or_else(|| RenameTarget::Type.rename(ident.to_string()));
        quote!(::std::borrow::Cow::Borrowed(#name))
    } else {
        quote!(<Self as #crate_name::TypeName>::type_name())
    };
    let gql_typename_string = if !object_args.name_type {
        let name = object_args
            .name
            .clone()
            .unwrap_or_else(|| RenameTarget::Type.rename(ident.to_string()));
        quote!(::std::string::ToString::to_string(#name))
    } else {
        quote!(::std::string::ToString::to_string(&#gql_typename))
    };

    let directives = gen_directive_calls(
        &crate_name,
        &object_args.directives,
        TypeDirectiveLocation::Object,
    );

    let desc = get_rustdoc(&object_args.attrs)?
        .map(|s| quote! { ::std::option::Option::Some(::std::string::ToString::to_string(#s)) })
        .unwrap_or_else(|| quote! {::std::option::Option::None});

    let s = match &object_args.data {
        Data::Struct(e) => e,
        _ => {
            return Err(Error::new_spanned(
                ident,
                "MergedObject can only be applied to an struct.",
            )
            .into());
        }
    };

    let mut types = Vec::new();
    for field in &s.fields {
        types.push(&field.ty);
    }

    let register_members = types.iter().map(|ty| {
        quote! {
            if let #crate_name::registry::MetaType::Object {
                fields: member_fields,
                cache_control: member_cache_control,
                ..
            } = registry.create_fake_output_type::<#ty>() {
                fields.extend(member_fields);
                merge_cache_control(&mut cache_control, member_cache_control);
            }
        }
    });

    let flat_resolve_field = {
        let checks: Vec<_> = (0..types.len())
            .rev()
            .map(|i| {
                let n = LitInt::new(&format!("{}", i), Span::call_site());
                quote! {
                    if let ::std::option::Option::Some(value) = self.#n.resolve_field(ctx).await? {
                        return ::std::result::Result::Ok(::std::option::Option::Some(value));
                    }
                }
            })
            .collect();
        quote! { #(#checks)* }
    };

    let flat_find_entity = {
        let checks: Vec<_> = (0..types.len())
            .rev()
            .map(|i| {
                let n = LitInt::new(&format!("{}", i), Span::call_site());
                quote! {
                    if let ::std::option::Option::Some(value) = self.#n.find_entity(ctx, params).await? {
                        return ::std::result::Result::Ok(::std::option::Option::Some(value));
                    }
                }
            })
            .collect();
        quote! { #(#checks)* }
    };

    let visible = visible_fn(&object_args.visible);
    let resolve_container = if object_args.serial {
        quote! { #crate_name::resolver_utils::resolve_container_serial(ctx, self).await }
    } else {
        quote! { #crate_name::resolver_utils::resolve_container(ctx, self).await }
    };

    let expanded = quote! {

        impl #impl_generics #crate_name::resolver_utils::ContainerType for #ident #ty_generics #where_clause {
            async fn resolve_field(&self, ctx: &#crate_name::Context<'_>) -> #crate_name::ServerResult<::std::option::Option<#crate_name::Value>> {
                #flat_resolve_field
                ::std::result::Result::Ok(::std::option::Option::None)
            }

            async fn find_entity(&self, ctx: &#crate_name::Context<'_>, params: &#crate_name::Value) ->  #crate_name::ServerResult<::std::option::Option<#crate_name::Value>> {
                #flat_find_entity
                ::std::result::Result::Ok(::std::option::Option::None)
            }
        }

        impl #impl_generics #crate_name::OutputType for #ident #ty_generics #where_clause {
            fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                #gql_typename
            }

            fn create_type_info(registry: &mut #crate_name::registry::Registry) -> ::std::string::String {
                registry.create_output_type::<Self, _>(#crate_name::registry::MetaTypeId::Object, |registry| {
                    let mut fields = #crate_name::indexmap::IndexMap::new();
                    let mut cache_control = #crate_name::CacheControl::default();

                    fn merge_cache_control(current: &mut #crate_name::CacheControl, member: #crate_name::CacheControl) {
                        current.public &= member.public;
                        if member.max_age == -1 || (current.max_age != -1 && member.max_age != 0 && (current.max_age == 0 || member.max_age < current.max_age)) {
                            current.max_age = member.max_age;
                        }
                    }
                    #(#register_members)*

                    #crate_name::registry::MetaType::Object {
                        name: #gql_typename_string,
                        description: #desc,
                        fields,
                        cache_control,
                        extends: #extends,
                        shareable: #shareable,
                        resolvable: true,
                        inaccessible: #inaccessible,
                        interface_object: #interface_object,
                        tags: ::std::vec![ #(#tags),* ],
                        keys: ::std::option::Option::None,
                        visible: #visible,
                        is_subscription: false,
                        rust_typename: ::std::option::Option::Some(::std::any::type_name::<Self>()),
                        directive_invocations: ::std::vec![ #(#directives),* ],
                        requires_scopes: ::std::vec![],
                    }
                })
            }

            async fn resolve(&self, ctx: &#crate_name::ContextSelectionSet<'_>, _field: &#crate_name::Positioned<#crate_name::parser::types::Field>) -> #crate_name::ServerResult<#crate_name::Value> {
                #resolve_container
            }
        }

        impl #impl_generics #crate_name::ObjectType for #ident #ty_generics #where_clause {}
    };
    Ok(expanded)
}

#[cfg(test)]
#[path = "../../tests/plain-graphql-macros/unit/merged_object.rs"]
mod tests;
