use crate::{
    args::{self, RenameTarget},
    output_type::OutputType,
    utils::{
        GeneratorResult, extract_input_args, gen_deprecation, generate_default, generate_guards,
        get_cfg_attrs, get_crate_path, get_rustdoc, get_type_path_and_name, parse_graphql_attrs,
        remove_graphql_attrs,
    },
};
use proc_macro::TokenStream;
use quote::quote;
use syn::{Block, Error, ImplItem, ItemImpl, ReturnType, ext::IdentExt};
pub fn generate(
    object_args: &args::ComplexObject,
    item_impl: &mut ItemImpl,
) -> GeneratorResult<TokenStream> {
    let crate_name = get_crate_path(&object_args.crate_path, object_args.internal);
    let (self_ty, _) = get_type_path_and_name(item_impl.self_ty.as_ref())?;
    let generics = &item_impl.generics;
    let where_clause = &item_impl.generics.where_clause;
    let mut resolvers = Vec::new();
    let mut schema_fields = Vec::new();
    for item in &mut item_impl.items {
        if let ImplItem::Fn(method) = item {
            let method_args: args::ComplexObjectField =
                parse_graphql_attrs(&method.attrs)?.unwrap_or_default();
            if method_args.skip {
                remove_graphql_attrs(&mut method.attrs);
                continue;
            }
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
            let mut use_params = Vec::new();
            let mut get_params = Vec::new();
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
                let param_ident = &ident.ident;
                use_params.push(quote! { #param_ident });
                let default = match default {
                    Some(default) => {
                        quote! { ::std::option::Option::Some(|| -> #ty { #default }) }
                    }
                    None => quote! { ::std::option::Option::None },
                };
                get_params.push(quote! {
                    let (_, #param_ident) = ctx.param_value::<#ty>(#name, #default)?;
                });
            }
            let output = method.sig.output.clone();
            let ty = match &output {
                ReturnType::Type(_, ty) => OutputType::parse(ty)?,
                ReturnType::Default => {
                    return Err(
                        Error::new_spanned(&output, "Resolver must have a return type").into(),
                    );
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
                    fields.push((::std::string::ToString::to_string(#field_name), field));
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
            let resolve_obj = if is_async {
                quote! {
                    {
                        let res = self.#field_ident(ctx, #(#use_params),*).await;
                        res.map_err(|err| ::std::convert::Into::<#crate_name::Error>::into(err).into_server_error(ctx.item.pos))
                    }
                }
            } else {
                match &ty {
                    OutputType::Value(_) => {
                        quote! {
                            ::std::result::Result::Ok(self.#field_ident(ctx, #(#use_params),*))
                        }
                    }
                    OutputType::Result(_) => {
                        quote! {
                            self.#field_ident(ctx, #(#use_params),*)
                                .map_err(|err| {
                                    ::std::convert::Into::<#crate_name::Error>::into(err)
                                        .into_server_error(ctx.item.pos)
                                })
                        }
                    }
                }
            };
            let guard_map_err = quote! {
                .map_err(|err| err.into_server_error(ctx.item.pos))
            };
            let guard = match method_args.guard.as_ref().or(object_args.guard.as_ref()) {
                Some(code) => Some(generate_guards(&crate_name, code, guard_map_err)),
                None => None,
            };
            let resolve_block = if is_async {
                quote! {
                    let f = async move {
                        #(#get_params)*
                        #guard
                        #resolve_obj
                    };
                    let obj = f.await.map_err(|err| ctx.set_error_path(err))?;
                    let ctx_obj = ctx.with_selection_set(&ctx.item.node.selection_set);
                    return #crate_name::OutputType::resolve(&obj, &ctx_obj, ctx.item)
                        .await
                        .map(::std::option::Option::Some);
                }
            } else {
                quote! {
                    #(#get_params)*
                    #guard
                    let obj = #resolve_obj.map_err(|err| ctx.set_error_path(err))?;
                    return #crate_name::resolver_utils::resolve_simple_field_value(ctx, &obj).await;
                }
            };
            resolvers.push(quote! {
                #(#cfg_attrs)*
                if ctx.item.node.name.node == #field_name {
                    #resolve_block
                }
            });
            remove_graphql_attrs(&mut method.attrs);
        }
    }
    let expanded = quote! {
        #item_impl


        impl #generics #crate_name::ComplexObject for #self_ty #where_clause {
            fn fields(registry: &mut #crate_name::registry::Registry) -> ::std::vec::Vec<(::std::string::String, #crate_name::registry::MetaField)> {
                let mut fields = ::std::vec::Vec::new();
                #(#schema_fields)*
                fields
            }

            async fn resolve_field(&self, ctx: &#crate_name::Context<'_>) -> #crate_name::ServerResult<::std::option::Option<#crate_name::Value>> {
                #(#resolvers)*
                ::std::result::Result::Ok(::std::option::Option::None)
            }
        }
    };
    Ok(expanded.into())
}
