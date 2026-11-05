use proc_macro2::Span;
use syn::{parse_quote, DeriveInput, LitStr, Path, Variant};

pub fn non_enum_error() -> syn::Error {
    syn::Error::new(Span::call_site(), "This macro only supports enums.")
}

pub fn disabled(variant: &Variant) -> syn::Result<bool> {
    let mut disabled = false;
    for attr in variant.attrs.iter().filter(|a| a.path().is_ident("strum")) {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("disabled") {
                return Err(meta.error("Unsupported variant attribute"));
            }
            if disabled {
                return Err(meta.error("Duplicate disabled attribute"));
            }
            disabled = true;
            Ok(())
        })?;
    }
    Ok(disabled)
}

pub fn crate_module_path(ast: &DeriveInput) -> syn::Result<Path> {
    let mut path = None;
    for attr in ast.attrs.iter().filter(|a| a.path().is_ident("strum")) {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("crate") {
                return Err(meta.error("Unsupported enum attribute"));
            }
            if path.is_some() {
                return Err(meta.error("Duplicate crate attribute"));
            }
            let value: LitStr = meta.value()?.parse()?;
            path = Some(value.parse::<Path>()?);
            Ok(())
        })?;
    }
    Ok(path.unwrap_or_else(|| parse_quote!(::strum)))
}

#[cfg(test)]
#[path = "../../tests/unit/src/helpers.rs"]
mod tests;
