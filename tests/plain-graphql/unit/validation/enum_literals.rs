use super::*;
use plain_graphql::{EmptySubscription, Enum, ID, InputObject, Json, Object, Schema};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
enum Choice {
    First,
    Second,
}

#[derive(InputObject)]
struct Nested {
    choice: Choice,
}

#[derive(InputObject)]
struct Filter {
    choices: Vec<Choice>,
    nested: Option<Nested>,
}

struct Query;

#[Object]
impl Query {
    async fn ready(&self) -> bool {
        true
    }
}

struct Mutation(Arc<AtomicUsize>);

#[Object]
impl Mutation {
    async fn record(
        &self,
        choice: Option<Choice>,
        filter: Option<Filter>,
        matrix: Option<Vec<Vec<Choice>>>,
        text: Option<String>,
        id: Option<ID>,
        json: Option<Json<serde_json::Value>>,
    ) -> bool {
        let _ = (choice, filter, matrix, text, id, json);
        self.0.fetch_add(1, Ordering::SeqCst);
        true
    }
}

fn schema() -> (Schema<Query, Mutation, EmptySubscription>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let schema = Schema::build(Query, Mutation(calls.clone()), EmptySubscription).finish();
    (schema, calls)
}

#[tokio::test]
async fn quoted_enum_literals_are_rejected_without_any_resolver_side_effects() {
    let (schema, calls) = schema();
    for query in [
        r#"mutation { first: record second: record(choice: "FIRST") }"#,
        r#"mutation { record(filter: {choices: [FIRST, "SECOND"]}) }"#,
        r#"mutation { record(filter: {choices: "FIRST"}) }"#,
        r#"mutation { record(filter: {choices: [], nested: {choice: "FIRST"}}) }"#,
        r#"mutation { record(matrix: [[FIRST], ["SECOND"]]) }"#,
        r#"mutation { ... on Mutation { record(choice: "FIRST") } }"#,
        r#"mutation { ...Bad } fragment Bad on Mutation { record(choice: "FIRST") }"#,
        r#"mutation { ...Bad } fragment Bad on Mutation { ...Other } fragment Other on Mutation { record(choice: "FIRST") }"#,
        r#"mutation { record(choice: "FIRST") @skip(if: true) }"#,
        r#"mutation($choice: Choice = "FIRST") { record(choice: $choice) }"#,
        r#"mutation($filter: Filter = {choices: ["FIRST"]}) { record(filter: $filter) }"#,
    ] {
        let response = schema.execute(query).await;
        assert!(!response.errors.is_empty(), "{query}: {response:?}");
        assert_eq!(response.data, plain_graphql::Value::Null, "{query}");
        assert!(!response.errors[0].locations.is_empty(), "{query}");
        assert_eq!(calls.load(Ordering::SeqCst), 0, "{query}");
    }
}

#[tokio::test]
async fn enum_variables_tokens_defaults_and_unrelated_string_scalars_remain_valid() {
    let (schema, calls) = schema();
    for query in [
        "mutation { record(choice: FIRST) }",
        "mutation { record(filter: {choices: [FIRST, SECOND], nested: {choice: SECOND}}) }",
        "mutation { record(filter: {choices: FIRST}, matrix: FIRST) }",
        "mutation($choice: Choice = FIRST) { record(choice: $choice) }",
        "mutation { ...Good } fragment Good on Mutation { record(choice: FIRST) }",
        r#"mutation { record(text: "FIRST", id: "SECOND", json: {choice: "FIRST", choices: ["SECOND"]}) }"#,
    ] {
        let before = calls.load(Ordering::SeqCst);
        let response = schema.execute(query).await;
        assert!(response.errors.is_empty(), "{query}: {:?}", response.errors);
        assert_eq!(calls.load(Ordering::SeqCst), before + 1, "{query}");
    }
    let response = schema.execute(plain_graphql::Request::new(
        "mutation($choice: Choice, $filter: Filter, $matrix: [[Choice!]!]) { record(choice: $choice, filter: $filter, matrix: $matrix) }",
    ).variables(plain_graphql::Variables::from_json(serde_json::json!({
        "choice": "FIRST", "filter": {"choices": ["FIRST", "SECOND"], "nested": {"choice": "SECOND"}}, "matrix": [["FIRST"], ["SECOND"]],
    })))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(calls.load(Ordering::SeqCst), 7);
}

#[tokio::test]
async fn invalid_defaults_in_other_operations_and_invalid_unused_fragments_still_fail_validation() {
    let (schema, calls) = schema();
    for query in [
        r#"mutation Selected { record } mutation Other($choice: Choice = "FIRST") { record(choice: $choice) }"#,
        r#"mutation Selected { record } fragment Unused on Mutation { record(choice: "FIRST") }"#,
        "mutation Selected { record(choice: UNKNOWN) }",
        "mutation Selected { ...Cycle } fragment Cycle on Mutation { ...Cycle }",
    ] {
        let response = schema
            .execute(plain_graphql::Request::new(query).operation_name("Selected"))
            .await;
        assert!(!response.errors.is_empty(), "{query}: {response:?}");
        assert_eq!(calls.load(Ordering::SeqCst), 0, "{query}");
    }
}

#[test]
fn enum_literals_in_directive_arguments_are_checked_without_extension_hooks() {
    use plain_graphql::{
        InputType,
        registry::{__DirectiveLocation, MetaDirective, MetaInputValue},
    };
    let mut registry = Registry::default();
    Choice::create_type_info(&mut registry);
    registry.add_directive(MetaDirective {
        name: "choose".into(),
        description: None,
        locations: vec![__DirectiveLocation::Field],
        args: [("choice".into(), MetaInputValue::new("choice", "Choice"))].into(),
        is_repeatable: false,
    });
    let quoted =
        plain_graphql::parser::parse_query(r#"{ ready @choose(choice: "FIRST") }"#).unwrap();
    let error = validate(&registry, &quoted).unwrap_err();
    assert_eq!(
        error.message,
        "Enum \"Choice\" cannot represent a string literal."
    );
    assert!(!error.locations.is_empty());
    let token = plain_graphql::parser::parse_query("{ ready @choose(choice: FIRST) }").unwrap();
    assert!(validate(&registry, &token).is_ok());
}
