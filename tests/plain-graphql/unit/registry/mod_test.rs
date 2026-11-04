use crate::registry::{__DirectiveLocation, MetaDirective};
#[test]
fn test_repeatable_directive_dsl() {
    let expected = r#"directive @testDirective repeatable on OBJECT | INTERFACE"#;
    assert_eq!(
        expected.to_string(),
        MetaDirective {
            name: "testDirective".to_string(),
            description: None,
            locations: vec![__DirectiveLocation::Object, __DirectiveLocation::Interface,],
            args: Default::default(),
            is_repeatable: true,
        }
        .sdl()
    )
}
#[test]
fn test_non_repeatable_directive_dsl() {
    let expected = r#"directive @testDirective on OBJECT | INTERFACE"#;
    assert_eq!(
        expected.to_string(),
        MetaDirective {
            name: "testDirective".to_string(),
            description: None,
            locations: vec![__DirectiveLocation::Object, __DirectiveLocation::Interface,],
            args: Default::default(),
            is_repeatable: false,
        }
        .sdl()
    )
}
