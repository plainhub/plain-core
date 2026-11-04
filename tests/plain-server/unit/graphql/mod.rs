use async_graphql::{
    ComplexObject, EmptyMutation, EmptySubscription, ID, Interface, Schema, value,
};
use plain_graphql_macros::{MergedObject, SimpleObject, object as Object};

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
#[graphql(getters = false, complex)]
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

#[derive(SimpleObject)]
struct Product {
    id: i32,
}

#[derive(Default)]
struct Entities;
#[Object]
impl Entities {
    #[graphql(entity)]
    async fn product(&self, id: i32) -> Product {
        Product { id }
    }
}

#[derive(MergedObject, Default)]
struct Federated(Entities, First);

#[tokio::test]
async fn trimming_empty_entity_methods_preserves_real_entity_lookup_and_input_errors() {
    let schema = Schema::new(Federated::default(), EmptyMutation, EmptySubscription);
    let response = schema.execute(r#"{ _entities(representations: [{__typename: "Product", id: 42}]) { ... on Product { id } } }"#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data, value!({ "_entities": [{ "id": 42 }] }));
    for representation in [
        r#"{__typename: "Product"}"#,
        r#"{__typename: "Product", id: "invalid"}"#,
        "{id: 42}",
    ] {
        let response = schema
            .execute(format!(
                "{{ _entities(representations: [{representation}]) {{ ... on Product {{ id }} }} }}"
            ))
            .await;
        assert!(
            !response.errors.is_empty(),
            "invalid representation accepted: {representation}"
        );
    }
}

struct GenericEntities<T>(std::marker::PhantomData<T>);

#[Object(concrete(name = "IntEntities", params(i32)))]
impl<T: Send + Sync> GenericEntities<T> {
    #[graphql(entity)]
    async fn product(&self, id: i32) -> Product {
        Product { id }
    }
}

#[tokio::test]
async fn generic_entity_objects_keep_concrete_lookup_delegation() {
    let schema = Schema::new(
        GenericEntities::<i32>(std::marker::PhantomData),
        EmptyMutation,
        EmptySubscription,
    );
    let response = schema.execute(r#"{ _entities(representations: [{__typename: "Product", id: 42}]) { ... on Product { id } } }"#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data, value!({ "_entities": [{ "id": 42 }] }));
    let response = schema.execute(r#"{ _entities(representations: [{__typename: "Product"}]) { ... on Product { id } } }"#).await;
    assert!(!response.errors.is_empty());
}

thread_local! {
    static CACHE_POLICIES: std::cell::Cell<[async_graphql::CacheControl; 2]> = const {
        std::cell::Cell::new([async_graphql::CacheControl { public: true, max_age: 0 }; 2])
    };
}

#[derive(async_graphql::SimpleObject)]
struct PolicyLeaf {
    value: i32,
}

macro_rules! policy_member {
    ($name:ident, $index:expr) => {
        struct $name;
        impl async_graphql::OutputType for $name {
            fn type_name() -> std::borrow::Cow<'static, str> {
                stringify!($name).into()
            }
            fn create_type_info(registry: &mut async_graphql::registry::Registry) -> String {
                registry.create_output_type::<Self, _>(
                    async_graphql::registry::MetaTypeId::Object,
                    |registry| {
                        let async_graphql::registry::MetaType::Object { fields, .. } =
                            registry.create_fake_output_type::<PolicyLeaf>()
                        else {
                            panic!("policy fixture must be an object")
                        };
                        async_graphql::registry::ObjectBuilder::new(
                            stringify!($name),
                            fields,
                        )
                        .cache_control(CACHE_POLICIES.with(|policies| policies.get()[$index]))
                        .build()
                    },
                )
            }
            async fn resolve(
                &self,
                ctx: &async_graphql::ContextSelectionSet<'_>,
                _: &async_graphql::Positioned<async_graphql::parser::types::Field>,
            ) -> async_graphql::ServerResult<async_graphql::Value> {
                async_graphql::resolver_utils::resolve_container(ctx, self).await
            }
        }
        impl async_graphql::resolver_utils::ContainerType for $name {
            async fn resolve_field(
                &self,
                ctx: &async_graphql::Context<'_>,
            ) -> async_graphql::ServerResult<Option<async_graphql::Value>> {
                async_graphql::resolver_utils::ContainerType::resolve_field(
                    &PolicyLeaf { value: $index },
                    ctx,
                )
                .await
            }
        }
        impl async_graphql::ObjectType for $name {}
    };
}
policy_member!(PolicyLeft, 0);
policy_member!(PolicyRight, 1);

#[derive(plain_graphql_macros::MergedObject)]
struct LocalPolicy(PolicyLeft, PolicyRight);
#[derive(async_graphql::MergedObject)]
struct ReferencePolicy(PolicyLeft, PolicyRight);

#[tokio::test]
async fn local_cache_policy_matches_the_official_runtime_for_defaults_limits_and_privacy() {
    for left_public in [false, true] {
        for right_public in [false, true] {
            for left_age in [i32::MIN, -2, -1, 0, 1, 30, i32::MAX] {
                for right_age in [i32::MIN, -2, -1, 0, 1, 30, i32::MAX] {
                    CACHE_POLICIES.with(|policies| {
                        policies.set([
                            async_graphql::CacheControl {
                                public: left_public,
                                max_age: left_age,
                            },
                            async_graphql::CacheControl {
                                public: right_public,
                                max_age: right_age,
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
                    let reference = Schema::new(
                        ReferencePolicy(PolicyLeft, PolicyRight),
                        EmptyMutation,
                        EmptySubscription,
                    )
                    .execute("{ value }")
                    .await;
                    assert!(local.errors.is_empty(), "{:?}", local.errors);
                    assert!(reference.errors.is_empty(), "{:?}", reference.errors);
                    assert_eq!(local.data, reference.data);
                    assert_eq!(
                        local.cache_control, reference.cache_control,
                        "ages={left_age}/{right_age}, public={left_public}/{right_public}"
                    );
                }
            }
        }
    }
}
