use crate::{
    EmptyMutation, EmptySubscription, Enum, InputObject, Interface, Object, Schema, SimpleObject,
    Union,
};

#[derive(SimpleObject)]
#[graphql(internal)]
struct ReachableA {
    #[graphql(getter = true)]
    id: i32,
}
#[derive(SimpleObject)]
#[graphql(internal)]
struct ReachableB {
    #[graphql(getter = true)]
    id: i32,
}
#[derive(Interface)]
#[graphql(internal, field(name = "id", ty = "&i32"))]
enum ReachableInterface {
    A(ReachableA),
    B(ReachableB),
}
#[derive(Interface)]
#[graphql(internal, field(name = "id", ty = "&i32"))]
enum BacklinkInterface {
    A(ReachableA),
    B(BacklinkOnlyNode),
}
#[derive(SimpleObject)]
#[graphql(internal)]
struct BacklinkOnlyNode {
    #[graphql(getter = true)]
    id: i32,
    detail: String,
}
#[derive(Union)]
#[graphql(internal)]
enum ReachableUnion {
    A(ReachableA),
    B(ReachableB),
}
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(internal)]
enum ReachableChoice {
    First,
    Second,
}
#[derive(InputObject)]
#[graphql(internal)]
struct ReachableInput {
    choice: ReachableChoice,
    nested: Option<Box<ReachableInput>>,
}
#[derive(SimpleObject)]
#[graphql(internal)]
struct UnreachableType {
    unused: bool,
}
struct ReachabilityQuery;
#[Object(internal)]
impl ReachabilityQuery {
    fn interface(&self) -> ReachableInterface {
        ReachableA { id: 1 }.into()
    }
    fn union(&self, filter: Option<ReachableInput>) -> ReachableUnion {
        let _ = filter;
        ReachableB { id: 2 }.into()
    }
}

#[tokio::test]
async fn introspection_preserves_reachable_inputs_abstract_types_and_system_types() {
    let schema = Schema::build(ReachabilityQuery, EmptyMutation, EmptySubscription)
        .register_output_type::<UnreachableType>()
        .register_output_type::<BacklinkInterface>()
        .finish();
    let response = schema.execute(r#"{
        __schema {
            types {
                name kind description
                fields { name description args { name defaultValue type { name kind ofType { name kind } } } type { name kind ofType { name kind } } }
                inputFields { name defaultValue type { name kind ofType { name kind } } }
                possibleTypes { name }
                interfaces { name }
                enumValues { name }
            }
            directives { name locations args { name type { name kind } } }
        }
        orphan: __type(name: "UnreachableType") { name }
        backlink: __type(name: "BacklinkInterface") { name possibleTypes { name } }
    }"#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let expected: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(include_str!(
        "../../../../plain-rs/testdata/graphql/introspection.json"
    ))
    .unwrap();
    assert_eq!(plain_libs::serde_json::to_value(&response.data).unwrap(), expected);
}
