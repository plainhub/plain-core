use crate::{
    args::{self, RenameTarget},
    utils::{GeneratorResult, RemoveLifetime, get_crate_path, get_rustdoc},
};
use darling::ast::{Data, Style};
use proc_macro::TokenStream;
use quote::quote;
use std::collections::HashSet;
use syn::{Error, Type, visit_mut::VisitMut};
pub fn generate(union_args: &args::Union) -> GeneratorResult<TokenStream> {
    let crate_name = get_crate_path(&union_args.crate_path, union_args.internal);
    let ident = &union_args.ident;
    let type_params = union_args.generics.type_params().collect::<Vec<_>>();
    let (impl_generics, ty_generics, where_clause) = union_args.generics.split_for_impl();
    let s = match &union_args.data {
        Data::Enum(s) => s,
        _ => return Err(Error::new_spanned(ident, "Union can only be applied to an enum.").into()),
    };
    let mut enum_names = Vec::new();
    let mut enum_items = HashSet::new();
    let mut type_into_impls = Vec::new();
    let gql_typename = {
        let name = union_args
            .name
            .clone()
            .unwrap_or_else(|| RenameTarget::Type.rename(ident.to_string()));
        quote ! (::std::borrow::Cow::Borrowed(#name))
    };
    let gql_typename_string = {
        let name = union_args
            .name
            .clone()
            .unwrap_or_else(|| RenameTarget::Type.rename(ident.to_string()));
        quote ! (::std::string::ToString::to_string(#name))
    };
    let desc = get_rustdoc(&union_args.attrs)?
        .map(|s| quote! { ::std::option::Option::Some(::std::string::ToString::to_string(#s)) })
        .unwrap_or_else(|| quote! {::std::option::Option::None});
    let mut lazy_types = Vec::new();
    #[derive(Clone)]
    struct LazyType {
        ty: syn::Type,
        enum_name: syn::Ident,
    }
    let mut collect_all_fields = Vec::new();
    for variant in s {
        let enum_name = &variant.ident;
        let ty = match variant.fields.style {
            Style::Tuple if variant.fields.fields.len() == 1 => &variant.fields.fields[0],
            Style::Tuple => {
                return Err(Error::new_spanned(
                    enum_name,
                    "Only single value variants are supported",
                )
                .into());
            }
            Style::Unit => {
                return Err(
                    Error::new_spanned(enum_name, "Empty variants are not supported").into(),
                );
            }
            Style::Struct => {
                return Err(Error::new_spanned(
                    enum_name,
                    "Variants with named fields are not supported",
                )
                .into());
            }
        };
        let mut ty = ty;
        while let Type::Group(group) = ty {
            ty = &*group.elem;
        }
        if matches!(ty, Type::Path(_) | Type::Macro(_)) {
            if !enum_items.insert(ty) {
                return Err(
                    Error::new_spanned(ty, "This type is already used in another variant").into(),
                );
            }
            enum_names.push(enum_name);
            let mut assert_ty = ty.clone();
            RemoveLifetime.visit_type_mut(&mut assert_ty);
            {
                type_into_impls . push (quote ! { # crate_name :: static_assertions_next :: assert_impl ! (for (# (# type_params) ,*) # assert_ty : # crate_name :: ObjectType) ; impl # impl_generics :: std :: convert :: From <# ty > for # ident # ty_generics # where_clause { fn from (obj : # ty) -> Self { # ident ::# enum_name (obj) } } }) ;
            }
            lazy_types.push(LazyType {
                ty: ty.clone(),
                enum_name: enum_name.clone(),
            });
            collect_all_fields.push(quote! {
                #ident::#enum_name(obj) => obj.collect_all_fields(ctx, fields)
            });
        } else {
            return Err(Error::new_spanned(ty, "Invalid type").into());
        }
    }
    if lazy_types.is_empty() {
        return Err(Error::new_spanned(
            ident,
            "A GraphQL Union type must include one or more unique member types.",
        )
        .into());
    }
    let get_introspection_typename = |lazy_types: Vec<LazyType>| {
        lazy_types.into_iter().map(|lazy| {
            let ty = lazy.ty;
            let enum_name = &lazy.enum_name;
            {
                quote! {
                    #ident::#enum_name(obj) => <#ty as #crate_name::OutputType>::type_name()
                }
            }
        })
    };
    let registry_types = |lazy_types: Vec<LazyType>| {
        lazy_types.into_iter().filter_map(|lazy| {
            let ty = lazy.ty;
            {
                Some(quote! {
                    <#ty as #crate_name::OutputType>::create_type_info(registry);
                })
            }
        })
    };
    let possible_types = |lazy_types: Vec<LazyType>| {
        lazy_types . into_iter () . map (| lazy | { let ty = lazy . ty ; { quote ! {
                possible_types.insert(<#ty as #crate_name::OutputType>::type_name().into_owned());
            } } })
    };
    let expanded = {
        let get_introspection_typename = get_introspection_typename(lazy_types.clone());
        let registry_types = registry_types(lazy_types.clone());
        let possible_types = possible_types(lazy_types.clone());
        quote! {
            #(#type_into_impls)*


            impl #impl_generics #crate_name::resolver_utils::ContainerType for #ident #ty_generics #where_clause {
                async fn resolve_field(&self, ctx: &#crate_name::Context<'_>) -> #crate_name::ServerResult<::std::option::Option<#crate_name::Value>> {
                    ::std::result::Result::Ok(::std::option::Option::None)
                }

                fn collect_all_fields<'__life>(&'__life self, ctx: &#crate_name::ContextSelectionSet<'__life>, fields: &mut #crate_name::resolver_utils::Fields<'__life>) -> #crate_name::ServerResult<()> {
                    match self {
                        #(#collect_all_fields),*
                    }
                }
            }


            impl #impl_generics #crate_name::OutputType for #ident #ty_generics #where_clause {
                fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                    #gql_typename
                }

                fn introspection_type_name(&self) -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                    match self {
                        #(#get_introspection_typename),*
                    }
                }

                fn create_type_info(registry: &mut #crate_name::registry::Registry) -> ::std::string::String {
                    registry.create_output_type::<Self, _>(#crate_name::registry::MetaTypeId::Union, |registry| {
                        #(#registry_types)*

                        #crate_name::registry::MetaType::Union {
                            name: #gql_typename_string,
                            description: #desc,
                            possible_types: {
                                let mut possible_types = #crate_name::indexmap::IndexSet::new();
                                #(#possible_types)*
                                possible_types
                            },


                            rust_typename: ::std::option::Option::Some(::std::any::type_name::<Self>()  ),
                        }
                    })
                }

                async fn resolve(&self, ctx: &#crate_name::ContextSelectionSet<'_>, _field: &#crate_name::Positioned<#crate_name::parser::types::Field>) -> #crate_name::ServerResult<#crate_name::Value> {
                    #crate_name::resolver_utils::resolve_container(ctx, self).await
                }
            }

            impl #impl_generics #crate_name::UnionType for #ident #ty_generics #where_clause {}
        }
    };
    Ok(expanded.into())
}
