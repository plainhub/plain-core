use crate::args::{self, Deprecation};
use darling::FromMeta;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Attribute, Error, Expr, ExprLit, FnArg, Ident, ImplItemFn, Lifetime, Lit, Meta, Pat, PatIdent,
    Type, TypeGroup, TypeParamBound, TypeReference, parse_quote,
    visit_mut::{self, VisitMut},
};
use thiserror::Error;
#[derive(Error, Debug)]
pub enum GeneratorError {
    #[error("{0}")]
    Syn(#[from] syn::Error),
    #[error("{0}")]
    Darling(#[from] darling::Error),
}
impl GeneratorError {
    pub fn write_errors(self) -> TokenStream {
        match self {
            GeneratorError::Syn(err) => err.to_compile_error(),
            GeneratorError::Darling(err) => err.write_errors(),
        }
    }
}
pub type GeneratorResult<T> = std::result::Result<T, GeneratorError>;
pub fn get_crate_path(crate_path: &Option<syn::Path>, internal: bool) -> syn::Path {
    if internal {
        parse_quote! { crate }
    } else if let Some(path) = crate_path {
        path.clone()
    } else {
        let name = match crate_name("plain-graphql") {
            Ok(FoundCrate::Name(name)) => name,
            Ok(FoundCrate::Itself) | Err(_) => "plain_graphql".to_string(),
        };
        let ident = Ident::new(&name, Span::call_site());
        parse_quote! { ::# ident }
    }
}
pub fn generate_guards(crate_name: &syn::Path, expr: &Expr, map_err: TokenStream) -> TokenStream {
    quote! { #crate_name::Guard::check(&(#expr), &ctx).await #map_err ?; }
}
pub fn get_rustdoc(attrs: &[Attribute]) -> GeneratorResult<Option<TokenStream>> {
    let mut lines = Vec::new();
    for attr in attrs {
        if let Meta::NameValue(value) = &attr.meta
            && value.path.is_ident("doc")
        {
            let Expr::Lit(ExprLit {
                lit: Lit::Str(line),
                ..
            }) = &value.value
            else {
                return Err(Error::new_spanned(
                    &value.value,
                    "GraphQL descriptions must use string literals",
                )
                .into());
            };
            lines.push(line.value().trim().to_owned());
        }
    }
    Ok((!lines.is_empty()).then(|| {
        let description = lines.join("\n").trim().to_owned();
        quote ! (#description)
    }))
}
fn generate_default_value(lit: &Lit) -> GeneratorResult<TokenStream> {
    match lit { Lit :: Str (value) => { let value = value . value () ; Ok (quote ! ({ ::std::borrow::ToOwned::to_owned(#value) })) } Lit :: Int (value) => { let value = value . base10_parse :: < i32 > () ? ; Ok (quote ! ({ ::std::convert::TryInto::try_into(#value).unwrap_or_default() })) } Lit :: Float (value) => { let value = value . base10_parse :: < f64 > () ? ; Ok (quote ! ({ ::std::convert::TryInto::try_into(#value).unwrap_or_default() })) } Lit :: Bool (value) => { let value = value . value ; Ok (quote ! ({ #value })) } _ => Err (Error :: new_spanned (lit , "The default value type only be string, integer, float and boolean, other types are not supported" ,) . into ()) , }
}
pub fn generate_default(
    default: &Option<args::DefaultValue>,
) -> GeneratorResult<Option<TokenStream>> {
    match default {
        Some(args::DefaultValue::Default) => {
            Ok(Some(quote! { ::std::default::Default::default() }))
        }
        Some(args::DefaultValue::Value(lit)) => Ok(Some(generate_default_value(lit)?)),
        None => Ok(None),
    }
}
pub fn get_cfg_attrs(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| !attr.path().segments.is_empty() && attr.path().segments[0].ident == "cfg")
        .cloned()
        .collect()
}
pub fn parse_graphql_attrs<T: FromMeta + Default>(
    attrs: &[Attribute],
) -> GeneratorResult<Option<T>> {
    for attr in attrs {
        if attr.path().is_ident("graphql") {
            return Ok(Some(T::from_meta(&attr.meta)?));
        }
    }
    Ok(None)
}
pub fn remove_graphql_attrs(attrs: &mut Vec<Attribute>) {
    if let Some((idx, _)) = attrs
        .iter()
        .enumerate()
        .find(|(_, a)| a.path().is_ident("graphql"))
    {
        attrs.remove(idx);
    }
}
pub fn get_type_path_and_name(ty: &Type) -> GeneratorResult<(&Type, String)> {
    match ty {
        Type::Path(path) => Ok((
            ty,
            path.path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap(),
        )),
        Type::Group(TypeGroup { elem, .. }) => get_type_path_and_name(elem),
        Type::TraitObject(trait_object) => Ok((
            ty,
            trait_object
                .bounds
                .iter()
                .find_map(|bound| match bound {
                    TypeParamBound::Trait(t) => {
                        Some(t.path.segments.last().map(|s| s.ident.to_string()).unwrap())
                    }
                    _ => None,
                })
                .unwrap(),
        )),
        _ => Err(Error::new_spanned(ty, "Invalid type").into()),
    }
}
pub fn gen_deprecation(deprecation: &Deprecation, crate_name: &syn::Path) -> TokenStream {
    match deprecation {
        Deprecation::NoDeprecated => {
            quote! { #crate_name::registry::Deprecation::NoDeprecated }
        }
        Deprecation::Deprecated {
            reason: Some(reason),
        } => {
            quote! { #crate_name::registry::Deprecation::Deprecated { reason: ::std::option::Option::Some(::std::string::ToString::to_string(#reason)) } }
        }
        Deprecation::Deprecated { reason: None } => {
            quote! { #crate_name::registry::Deprecation::Deprecated { reason: ::std::option::Option::None } }
        }
    }
}
pub fn extract_input_args<T: FromMeta + Default>(
    crate_name: &syn::Path,
    method: &mut ImplItemFn,
) -> GeneratorResult<Vec<(PatIdent, Type, T)>> {
    let mut args = Vec::new();
    let mut create_ctx = true;
    if method.sig.inputs.is_empty() {
        return Err(Error::new_spanned(
            &method.sig,
            "The self receiver must be the first parameter.",
        )
        .into());
    }
    for (idx, arg) in method.sig.inputs.iter_mut().enumerate() {
        if let FnArg::Receiver(receiver) = arg {
            if idx != 0 {
                return Err(Error::new_spanned(
                    receiver,
                    "The self receiver must be the first parameter.",
                )
                .into());
            }
        } else if let FnArg::Typed(pat) = arg {
            if idx == 0 {
                return Err(Error::new_spanned(
                    pat,
                    "The self receiver must be the first parameter.",
                )
                .into());
            }
            match (&*pat.pat, &*pat.ty) {
                (Pat::Ident(arg_ident), Type::Reference(TypeReference { elem, .. })) => {
                    if let Type::Path(path) = elem.as_ref() {
                        if idx != 1 || path.path.segments.last().unwrap().ident != "Context" {
                            args.push((
                                arg_ident.clone(),
                                pat.ty.as_ref().clone(),
                                parse_graphql_attrs::<T>(&pat.attrs)?.unwrap_or_default(),
                            ));
                        } else {
                            create_ctx = false;
                        }
                    }
                }
                (Pat::Ident(arg_ident), ty) => {
                    args.push((
                        arg_ident.clone(),
                        ty.clone(),
                        parse_graphql_attrs::<T>(&pat.attrs)?.unwrap_or_default(),
                    ));
                    remove_graphql_attrs(&mut pat.attrs);
                }
                _ => {
                    return Err(Error::new_spanned(arg, "Invalid argument type.").into());
                }
            }
        }
    }
    if create_ctx {
        let arg = syn::parse2::<FnArg>(quote! { _: &#crate_name::Context<'_> }).unwrap();
        method.sig.inputs.insert(1, arg);
    }
    Ok(args)
}
pub struct RemoveLifetime;
impl VisitMut for RemoveLifetime {
    fn visit_lifetime_mut(&mut self, i: &mut Lifetime) {
        i.ident = Ident::new("_", Span::call_site());
        visit_mut::visit_lifetime_mut(self, i);
    }
}
#[cfg(test)]
#[path = "../../tests/plain-graphql-macros/unit/utils.rs"]
mod tests;
