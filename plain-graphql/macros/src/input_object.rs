use crate::{
    args::{self, RenameTarget},
    utils::{GeneratorResult, gen_deprecation, generate_default, get_crate_path, get_rustdoc},
};
use darling::ast::Data;
use proc_macro::TokenStream;
use quote::quote;
use syn::{Error, ext::IdentExt};
pub fn generate(object_args: &args::InputObject) -> GeneratorResult<TokenStream> {
    let crate_name = get_crate_path(&object_args.crate_path, object_args.internal);
    let (impl_generics, ty_generics, where_clause) = object_args.generics.split_for_impl();
    let ident = &object_args.ident;
    let s = match &object_args.data {
        Data::Struct(s) => s,
        _ => {
            return Err(
                Error::new_spanned(ident, "InputObject can only be applied to an struct.").into(),
            );
        }
    };
    for field in &s.fields {
        if field.ident.is_none() {
            return Err(Error::new_spanned(ident, "All fields must be named.").into());
        }
    }
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
    let desc = get_rustdoc(&object_args.attrs)?;
    let has_desc = desc.is_some();
    let desc = desc
        .map(|s| quote! { ::std::option::Option::Some(::std::string::ToString::to_string(#s)) })
        .unwrap_or_else(|| quote! {::std::option::Option::None});
    let mut get_fields = Vec::new();
    let mut put_fields = Vec::new();
    let mut fields = Vec::new();
    let mut schema_fields = Vec::new();
    for field in &s.fields {
        let ident = field.ident.as_ref().unwrap();
        let ty = &field.ty;
        let name = field
            .name
            .clone()
            .unwrap_or_else(|| RenameTarget::Field.rename(ident.unraw().to_string()));
        if field.skip {
            get_fields.push(quote! {
                let #ident: #ty = ::std::default::Default::default();
            });
            fields.push(ident);
            continue;
        }
        let desc = get_rustdoc(&field.attrs)?;
        let has_desc = desc.is_some();
        let default = generate_default(&field.default)?;
        let schema_default = default.as_ref().map(|value| {
            quote! {
                ::std::option::Option::Some(::std::string::ToString::to_string(
                    &<#ty as #crate_name::InputType>::to_value(&#value)
                ))
            }
        });
        if let Some(default) = default {
            get_fields . push (quote ! { let # ident : # ty = { match obj . get (# name) { :: std :: option :: Option :: Some (value) => { let # ident = # crate_name :: InputType :: parse (:: std :: option :: Option :: Some (:: std :: clone :: Clone :: clone (& value))) . map_err (# crate_name :: InputValueError :: propagate) ?;  # ident } , :: std :: option :: Option :: None => # default , } } ; }) ;
        } else {
            get_fields . push (quote ! { let # ident : # ty = # crate_name :: InputType :: parse (obj . get (# name) . cloned ()) . map_err (# crate_name :: InputValueError :: propagate) ?;  }) ;
        }
        put_fields.push(quote! {
            map.insert(
                #crate_name::Name::new(#name),
                #crate_name::InputType::to_value(&self.#ident)
            );
        });
        fields.push(ident);
        let has_deprecation = !matches!(field.deprecation, args::Deprecation::NoDeprecated);
        let deprecation_expr = gen_deprecation(&field.deprecation, &crate_name);
        let mut input_sets = Vec::new();
        if has_desc {
            let desc = desc.as_ref().expect("checked desc");
            input_sets . push (quote ! {
                input_value.description = ::std::option::Option::Some(::std::string::ToString::to_string(#desc));
            }) ;
        }
        if let Some(schema_default) = schema_default {
            input_sets.push(quote ! (input_value.default_value = #schema_default;));
        }
        if has_deprecation {
            input_sets.push(quote ! (input_value.deprecation = #deprecation_expr;));
        }
        schema_fields.push(quote! {
            {
                let mut input_value = #crate_name::registry::MetaInputValue::new(
                    ::std::string::ToString::to_string(#name),
                    <#ty as #crate_name::InputType>::create_type_info(registry),
                );
                #(#input_sets)*
                fields.insert(::std::borrow::ToOwned::to_owned(#name), input_value);
            }
        })
    }
    if get_fields.is_empty() {
        return Err(Error::new_spanned(
            ident,
            "A GraphQL Input Object type must define one or more input fields.",
        )
        .into());
    }
    let mut input_object_builder = Vec::new();
    input_object_builder.push(quote ! (.rust_typename(::std::any::type_name::<Self>())));
    if has_desc {
        input_object_builder.push(quote ! (.description(#desc)));
    }
    let expanded = {
        quote! {

            impl #impl_generics #crate_name::InputType for #ident #ty_generics #where_clause {
                type RawValueType = Self;

                fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                    #gql_typename
                }

                fn create_type_info(registry: &mut #crate_name::registry::Registry) -> ::std::string::String {
                    registry.create_input_type::<Self, _>(#crate_name::registry::MetaTypeId::InputObject, |registry| {
                        #crate_name::registry::InputObjectBuilder::new(
                            #gql_typename_string,
                            {
                                let mut fields = #crate_name::indexmap::IndexMap::new();
                                #(#schema_fields)*
                                fields
                            },
                        )
                        #(#input_object_builder)*
                        .build()
                    })
                }

                fn parse(value: ::std::option::Option<#crate_name::Value>) -> #crate_name::InputValueResult<Self> {
                    if let ::std::option::Option::Some(#crate_name::Value::Object(obj)) = value {
                        #(#get_fields)*
                        let obj = Self { #(#fields),* };

                        ::std::result::Result::Ok(obj)
                    } else {
                        ::std::result::Result::Err(#crate_name::InputValueError::expected_type(value.unwrap_or_default()))
                    }
                }

                fn to_value(&self) -> #crate_name::Value {
                    let mut map = #crate_name::indexmap::IndexMap::new();
                    #(#put_fields)*
                    #crate_name::Value::Object(map)
                }

                fn    as_raw_value(&self) -> ::std::option::Option<&Self::RawValueType> {
                    ::std::option::Option::Some(self)
                }
            }

            impl #impl_generics #crate_name::InputObjectType for #ident #ty_generics #where_clause {}
        }
    };
    Ok(expanded.into())
}
