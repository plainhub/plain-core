use super::*;

#[test]
fn retained_attributes_validate_duplicates_and_parse_crate_paths() {
    let variant: Variant = parse_quote!(
        #[strum(disabled)]
        Hidden
    );
    assert!(disabled(&variant).unwrap());
    let duplicate: Variant = parse_quote!(
        #[strum(disabled, disabled)]
        Hidden
    );
    assert!(disabled(&duplicate).is_err());
    let unknown: Variant = parse_quote!(
        #[strum(unknown)]
        Hidden
    );
    assert!(disabled(&unknown).is_err());
    let ast: DeriveInput = parse_quote!(
        #[strum(crate = "::custom::strum")]
        enum Value {
            A,
        }
    );
    assert_eq!(
        crate_module_path(&ast).unwrap(),
        parse_quote!(::custom::strum)
    );
    let duplicate: DeriveInput = parse_quote!(
        #[strum(crate = "strum", crate = "strum")]
        enum Value {
            A,
        }
    );
    assert!(crate_module_path(&duplicate).is_err());
}
