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
