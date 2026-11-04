use crate::{
    Context, ContextSelectionSet, ObjectType, OutputType, Positioned, ServerResult, Value,
    model::{__Schema, __Type},
    parser::types::Field,
    registry,
    resolver_utils::{ContainerType, resolve_container},
    schema::IntrospectionMode,
};
use std::borrow::Cow;
pub(crate) struct QueryRoot<T> {
    pub(crate) inner: T,
}
impl<T: ObjectType> ContainerType for QueryRoot<T> {
    async fn resolve_field(&self, ctx: &Context<'_>) -> ServerResult<Option<Value>> {
        if matches!(
            ctx.schema_env.registry.introspection_mode,
            IntrospectionMode::Enabled | IntrospectionMode::IntrospectionOnly
        ) && matches!(
            ctx.query_env.introspection_mode,
            IntrospectionMode::Enabled | IntrospectionMode::IntrospectionOnly,
        ) {
            if ctx.item.node.name.node == "__schema" {
                let mut ctx_obj = ctx.with_selection_set(&ctx.item.node.selection_set);
                ctx_obj.is_for_introspection = true;
                let type_names = ctx.schema_env.registry.introspection_type_names();
                return OutputType::resolve(
                    &__Schema::new(&ctx.schema_env.registry, &type_names),
                    &ctx_obj,
                    ctx.item,
                )
                .await
                .map(Some);
            } else if ctx.item.node.name.node == "__type" {
                let (_, type_name) = ctx.param_value::<String>("name", None)?;
                let mut ctx_obj = ctx.with_selection_set(&ctx.item.node.selection_set);
                ctx_obj.is_for_introspection = true;
                let type_names = ctx.schema_env.registry.introspection_type_names();
                return OutputType::resolve(
                    &ctx.schema_env
                        .registry
                        .types
                        .get(&type_name)
                        .filter(|_| type_names.contains(type_name.as_str()))
                        .map(|ty| __Type::new_simple(&ctx.schema_env.registry, &type_names, ty)),
                    &ctx_obj,
                    ctx.item,
                )
                .await
                .map(Some);
            }
        }
        if ctx.schema_env.registry.introspection_mode == IntrospectionMode::IntrospectionOnly
            || ctx.query_env.introspection_mode == IntrospectionMode::IntrospectionOnly
        {
            return Ok(None);
        }
        self.inner.resolve_field(ctx).await
    }
}
impl<T: ObjectType> OutputType for QueryRoot<T> {
    fn type_name() -> Cow<'static, str> {
        T::type_name()
    }
    fn create_type_info(registry: &mut registry::Registry) -> String {
        let root = T::create_type_info(registry);
        if matches!(
            registry.introspection_mode,
            IntrospectionMode::Enabled | IntrospectionMode::IntrospectionOnly
        ) {
            registry.create_introspection_types();
        }
        root
    }
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        _field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        resolve_container(ctx, self).await
    }
}
impl<T: ObjectType> ObjectType for QueryRoot<T> {}
