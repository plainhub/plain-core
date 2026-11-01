use async_graphql::{
    Context, ContextSelectionSet, ObjectType, OutputType, Positioned, ServerResult, Value,
    parser::types::Field,
    registry::{self, MetaTypeId},
    resolver_utils::{ContainerType, resolve_container},
};
use std::borrow::Cow;

// The published mobile peer schema has an empty Query object.
pub(crate) struct Query;
impl ContainerType for Query {
    async fn resolve_field(&self, _: &Context<'_>) -> ServerResult<Option<Value>> {
        Ok(None)
    }
}
impl OutputType for Query {
    fn type_name() -> Cow<'static, str> {
        Cow::Borrowed("Query")
    }
    fn create_type_info(registry: &mut registry::Registry) -> String {
        registry.create_output_type::<Self, _>(MetaTypeId::Object, |_| {
            registry::ObjectBuilder::new("Query", Default::default())
                .rust_typename(std::any::type_name::<Self>())
                .build()
        })
    }
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        _: &Positioned<Field>,
    ) -> ServerResult<Value> {
        resolve_container(ctx, self).await
    }
}
impl ObjectType for Query {}
