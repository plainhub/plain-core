use crate::{
    args::{self, RenameTarget},
    output_type::OutputType,
    utils::{
        GeneratorResult, extract_input_args, gen_deprecation, generate_default, generate_guards,
        get_cfg_attrs, get_crate_path, get_rustdoc, get_type_path_and_name, parse_graphql_attrs,
        remove_graphql_attrs,
    },
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Block, Error, ImplItem, ItemImpl, PatIdent, ReturnType, Type, ext::IdentExt};
pub fn generate(
    object_args: &args::Object,
    item_impl: &mut ItemImpl,
) -> GeneratorResult<TokenStream> {
    let crate_name = get_crate_path(&object_args.crate_path, object_args.internal);
    let (self_ty, self_name) = get_type_path_and_name(item_impl.self_ty.as_ref())?;
    let (impl_generics, _, where_clause) = item_impl.generics.split_for_impl();
    let gql_typename = {
        object_args
            .name
            .as_ref()
            .map(|name| quote ! (::std::borrow::Cow::Borrowed(#name)))
            .unwrap_or_else(|| {
                let name = RenameTarget::Type.rename(self_name.clone());
                quote ! (::std::borrow::Cow::Borrowed(#name))
            })
    };
    let gql_typename_string = {
        let name = object_args
            .name
            .as_ref()
            .cloned()
            .unwrap_or_else(|| RenameTarget::Type.rename(self_name.clone()));
        quote ! (::std::string::ToString::to_string(#name))
    };
    let rustdoc = get_rustdoc(&item_impl.attrs)?;
    let has_desc = rustdoc.is_some();
    let desc = {
        rustdoc
            .map(|s| quote ! (::std::option::Option::Some(::std::string::ToString::to_string(#s))))
            .unwrap_or_else(|| quote!(::std::option::Option::None))
    };
    let mut resolvers = Vec::new();
    let mut schema_fields = Vec::new();
    for item in &mut item_impl.items {
        if let ImplItem::Fn(method) = item {
            let method_args: args::ObjectField =
                parse_graphql_attrs(&method.attrs)?.unwrap_or_default();
            if !method_args.skip {
                let is_async = method.sig.asyncness.is_some();
                let cfg_attrs = get_cfg_attrs(&method.attrs);
                let field_name = method_args.name.clone().unwrap_or_else(|| {
                    RenameTarget::Field.rename(method.sig.ident.unraw().to_string())
                });
                let field_desc_value = get_rustdoc(&method.attrs)?;
                let has_field_desc = field_desc_value.is_some();
                let field_desc = field_desc_value . map (| s | quote ! { ::std::option::Option::Some(::std::string::ToString::to_string(#s)) }) . unwrap_or_else (| | quote ! { ::std::option::Option::None }) ;
                let field_deprecation = gen_deprecation(&method_args.deprecation, &crate_name);
                let cache_control = {
                    let public = method_args.cache_control.is_public();
                    let max_age = if method_args.cache_control.no_cache {
                        -1
                    } else {
                        method_args.cache_control.max_age as i32
                    };
                    quote! {
                        #crate_name::CacheControl {
                            public: #public,
                            max_age: #max_age,
                        }
                    }
                };
                let has_cache_control = method_args.cache_control.no_cache
                    || method_args.cache_control.max_age != 0
                    || !method_args.cache_control.is_public();
                let has_deprecation =
                    !matches!(method_args.deprecation, args::Deprecation::NoDeprecated);
                let args = extract_input_args::<args::Argument>(&crate_name, method)?;
                let mut schema_args = Vec::new();
                let mut params = Vec::new();
                for (
                    ident,
                    ty,
                    args::Argument {
                        name,
                        desc,
                        default,
                        deprecation,
                        ..
                    },
                ) in &args
                {
                    let name = name.clone().unwrap_or_else(|| {
                        RenameTarget::Argument.rename(ident.ident.unraw().to_string())
                    });
                    let has_desc = desc.is_some();
                    let default = generate_default(default)?;
                    let schema_default = default.as_ref().map(|value| {
                        quote! {
                            ::std::option::Option::Some(::std::string::ToString::to_string(
                                &<#ty as #crate_name::InputType>::to_value(&#value)
                            ))
                        }
                    });
                    let has_deprecation = !matches!(deprecation, args::Deprecation::NoDeprecated);
                    let deprecation_expr = gen_deprecation(deprecation, &crate_name);
                    let mut arg_sets = Vec::new();
                    if has_desc {
                        let desc = desc.as_ref().expect("checked desc");
                        arg_sets . push (quote ! {
                            arg.description = ::std::option::Option::Some(::std::string::ToString::to_string(#desc));
                        }) ;
                    }
                    if let Some(schema_default) = schema_default {
                        arg_sets.push(quote ! (arg.default_value = #schema_default;));
                    }
                    if has_deprecation {
                        arg_sets.push(quote ! (arg.deprecation = #deprecation_expr;));
                    }
                    schema_args.push(quote! {
                        {
                            let mut arg = #crate_name::registry::MetaInputValue::new(
                                ::std::string::ToString::to_string(#name),
                                <#ty as #crate_name::InputType>::create_type_info(registry),
                            );
                            #(#arg_sets)*
                            field.args.insert(::std::string::ToString::to_string(#name), arg);
                        }
                    });
                    params.push(FieldResolverParameter {
                        ty,
                        name,
                        default,
                        ident: ident.clone(),
                    });
                }
                let output = method.sig.output.clone();
                let ty = match &output {
                    ReturnType::Type(_, ty) => OutputType::parse(ty)?,
                    ReturnType::Default => {
                        return Err(Error::new_spanned(
                            &output,
                            "Resolver must have a return type",
                        )
                        .into());
                    }
                };
                let schema_ty = ty.value_type();
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
                    #(#cfg_attrs)*
                    {
                        let mut field = #crate_name::registry::MetaField::new(
                            ::std::string::ToString::to_string(#field_name),
                            <#schema_ty as #crate_name::OutputType>::create_type_info(registry),
                        );
                        #(#schema_args)*
                        #(#field_sets)*
                        fields.insert(::std::string::ToString::to_string(#field_name), field);
                    }
                });
                let field_ident = &method.sig.ident;
                if is_async && let OutputType::Value(inner_ty) = &ty {
                    let block = &method.block;
                    let new_block = quote ! ({
                        {
                            ::std::result::Result::Ok(async move {
                                let value:#inner_ty = #block;
                                value
                            }.await)
                        }
                    });
                    method.block = syn::parse2::<Block>(new_block).expect("invalid block");
                    method.sig.output =
                        syn::parse2::<ReturnType>(quote! { -> #crate_name::Result<#inner_ty> })
                            .expect("invalid result type");
                }
                let extract_params = params
                    .iter()
                    .map(generate_parameter_extraction)
                    .collect::<Vec<_>>();
                let use_params = params.iter().map(
                    |FieldResolverParameter {
                         ident: PatIdent { ident, .. },
                         ..
                     }| ident,
                );
                let guard_map_err = quote! {
                    .map_err(|err| ctx.set_error_path(err.into_server_error(ctx.item.pos)))
                };
                let guard = match method_args.guard.as_ref().or(object_args.guard.as_ref()) {
                    Some(code) => generate_guards(&crate_name, code, guard_map_err),
                    None => Default::default(),
                };
                let resolve_body = if is_async {
                    quote! {
                        return #crate_name::resolver_utils::resolve_field_async(
                            ctx,
                            self.#field_ident(ctx, #(#use_params),*)
                        )
                        .await;
                    }
                } else {
                    match &ty {
                        OutputType::Value(_) => {
                            quote! {
                                let obj: #schema_ty = self.#field_ident(ctx, #(#use_params),*);
                                return #crate_name::resolver_utils::resolve_simple_field_value(ctx, &obj).await;
                            }
                        }
                        OutputType::Result(_) => {
                            quote! {
                                let obj: #schema_ty = self.#field_ident(ctx, #(#use_params),*)
                                    .map_err(|err| {
                                        let err = ::std::convert::Into::<#crate_name::Error>::into(err)
                                            .into_server_error(ctx.item.pos);
                                        ctx.set_error_path(err)
                                    })?;
                                return #crate_name::resolver_utils::resolve_simple_field_value(ctx, &obj).await;
                            }
                        }
                    }
                };
                resolvers.push(quote! {
                    #(#cfg_attrs)*
                    #field_name => {
                        #(#extract_params)*
                        #guard
                        #resolve_body
                    }
                });
            }
            remove_graphql_attrs(&mut method.attrs);
        }
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
    if resolvers.is_empty() {
        return Err(Error::new_spanned(
            self_ty,
            "A GraphQL Object type must define one or more fields.",
        )
        .into());
    }
    let resolve_container = {
        quote! { #crate_name::resolver_utils::resolve_container(ctx, self).await }
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
    let resolve_field_resolver_match = generate_field_match(resolvers)?;
    let expanded = {
        quote! {
            #item_impl

            #[doc(hidden)]

            const _: () = {


                    impl #impl_generics #crate_name::resolver_utils::ContainerType for #self_ty #where_clause {
                    async fn resolve_field(&self, ctx: &#crate_name::Context<'_>) -> #crate_name::ServerResult<::std::option::Option<#crate_name::Value>> {
                        #resolve_field_resolver_match

                        ::std::result::Result::Ok(::std::option::Option::None)
                    }


                }

                    impl #impl_generics #crate_name::OutputType for #self_ty #where_clause {
                    fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                        #gql_typename
                    }

                    fn create_type_info(registry: &mut #crate_name::registry::Registry) -> ::std::string::String {
                        let ty = registry.create_output_type::<Self, _>(#crate_name::registry::MetaTypeId::Object, |registry| {
                            #crate_name::registry::ObjectBuilder::new(
                                #gql_typename_string,
                                {
                                    let mut fields = #crate_name::indexmap::IndexMap::with_capacity(#field_count);
                                    #(#schema_fields)*
                                    fields
                                },
                            )
                            #(#object_builder)*
                            .build()
                        });


                        ty
                    }

                    async fn resolve(
                        &self,
                        ctx: &#crate_name::ContextSelectionSet<'_>,
                        _field: &#crate_name::Positioned<#crate_name::parser::types::Field>
                    ) -> #crate_name::ServerResult<#crate_name::Value> {
                        #resolve_container
                    }
                }

                impl #impl_generics #crate_name::ObjectType for #self_ty #where_clause {}
            };
        }
    };
    Ok(expanded)
}
fn generate_field_match(
    resolvers: Vec<proc_macro2::TokenStream>,
) -> GeneratorResult<proc_macro2::TokenStream> {
    if resolvers.is_empty() {
        return Ok(quote!());
    }
    Ok(quote! {
        match ctx.item.node.name.node.as_str() {
            #(#resolvers)*
            _ => {}
        }
    })
}
fn generate_parameter_extraction(parameter: &FieldResolverParameter) -> proc_macro2::TokenStream {
    let FieldResolverParameter {
        ty,
        name,
        default,
        ident,
    } = parameter;
    let default = match default {
        Some(default) => quote! { ::std::option::Option::Some(|| -> #ty { #default }) },
        None => quote! { ::std::option::Option::None },
    };
    let ident = &ident.ident;
    quote! {
        let (_, #ident) = ctx.param_value::<#ty>(#name, #default)
            .map_err(|err| ctx.set_error_path(err))?;
    }
}
struct FieldResolverParameter<'a> {
    ty: &'a Type,
    ident: PatIdent,
    name: String,
    default: Option<proc_macro2::TokenStream>,
}
#[cfg(test)]
#[path = "../../../tests/plain-graphql-macros/unit/object.rs"]
mod tests;
