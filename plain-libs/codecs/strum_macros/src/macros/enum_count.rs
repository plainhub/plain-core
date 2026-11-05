use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput};

use crate::helpers::{crate_module_path, disabled, non_enum_error};

pub(crate) fn enum_count_inner(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let n = match &ast.data {
        Data::Enum(v) => v.variants.iter().try_fold(0usize, |acc, v| {
            if !disabled(v)? {
                Ok::<usize, syn::Error>(acc + 1usize)
            } else {
                Ok::<usize, syn::Error>(acc)
            }
        })?,
        _ => return Err(non_enum_error()),
    };
    let strum_module_path = crate_module_path(ast)?;

    // Used in the quasi-quotation below as `#name`
    let name = &ast.ident;

    // Helper is provided for handling complex generic types correctly and effortlessly
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    Ok(quote! {
        // Implementation
        impl #impl_generics #strum_module_path::EnumCount for #name #ty_generics #where_clause {
            const COUNT: usize = #n;
        }
    })
}
