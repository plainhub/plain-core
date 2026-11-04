use super::*;
use crate::{model::__DirectiveLocation, registry::MetaDirective};
#[test]
fn test_escape_string() {
    assert_eq!(
        escape_string("1\\\x08d\x0c3\n4\r5\t6"),
        "1\\\\\\bd\\f3\\n4\\r5\\t6"
    );
}
#[test]
fn test_type_directive_sdl_without_federation() {
    let expected = r#"directive @custom_type_directive(optionalWithoutDefault: String, optionalWithDefault: String = "DEFAULT") on FIELD_DEFINITION | OBJECT
schema {
	query: Query
}
"#;
    let mut registry = Registry::default();
    registry.add_directive(MetaDirective {
        name: "custom_type_directive".to_string(),
        description: None,
        locations: vec![
            __DirectiveLocation::FieldDefinition,
            __DirectiveLocation::Object,
        ],
        args: [
            (
                "optionalWithoutDefault".to_string(),
                MetaInputValue {
                    name: "optionalWithoutDefault".to_string(),
                    description: None,
                    ty: "String".to_string(),
                    deprecation: Deprecation::NoDeprecated,
                    default_value: None,
                },
            ),
            (
                "optionalWithDefault".to_string(),
                MetaInputValue {
                    name: "optionalWithDefault".to_string(),
                    description: None,
                    ty: "String".to_string(),
                    deprecation: Deprecation::NoDeprecated,
                    default_value: Some("\"DEFAULT\"".to_string()),
                },
            ),
        ]
        .into(),
        is_repeatable: false,
    });
    registry.query_type = "Query".to_string();
    let sdl = registry.export_sdl();
    assert_eq!(sdl, expected)
}
