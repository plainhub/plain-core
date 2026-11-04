use plain_graphql::{
    ComplexObject, EmptyMutation, EmptySubscription, ID, Interface, MergedObject, Object, Schema,
    SimpleObject, value,
};

#[derive(Default)]
struct First;
#[Object(cache_control(max_age = 60))]
impl First {
    fn duplicate(&self) -> i32 {
        1
    }
    fn first(&self) -> i32 {
        10
    }
}

#[derive(Default)]
struct Last;
#[Object(cache_control(private, max_age = 30))]
impl Last {
    fn duplicate(&self) -> i32 {
        2
    }
    fn last(&self) -> i32 {
        20
    }
}

#[derive(MergedObject, Default)]
struct Merged(First, Last);

#[tokio::test]
async fn merged_registration_preserves_field_order_precedence_and_cache_policy() {
    let schema = Schema::new(Merged::default(), EmptyMutation, EmptySubscription);
    let response = schema.execute("{ alias: duplicate first last }").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data,
        value!({ "alias": 2, "first": 10, "last": 20 })
    );
    assert_eq!(response.cache_control.max_age, 30);
    assert!(!response.cache_control.public);
    let response = schema
        .execute(r#"{ __type(name: "Merged") { fields { name } } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data,
        value!({ "__type": { "fields": [{ "name": "duplicate" }, { "name": "first" }, { "name": "last" }] } })
    );
}

#[derive(SimpleObject)]
#[graphql(complex)]
struct Lean {
    #[graphql(getter = true)]
    id: ID,
    value: i32,
}
impl Lean {
    fn value(&self) -> i32 {
        99
    }
}
#[ComplexObject]
impl Lean {
    fn doubled(&self) -> i32 {
        self.value * 2
    }
}

#[derive(Interface)]
#[graphql(field(name = "id", ty = "&ID"))]
enum Node {
    Lean(Lean),
}

struct NodeQuery;
#[Object]
impl NodeQuery {
    fn node(&self) -> Node {
        let item = Lean {
            id: ID::from("item"),
            value: 7,
        };
        assert_eq!(item.value(), 99);
        item.into()
    }
}

#[tokio::test]
async fn selective_getters_preserve_interfaces_complex_fields_and_fragment_resolution() {
    let schema = Schema::new(NodeQuery, EmptyMutation, EmptySubscription);
    let response = schema
        .execute("{ node { id ... on Lean { value doubled } } }")
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data,
        value!({ "node": { "id": "item", "value": 7, "doubled": 14 } })
    );
}

thread_local! {
    static CACHE_POLICIES: std::cell::Cell<[plain_graphql::CacheControl; 2]> = const {
        std::cell::Cell::new([plain_graphql::CacheControl { public: true, max_age: 0 }; 2])
    };
}

#[derive(plain_graphql::SimpleObject)]
struct PolicyLeaf {
    value: i32,
}

macro_rules! policy_member {
    ($name:ident, $index:expr) => {
        struct $name;
        impl plain_graphql::OutputType for $name {
            fn type_name() -> std::borrow::Cow<'static, str> {
                stringify!($name).into()
            }
            fn create_type_info(registry: &mut plain_graphql::registry::Registry) -> String {
                registry.create_output_type::<Self, _>(
                    plain_graphql::registry::MetaTypeId::Object,
                    |registry| {
                        let plain_graphql::registry::MetaType::Object { fields, .. } =
                            registry.create_fake_output_type::<PolicyLeaf>()
                        else {
                            panic!("policy fixture must be an object")
                        };
                        plain_graphql::registry::ObjectBuilder::new(stringify!($name), fields)
                            .cache_control(CACHE_POLICIES.with(|policies| policies.get()[$index]))
                            .build()
                    },
                )
            }
            async fn resolve(
                &self,
                ctx: &plain_graphql::ContextSelectionSet<'_>,
                _: &plain_graphql::Positioned<plain_graphql::parser::types::Field>,
            ) -> plain_graphql::ServerResult<plain_graphql::Value> {
                plain_graphql::resolver_utils::resolve_container(ctx, self).await
            }
        }
        impl plain_graphql::resolver_utils::ContainerType for $name {
            async fn resolve_field(
                &self,
                ctx: &plain_graphql::Context<'_>,
            ) -> plain_graphql::ServerResult<Option<plain_graphql::Value>> {
                plain_graphql::resolver_utils::ContainerType::resolve_field(
                    &PolicyLeaf { value: $index },
                    ctx,
                )
                .await
            }
        }
        impl plain_graphql::ObjectType for $name {}
    };
}
policy_member!(PolicyLeft, 0);
policy_member!(PolicyRight, 1);

