use crate::{
    args::{self, RenameTarget, SubscriptionField},
    output_type::OutputType,
    utils::{
        GeneratorResult, extract_input_args, gen_deprecation, generate_default, generate_guards,
        get_cfg_attrs, get_crate_path, get_rustdoc, get_type_path_and_name, parse_graphql_attrs,
        remove_graphql_attrs,
    },
};
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Block, Error, ImplItem, ItemImpl, ReturnType, Type, TypeImplTrait, TypeParamBound,
    ext::IdentExt,
};
pub fn generate(
    subscription_args: &args::Subscription,
    item_impl: &mut ItemImpl,
) -> GeneratorResult<TokenStream> {
    let crate_name = get_crate_path(&subscription_args.crate_path, subscription_args.internal);
    let (self_ty, self_name) = get_type_path_and_name(item_impl.self_ty.as_ref())?;
    let generics = &item_impl.generics;
    let where_clause = &item_impl.generics.where_clause;
    let gql_typename = {
        let name = subscription_args
            .name
            .clone()
            .unwrap_or_else(|| RenameTarget::Type.rename(self_name.clone()));
        quote ! (::std::borrow::Cow::Borrowed(#name))
    };
    let gql_typename_string = {
        let name = subscription_args
            .name
            .clone()
            .unwrap_or_else(|| RenameTarget::Type.rename(self_name.clone()));
        quote ! (::std::string::ToString::to_string(#name))
    };
    let desc = {
        get_rustdoc(&item_impl.attrs)?
            .map(|s| quote ! (::std::option::Option::Some(::std::string::ToString::to_string(#s))))
            .unwrap_or_else(|| quote!(::std::option::Option::None))
    };
    let mut create_stream = Vec::new();
    let mut schema_fields = Vec::new();
    for item in &mut item_impl.items {
        if let ImplItem::Fn(method) = item {
            let field: SubscriptionField = parse_graphql_attrs(&method.attrs)?.unwrap_or_default();
            if field.skip {
                remove_graphql_attrs(&mut method.attrs);
                continue;
            }
            let ident = method.sig.ident.clone();
            let field_name = field.name.clone().unwrap_or_else(|| {
                RenameTarget::Field.rename(method.sig.ident.unraw().to_string())
            });
            let field_desc_value = get_rustdoc(&method.attrs)?;
            let has_field_desc = field_desc_value.is_some();
            let field_desc = field_desc_value . map (| s | quote ! {::std::option::Option::Some(::std::string::ToString::to_string(#s))}) . unwrap_or_else (| | quote ! { ::std::option::Option::None }) ;
            let field_deprecation = gen_deprecation(&field.deprecation, &crate_name);
            let has_deprecation = !matches!(field.deprecation, args::Deprecation::NoDeprecated);
            let cfg_attrs = get_cfg_attrs(&method.attrs);
            if method.sig.asyncness.is_none() {
                return Err(Error::new_spanned(
                    method,
                    "The subscription stream function must be asynchronous",
                )
                .into());
            }
            let mut schema_args = Vec::new();
            let mut use_params = Vec::new();
            let mut get_params = Vec::new();
            let args = extract_input_args::<args::SubscriptionFieldArgument>(&crate_name, method)?;
            for (
                ident,
                ty,
                args::SubscriptionFieldArgument {
                    name,
                    desc,
                    default,
                    deprecation,
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
            let ty = match &method.sig.output {
                ReturnType::Type(_, ty) => OutputType::parse(ty)?,
                ReturnType::Default => {
                    return Err(Error::new_spanned(
                        &method.sig.output,
                        "Resolver must have a return type",
                    )
                    .into());
                }
            };
            let res_ty = ty.value_type();
            let stream_ty = if let Type::ImplTrait(TypeImplTrait { bounds, .. }) = &res_ty {
                let mut r = None;
                for b in bounds {
                    if let TypeParamBound::Trait(b) = b {
                        r = Some(quote! { dyn #b });
                    }
                }
                quote! { #r }
            } else {
                quote! { #res_ty }
            };
            if let OutputType::Value(inner_ty) = &ty {
                let block = &method.block;
                let new_block = quote ! ({
                    {
                        let value = (move || { async move #block })().await;
                        ::std::result::Result::Ok(value)
                    }
                });
                method.block = syn::parse2::<Block>(new_block).expect("invalid block");
                method.sig.output =
                    syn::parse2::<ReturnType>(quote! { -> #crate_name::Result<#inner_ty> })
                        .expect("invalid result type");
            }
            let mut field_sets = Vec::new();
            if has_field_desc {
                field_sets.push(quote ! (field.description = #field_desc;));
            }
            if has_deprecation {
                field_sets.push(quote ! (field.deprecation = #field_deprecation;));
            }
            schema_fields . push (quote ! {
                #(#cfg_attrs)*
                {
                    let mut field = #crate_name::registry::MetaField::new(
                        ::std::string::ToString::to_string(#field_name),
                        <<#stream_ty as #crate_name::futures_util::stream::Stream>::Item as #crate_name::OutputType>::create_type_info(registry),
                    );
                    #(#schema_args)*
                    #(#field_sets)*
                    fields.insert(::std::string::ToString::to_string(#field_name), field);
                }
            }) ;
            let create_field_stream = quote! {
                self.#ident(ctx, #(#use_params),*)
                    .await
                    .map_err(|err| {
                        ::std::convert::Into::<#crate_name::Error>::into(err).into_server_error(ctx.item.pos)
                            .with_path(::std::vec![#crate_name::PathSegment::Field(::std::borrow::ToOwned::to_owned(&*field_name))])
                    })
            };
            let guard_map_err = quote! {
                .map_err(|err| {
                    err.into_server_error(ctx.item.pos)
                        .with_path(::std::vec![#crate_name::PathSegment::Field(::std::borrow::ToOwned::to_owned(&*field_name))])
                })
            };
            let guard = match field.guard.as_ref().or(subscription_args.guard.as_ref()) {
                Some(code) => Some(generate_guards(&crate_name, code, guard_map_err)),
                None => None,
            };
            let stream_fn = quote! {
                let field_name =   ctx.item.node.response_key().node . clone ( );
                let field = ::std::sync::Arc::new(  ctx.item . clone ( ));

                let  stream  = async {
                    #(#get_params)*
                    #guard
                    #create_field_stream
                }  .await.map_err(|err| ctx.set_error_path(err))?;

                let  schema_env  = ctx. schema_env .  clone(  );
                let query_env =   ctx.query_env . clone ( );
                let stream = #crate_name::futures_util::stream::StreamExt::then(stream,
                    move |msg| {
                        let schema_env =  schema_env . clone(  );
                        let query_env =  query_env . clone(  );
                        let field =  field . clone(  );
                        let field_name =  field_name . clone(  );
                                async move {
                                    let ctx_selection_set = query_env.create_context(
                                        &schema_env,
                                          Some(#crate_name::QueryPathNode {
                                            parent  :None,
                                            segment: #crate_name::QueryPathSegment::Name(&field_name)
                                        }),
                                        &field.node.selection_set,
                                    );

                                    let  mut response  =  match
                                        #crate_name::OutputType::resolve(&msg, &ctx_selection_set, &  field)
                                            .await
                                             { Ok ( value )
                                       =>  {
                                        let mut map = #crate_name::indexmap::IndexMap::new();
                                        map.insert(  field_name  , value  );
                                        #crate_name::Response::new(#crate_name::Value::Object(map))
                                    } Err (error )
                                     =>  #crate_name::Response::from_errors(  vec![ error ]) , } ;

                                     response .errors.extend(::std::mem::take(&mut *query_env.errors.lock().unwrap()));
                                      Ok( response )
                    }
                });
                #crate_name::ServerResult::Ok(stream)
            };
            create_stream . push (quote ! {
                #(#cfg_attrs)*
                if ctx.item.node.name.node == #field_name {
                    let stream = #crate_name::futures_util::stream::TryStreamExt::try_flatten(
                        #crate_name::futures_util::stream::once((move || async move { #stream_fn })())
                    );
                    let stream = #crate_name::futures_util::StreamExt::map(stream, |res| match res {
                        ::std::result::Result::Ok(resp) => resp,
                        ::std::result::Result::Err(err) => #crate_name::Response::from_errors(::std::vec![err]),
                    });
                    return ::std::option::Option::Some(::std::boxed::Box::pin(stream));
                }
            }) ;
            remove_graphql_attrs(&mut method.attrs);
        }
    }
    if create_stream.is_empty() {
        return Err(Error::new_spanned(
            self_ty,
            "A GraphQL Object type must define one or more fields.",
        )
        .into());
    }
    let field_count = schema_fields.len();
    let expanded = quote! {
        #item_impl


        impl #generics #crate_name::SubscriptionType for #self_ty #where_clause {
            fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                #gql_typename
            }


            fn create_type_info(registry: &mut #crate_name::registry::Registry) -> ::std::string::String {
                registry.create_subscription_type::<Self, _>(|registry| #crate_name::registry::MetaType::Object {
                    name: #gql_typename_string,
                    description: #desc,
                    fields: {
                        let mut fields = #crate_name::indexmap::IndexMap::with_capacity(#field_count);
                        #(#schema_fields)*
                        fields
                    },
                    cache_control: ::std::default::Default::default(),


                    is_subscription: true,
                    rust_typename: ::std::option::Option::Some(::std::any::type_name::<Self>())  ,
                })
            }

            fn create_field_stream<'__life>(
                &'__life self,
                ctx: &'__life #crate_name::Context<'_>,
            ) -> ::std::option::Option<::std::pin::Pin<::std::boxed::Box<dyn #crate_name::futures_util::stream::Stream<Item = #crate_name::Response> + ::std::marker::Send + '__life>>> {
                #(#create_stream)*
                ::std::option::Option::None
            }
        }
    };
    Ok(expanded.into())
}
