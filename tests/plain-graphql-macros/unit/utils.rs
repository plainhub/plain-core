use super::*;
use syn::parse::Parser;

#[test]
fn descriptions_preserve_trimmed_lines_empty_descriptions_and_escaping() {
    let attrs = syn::Attribute::parse_outer
        .parse_str(
            r#"
        #[doc = " First line "]
        #[doc = ""]
        #[doc = " quoted \"text\" and \\path "]
    "#,
        )
        .unwrap();
    let description = get_rustdoc(&attrs).unwrap().unwrap();
    assert_eq!(
        syn::parse2::<syn::LitStr>(description).unwrap().value(),
        "First line\n\nquoted \"text\" and \\path"
    );
    assert!(get_rustdoc(&[]).unwrap().is_none());
    let attrs = syn::Attribute::parse_outer
        .parse_str(r##"#[doc = " "]"##)
        .unwrap();
    let description = get_rustdoc(&attrs).unwrap().unwrap();
    assert_eq!(syn::parse2::<syn::LitStr>(description).unwrap().value(), "");
}

#[test]
fn unused_dynamic_description_expressions_are_rejected() {
    let attrs = syn::Attribute::parse_outer
        .parse_str(r##"#[doc = concat!("a", "b")]"##)
        .unwrap();
    assert!(get_rustdoc(&attrs).is_err());
}

#[test]
fn crate_paths_preserve_internal_and_explicit_consumer_paths() {
    let internal = get_crate_path(&None, true);
    assert_eq!(quote!(#internal).to_string(), "crate");
    let standard = get_crate_path(&None, false);
    assert_eq!(quote!(#standard).to_string(), ":: plain_graphql");
    let explicit = get_crate_path(&Some(syn::parse_quote!(::renamed_graphql)), false);
    assert_eq!(quote!(#explicit).to_string(), ":: renamed_graphql");
}


#[test]
fn generator_error_preserves_syn_source_and_compile_error_tokens() {
    use std::error::Error as _;
    let original = syn::parse_str::<syn::Type>("fn(").unwrap_err();
    let expected = original.to_compile_error().to_string();
    let message = original.to_string();
    let error = GeneratorError::from(original);
    assert_eq!(error.to_string(), message);
    assert!(error.source().unwrap().downcast_ref::<syn::Error>().is_some());
    assert_eq!(error.write_errors().to_string(), expected);
}
