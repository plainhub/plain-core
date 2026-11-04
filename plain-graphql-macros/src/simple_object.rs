use crate::{
    args::{self, RenameTarget},
    utils::{GeneratorResult, gen_deprecation, generate_guards, get_crate_path, get_rustdoc},
};
use darling::ast::Data;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Error, ext::IdentExt};
pub fn generate(object_args: &args::SimpleObject) -> GeneratorResult<TokenStream> {
    let crate_name = get_crate_path(&object_args.crate_path, object_args.internal);
    let ident = &object_args.ident;
    let (impl_generics, ty_generics, where_clause) = object_args.generics.split_for_impl();
    let gql_typename = {
        object_args
            .name
            .as_ref()
            .map(|name| quote ! (::std::borrow::Cow::Borrowed(#name)))
            .unwrap_or_else(|| {
                let name = RenameTarget::Type.rename(ident.to_string());
                quote ! (::std::borrow::Cow::Borrowed(#name))
            })
    };
    let gql_typename_string = {
        let name = object_args
            .name
            .as_ref()
            .cloned()
            .unwrap_or_else(|| RenameTarget::Type.rename(ident.to_string()));
        quote ! (::std::string::ToString::to_string(#name))
    };
    let desc_value = get_rustdoc(&object_args.attrs)?;
    let has_desc = desc_value.is_some();
    let desc = desc_value
        .map(|s| quote! {::std::option::Option::Some(::std::string::ToString::to_string(#s))})
        .unwrap_or_else(|| quote! { ::std::option::Option::None });
    let s = match &object_args.data {
        Data::Struct(e) => e,
        _ => {
            return Err(Error::new_spanned(
                ident,
                "SimpleObject can only be applied to an struct.",
            )
            .into());
        }
    };
    let mut getters = Vec::new();
    let mut resolvers = Vec::new();
    let mut schema_fields = Vec::new();
    for field in &s.fields {
        if field.skip {
            continue;
        }
        let ident = field
            .ident
            .as_ref()
            .ok_or_else(|| Error::new_spanned(ident, "All fields must be named."))?;
        let field_name = field
            .name
            .clone()
            .unwrap_or_else(|| RenameTarget::Field.rename(ident.unraw().to_string()));
        let field_desc_value = get_rustdoc(&field.attrs)?;
        let has_field_desc = field_desc_value.is_some();
        let field_desc = field_desc_value
            .map(|s| quote! {::std::option::Option::Some(::std::string::ToString::to_string(#s))})
            .unwrap_or_else(|| quote! { ::std::option::Option::None });
        let field_deprecation = gen_deprecation(&field.deprecation, &crate_name);
        let ty = &field.ty;
        let cache_control = {
            let public = field.cache_control.is_public();
            let max_age = if field.cache_control.no_cache {
                -1
            } else {
                field.cache_control.max_age as i32
            };
            quote! {
                #crate_name::CacheControl {
                    public: #public,
                    max_age: #max_age,
                }
            }
        };
        let has_cache_control = field.cache_control.no_cache
            || field.cache_control.max_age != 0
            || !field.cache_control.is_public();
        let has_deprecation = !matches!(field.deprecation, args::Deprecation::NoDeprecated);
        {
            let mut field_sets = Vec::new();
            if has_field_desc {
                field_sets.push(quote ! (field.description = #field_desc;));
            }
            if has_deprecation {
                field_sets.push(quote ! (field.deprecation = #field_deprecation;));
            }
            if has_cache_control {
                field_sets.push(quote ! (field.cache_control = #cache_control;));
            }
            schema_fields.push(quote! {
                let mut field = #crate_name::registry::MetaField::new(
                    ::std::string::ToString::to_string(#field_name),
                    <#ty as #crate_name::OutputType>::create_type_info(registry),
                );
                #(#field_sets)*
                fields.insert(::std::string::ToString::to_string(#field_name), field);
            });
        }
        let guard_map_err = quote! {
            .map_err(|err| ctx.set_error_path(err.into_server_error(ctx.item.pos)))
        };
        let guard = match field.guard.as_ref().or(object_args.guard.as_ref()) {
            Some(code) => Some(generate_guards(&crate_name, code, guard_map_err)),
            None => None,
        };
        let block = quote! { &self.#ident };
        let vis = &field.vis;
        let ty = quote! { &#ty };
        {
            if field.getter {
                getters . push (quote ! {
                 #[inline]

                 #vis async fn #ident(&self, ctx: &#crate_name::Context<'_>) -> #crate_name::Result<#ty> {
                     ::std::result::Result::Ok(#block)
                 }
            }) ;
            }
            resolvers.push(quote! {
                if ctx.item.node.name.node == #field_name {
                    #guard
                    let obj: #ty = #block;
                    return #crate_name::resolver_utils::resolve_simple_field_value(ctx, &obj).await;
                }
            });
        }
    }
    if resolvers.is_empty() {
        return Err(Error::new_spanned(
            ident,
            "A GraphQL Object type must define one or more fields.",
        )
        .into());
    }
    let cache_control = {
        let public = object_args.cache_control.is_public();
        let max_age = if object_args.cache_control.no_cache {
            -1
        } else {
            object_args.cache_control.max_age as i32
        };
        quote! {
            #crate_name::CacheControl {
                public: #public,
                max_age: #max_age,
            }
        }
    };
    let has_cache_control = object_args.cache_control.no_cache
        || object_args.cache_control.max_age != 0
        || !object_args.cache_control.is_public();
    let field_count = schema_fields.len();
    let mut object_builder_base = Vec::new();
    object_builder_base.push(quote ! (.rust_typename(::std::any::type_name::<Self>())));
    if has_desc {
        object_builder_base.push(quote ! (.description(#desc)));
    }
    if has_cache_control {
        object_builder_base.push(quote ! (.cache_control(#cache_control)));
    }
    let object_builder = object_builder_base;
    let mut concat_complex_fields = quote!();
    let mut complex_resolver = quote!();
    if object_args.complex {
        concat_complex_fields = quote! {
            fields.extend(<Self as #crate_name::ComplexObject>::fields(registry));
        };
        complex_resolver = quote! {
            if let Some(value) = <Self as #crate_name::ComplexObject>::resolve_field(self, ctx).await? {
                return Ok(Some(value));
            }
        };
    }
    let resolve_container = {
        quote! { #crate_name::resolver_utils::resolve_container(ctx, self).await }
    };
    let getter_impl = (!getters.is_empty()).then(|| {
        quote! {
            impl #impl_generics #ident #ty_generics #where_clause {
                #(#getters)*
            }
        }
    });
    let expanded = {
        quote! {
            #getter_impl

            impl #impl_generics #crate_name::resolver_utils::ContainerType for #ident #ty_generics #where_clause {
                async fn resolve_field(&self, ctx: &#crate_name::Context<'_>) -> #crate_name::ServerResult<::std::option::Option<#crate_name::Value>> {
                    #(#resolvers)*
                    #complex_resolver
                    ::std::result::Result::Ok(::std::option::Option::None)
                }
            }

            impl #impl_generics #crate_name::OutputType for #ident #ty_generics #where_clause {
                fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                    #gql_typename
                }

                fn create_type_info(registry: &mut #crate_name::registry::Registry) -> ::std::string::String {
                    registry.create_output_type::<Self, _>(#crate_name::registry::MetaTypeId::Object, |registry| {
                        #crate_name::registry::ObjectBuilder::new(
                            #gql_typename_string,
                            {
                                let mut fields = #crate_name::indexmap::IndexMap::with_capacity(#field_count);
                                #(#schema_fields)*
                                #concat_complex_fields
                                fields
                            },
                        )
                        #(#object_builder)*
                        .build()
                    })
                }

                async fn resolve(&self, ctx: &#crate_name::ContextSelectionSet<'_>, _field: &#crate_name::Positioned<#crate_name::parser::types::Field>) -> #crate_name::ServerResult<#crate_name::Value> {
                    #resolve_container
                }
            }

            impl #impl_generics #crate_name::ObjectType for #ident #ty_generics #where_clause {}
        }
    };
    Ok(expanded)
}
#[cfg(test)]
#[path = "../../tests/plain-graphql-macros/unit/simple_object.rs"]
mod tests;
