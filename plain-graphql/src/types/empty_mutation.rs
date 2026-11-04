use crate::{
    Context, ContextSelectionSet, ObjectType, OutputType, Positioned, ServerError, ServerResult,
    Value, parser::types::Field, registry, registry::MetaTypeId, resolver_utils::ContainerType,
};
use std::borrow::Cow;
#[doc = " Empty mutation"]
#[doc = ""]
#[doc = " Only the parameters used to construct the Schema, representing an"]
#[doc = " unconfigured mutation."]
#[doc = ""]
#[doc = " # Examples"]
#[doc = ""]
#[doc = " ```rust"]
#[doc = " use plain_graphql::*;"]
#[doc = ""]
#[doc = " struct Query;"]
#[doc = ""]
#[doc = " #[Object]"]
#[doc = " impl Query {"]
#[doc = "     async fn value(&self) -> i32 {"]
#[doc = "         // A GraphQL Object type must define one or more fields."]
#[doc = "         100"]
#[doc = "     }"]
#[doc = " }"]
#[doc = ""]
#[doc = " let schema = Schema::new(Query, EmptyMutation, EmptySubscription);"]
#[doc = " ```"]
#[derive(Default, Copy, Clone)]
pub struct EmptyMutation;
impl ContainerType for EmptyMutation {
    fn is_empty() -> bool {
        true
    }
    async fn resolve_field(&self, _ctx: &Context<'_>) -> ServerResult<Option<Value>> {
        Ok(None)
    }
}
impl OutputType for EmptyMutation {
    fn type_name() -> Cow<'static, str> {
        Cow::Borrowed("EmptyMutation")
    }
    fn create_type_info(registry: &mut registry::Registry) -> String {
        registry.create_output_type::<Self, _>(MetaTypeId::Object, |_| registry::MetaType::Object {
            name: "EmptyMutation".to_string(),
            description: None,
            fields: Default::default(),
            cache_control: Default::default(),

            is_subscription: false,
            rust_typename: Some(std::any::type_name::<Self>()),
        })
    }
    async fn resolve(
        &self,
        _ctx: &ContextSelectionSet<'_>,
        _field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        Err(ServerError::new(
            "Schema is not configured for mutations.",
            None,
        ))
    }
}
impl ObjectType for EmptyMutation {}
