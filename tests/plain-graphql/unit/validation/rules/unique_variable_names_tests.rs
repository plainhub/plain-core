use super::*;
pub fn factory<'a>() -> UniqueVariableNames<'a> {
    UniqueVariableNames::default()
}
#[test]
fn unique_variable_names() {
    expect_passes_rule!(
        factory,
        r#"
          query A($x: Int, $y: String) { __typename }
          query B($x: String, $y: Int) { __typename }
        "#,
    );
}
#[test]
fn duplicate_variable_names() {
    expect_fails_rule!(
        factory,
        r#"
          query A($x: Int, $x: Int, $x: String) { __typename }
          query B($x: String, $x: Int) { __typename }
          query C($x: Int, $x: Int) { __typename }
        "#,
    );
}
