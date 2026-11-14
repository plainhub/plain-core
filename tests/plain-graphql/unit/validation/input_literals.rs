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
        json: Option<Json<plain_libs::serde_json::Value>>,
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
    ).variables(plain_graphql::Variables::from_json(plain_libs::serde_json::json!({
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

mod oneof {
    use crate::{EmptySubscription, InputObject, InputType, Object, Request, Schema, Variables};
    use plain_libs::serde_json::json;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(InputObject)]
    #[graphql(oneof)]
    struct Choice {
        number: Option<i32>,
        text: Option<String>,
    }

    #[derive(InputObject)]
    struct Batch {
        choices: Vec<Choice>,
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
        async fn before(&self) -> bool {
            self.0.fetch_add(1, Ordering::SeqCst);
            true
        }
        async fn choose(&self, choice: Choice) -> bool {
            self.0.fetch_add(1, Ordering::SeqCst);
            choice.number.is_some() || choice.text.is_some()
        }
        async fn batch(&self, input: Batch) -> bool {
            self.0.fetch_add(1, Ordering::SeqCst);
            !input.choices.is_empty()
        }
    }
    fn schema() -> (Schema<Query, Mutation, EmptySubscription>, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        (
            Schema::build(Query, Mutation(calls.clone()), EmptySubscription).finish(),
            calls,
        )
    }

    #[tokio::test]
    async fn derived_oneof_exports_its_contract_and_omits_unselected_fields() {
        let (schema, _) = schema();
        assert!(schema.sdl().contains("input Choice @oneOf {"));
        assert!(schema.sdl().contains("directive @oneOf on INPUT_OBJECT"));
        let response = schema
            .execute("{__type(name:\"Choice\"){isOneOf inputFields{name defaultValue type{kind}}}}")
            .await;
        assert!(response.errors.is_empty());
        let data = response.data.into_json().unwrap();
        assert_eq!(data["__type"]["isOneOf"], true);
        for field in data["__type"]["inputFields"].as_array().unwrap() {
            assert!(field["defaultValue"].is_null());
            assert_ne!(field["type"]["kind"], "NON_NULL");
        }
        assert_eq!(
            Choice {
                number: Some(7),
                text: None
            }
            .to_value()
            .into_json()
            .unwrap(),
            json!({"number":7})
        );
        assert_eq!(
            Choice {
                number: None,
                text: Some("ok".into())
            }
            .to_value()
            .into_json()
            .unwrap(),
            json!({"text":"ok"})
        );
    }

    #[tokio::test]
    async fn invalid_oneof_values_reject_the_whole_request_before_side_effects() {
        let (schema, calls) = schema();
        for input in [
            json!({}),
            json!({"number":1,"text":"ok"}),
            json!({"number":null}),
            json!({"text":null}),
            json!({"number":1,"text":null}),
            json!({"number":null,"text":null}),
            json!(null),
        ] {
            let response = schema
                .execute(
                    Request::new("mutation($choice:Choice!){before choose(choice:$choice)}")
                        .variables(Variables::from_json(json!({"choice":input}))),
                )
                .await;
            assert!(!response.errors.is_empty(), "{input}");
            assert!(
                response.errors.iter().all(|e| e.path.is_empty()),
                "{input}: {:?}",
                response.errors
            );
            assert_eq!(calls.load(Ordering::SeqCst), 0, "{input}");
        }
        for query in [
            "mutation{before choose(choice:{})}",
            "mutation{before choose(choice:{number:1,text:\"ok\"})}",
            "mutation{before choose(choice:{number:null})}",
            "mutation($number:Int! = 1,$text:String! = \"ok\"){before choose(choice:{number:$number,text:$text})}",
            "mutation{before batch(input:{choices:[{number:1},{text:null}]})}",
            "mutation($choice:Choice! = {}){before choose(choice:$choice)}",
            "mutation($text:String){before choose(choice:{text:$text})}",
            "mutation($text:String = \"ok\"){before choose(choice:{text:$text})}",
            "mutation($text:String){before ...Selection} fragment Selection on Mutation{choose(choice:{text:$text})}",
        ] {
            let response = schema
                .execute(Request::new(query).variables(Variables::from_json(json!({"text":"ok"}))))
                .await;
            assert!(!response.errors.is_empty(), "{query}");
            assert_eq!(
                calls.load(Ordering::SeqCst),
                0,
                "{query}: {:?}",
                response.errors
            );
        }
    }

    #[tokio::test]
    async fn both_oneof_branches_and_non_null_variables_execute_normally() {
        let (schema, calls) = schema();
        for request in [
            Request::new("mutation{choose(choice:{number:7})}"),
            Request::new("mutation{choose(choice:{text:\"ok\"})}"),
            Request::new("mutation($choice:Choice!){choose(choice:$choice)}")
                .variables(Variables::from_json(json!({"choice":{"text":"ok"}}))),
            Request::new("mutation($text:String!){choose(choice:{text:$text})}")
                .variables(Variables::from_json(json!({"text":"ok"}))),
            Request::new("mutation($text:String! = \"ok\"){choose(choice:{text:$text})}"),
            Request::new("mutation($choice:Choice! = {number:7}){choose(choice:$choice)}"),
            Request::new("mutation{batch(input:{choices:[{number:7},{text:\"ok\"}]})}"),
        ] {
            let before = calls.load(Ordering::SeqCst);
            let response = schema.execute(request).await;
            assert!(response.errors.is_empty(), "{:?}", response.errors);
            assert_eq!(calls.load(Ordering::SeqCst), before + 1);
        }
    }

    #[test]
    fn oneof_schema_rejects_required_fields_and_field_defaults() {
        use crate::registry::{InputObjectBuilder, MetaInputValue};
        for (ty, default) in [("Int!", None), ("Int", Some("7".into()))] {
            let mut field = MetaInputValue::new("number", ty);
            field.default_value = default;
            let fields = [("number".into(), field)].into_iter().collect();
            assert!(
                std::panic::catch_unwind(|| InputObjectBuilder::new("Invalid", fields)
                    .oneof(true)
                    .build())
                .is_err()
            );
        }
    }
}