#[derive(plain_graphql::MergedObject)]
struct LocalPolicy(PolicyLeft, PolicyRight);

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachePolicy {
    public: bool,
    max_age: i32,
}
#[derive(serde::Deserialize)]
struct CachePolicyCase {
    left: CachePolicy,
    right: CachePolicy,
    expected: CachePolicy,
}

#[tokio::test]
async fn local_cache_policy_matches_frozen_reference_cases() {
    let cases: Vec<CachePolicyCase> = serde_json::from_str(include_str!(
        "../../../../plain-rs/testdata/graphql/cache-policy.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 196);
    for case in cases {
        CACHE_POLICIES.with(|policies| {
            policies.set([
                plain_graphql::CacheControl {
                    public: case.left.public,
                    max_age: case.left.max_age,
                },
                plain_graphql::CacheControl {
                    public: case.right.public,
                    max_age: case.right.max_age,
                },
            ])
        });
        let local = Schema::new(
            LocalPolicy(PolicyLeft, PolicyRight),
            EmptyMutation,
            EmptySubscription,
        )
        .execute("{ value }")
        .await;
        assert!(local.errors.is_empty(), "{:?}", local.errors);
        assert_eq!(local.data, value!({ "value": 1 }));
        assert_eq!(local.cache_control.public, case.expected.public);
        assert_eq!(local.cache_control.max_age, case.expected.max_age);
    }
}

struct LeftEvents;
#[plain_graphql::Subscription]
impl LeftEvents {
    async fn values(&self) -> impl futures_util::Stream<Item = plain_graphql::Result<i32>> {
        futures_util::stream::iter([Ok(7), Err(plain_graphql::Error::new("stream error"))])
    }
}
struct RightEvents;
#[plain_graphql::Subscription]
impl RightEvents {
    async fn final_value(&self) -> impl futures_util::Stream<Item = i32> {
        futures_util::stream::once(async { 9 })
    }
}
#[derive(plain_graphql::MergedSubscription)]
struct CombinedEvents(LeftEvents, RightEvents);

#[tokio::test]
async fn merged_subscriptions_preserve_selection_aliases_errors_and_member_order() {
    use futures_util::StreamExt;
    let schema = Schema::new(
        First,
        EmptyMutation,
        CombinedEvents(LeftEvents, RightEvents),
    );
    let responses = schema
        .execute_stream("subscription { alias: values }")
        .collect::<Vec<_>>()
        .await;
    assert_eq!(responses.len(), 2);
    assert!(responses[0].errors.is_empty());
    assert_eq!(responses[0].data, value!({ "alias": 7 }));
    assert_eq!(responses[1].errors.len(), 1);
    assert_eq!(responses[1].errors[0].message, "stream error");
    assert_eq!(
        responses[1].errors[0].path,
        vec![plain_graphql::PathSegment::Field("alias".into())]
    );
    let responses = schema
        .execute_stream("subscription { finalValue }")
        .collect::<Vec<_>>()
        .await;
    assert_eq!(responses.len(), 1);
    assert!(responses[0].errors.is_empty());
    assert_eq!(responses[0].data, value!({ "finalValue": 9 }));
    let response = schema
        .execute("{ __type(name: \"CombinedEvents\") { fields { name } } }")
        .await;
    assert!(response.errors.is_empty());
    assert_eq!(
        response.data,
        value!({ "__type": { "fields": [{ "name": "values" }, { "name": "finalValue" }] } })
    );
}
