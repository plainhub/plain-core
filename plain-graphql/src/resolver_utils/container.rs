use crate::{
    Context, ContextSelectionSet, Error, Name, OutputType, ServerError, ServerResult, Value,
    parser::types::Selection,
};

use plain_libs::indexmap::IndexMap;
use std::{future::Future, pin::Pin, sync::Arc};
pub trait ContainerType: OutputType {
    #[doc(hidden)]
    fn is_empty() -> bool {
        false
    }
    fn resolve_field(
        &self,
        ctx: &Context<'_>,
    ) -> impl Future<Output = ServerResult<Option<Value>>> + Send;
    fn collect_all_fields<'a>(
        &'a self,
        ctx: &ContextSelectionSet<'a>,
        fields: &mut Fields<'a>,
    ) -> ServerResult<()>
    where
        Self: Send + Sync,
    {
        fields.add_set(ctx, self)
    }
}
impl<T: ContainerType + ?Sized> ContainerType for &T {
    async fn resolve_field(&self, ctx: &Context<'_>) -> ServerResult<Option<Value>> {
        T::resolve_field(*self, ctx).await
    }
}
impl<T: ContainerType + ?Sized> ContainerType for Arc<T> {
    async fn resolve_field(&self, ctx: &Context<'_>) -> ServerResult<Option<Value>> {
        T::resolve_field(self, ctx).await
    }
}
impl<T: ContainerType + ?Sized> ContainerType for Box<T> {
    async fn resolve_field(&self, ctx: &Context<'_>) -> ServerResult<Option<Value>> {
        T::resolve_field(self, ctx).await
    }
}
impl<T: ContainerType, E: Into<Error> + Send + Sync + Clone> ContainerType for Result<T, E> {
    async fn resolve_field(&self, ctx: &Context<'_>) -> ServerResult<Option<Value>> {
        match self {
            Ok(value) => T::resolve_field(value, ctx).await,
            Err(err) => Err(ctx.set_error_path(err.clone().into().into_server_error(ctx.item.pos))),
        }
    }
}
pub async fn resolve_container<'a, T: ContainerType + ?Sized>(
    ctx: &ContextSelectionSet<'a>,
    root: &'a T,
) -> ServerResult<Value> {
    resolve_container_inner(ctx, root, true).await
}
pub async fn resolve_container_serial<'a, T: ContainerType + ?Sized>(
    ctx: &ContextSelectionSet<'a>,
    root: &'a T,
) -> ServerResult<Value> {
    resolve_container_inner(ctx, root, false).await
}
pub(crate) fn create_value_object(values: Vec<(Name, Value)>) -> Value {
    let mut map = IndexMap::new();
    for (name, value) in values {
        insert_value(&mut map, name, value);
    }
    Value::Object(map)
}
fn insert_value(target: &mut IndexMap<Name, Value>, name: Name, value: Value) {
    if let Some(prev_value) = target.get_mut(&name) {
        if let Value::Object(target_map) = prev_value {
            if let Value::Object(obj) = value {
                for (key, value) in obj.into_iter() {
                    insert_value(target_map, key, value);
                }
            }
        } else if let Value::List(target_list) = prev_value
            && let Value::List(list) = value
        {
            for (idx, value) in list.into_iter().enumerate() {
                if let Some(Value::Object(target_map)) = target_list.get_mut(idx)
                    && let Value::Object(obj) = value
                {
                    for (key, value) in obj.into_iter() {
                        insert_value(target_map, key, value);
                    }
                }
            }
        }
    } else {
        target.insert(name, value);
    }
}
async fn resolve_container_inner<'a, T: ContainerType + ?Sized>(
    ctx: &ContextSelectionSet<'a>,
    root: &'a T,
    parallel: bool,
) -> ServerResult<Value> {
    let mut fields = Fields(Vec::new());
    fields.add_set(ctx, root)?;
    let res = if parallel {
        futures_util::future::try_join_all(fields.0).await?
    } else {
        let mut results = Vec::with_capacity(fields.0.len());
        for field in fields.0 {
            results.push(field.await?);
        }
        results
    };
    Ok(create_value_object(res))
}
type BoxFieldFuture<'a> = Pin<Box<dyn Future<Output = ServerResult<(Name, Value)>> + 'a + Send>>;
pub struct Fields<'a>(Vec<BoxFieldFuture<'a>>);
impl<'a> Fields<'a> {
    pub fn add_set<T: ContainerType + ?Sized>(
        &mut self,
        ctx: &ContextSelectionSet<'a>,
        root: &'a T,
    ) -> ServerResult<()> {
        for selection in &ctx.item.node.items {
            match &selection.node {
                Selection::Field(field) => {
                    if field.node.name.node == "__typename" {
                        let ctx_field = ctx.with_field(field);
                        let field_name = ctx_field.item.node.response_key().node.clone();
                        let typename = root.introspection_type_name().into_owned();
                        self.0.push(Box::pin(async move {
                            Ok((field_name, Value::String(typename)))
                        }));
                        continue;
                    }
                    let resolve_fut = Box::pin({
                        let ctx = ctx.clone();
                        async move {
                            let ctx_field = ctx.with_field(field);
                            let field_name = ctx_field.item.node.response_key().node.clone();
                            Ok((
                                field_name,
                                root.resolve_field(&ctx_field).await?.unwrap_or_default(),
                            ))
                        }
                    });
                    self.0.push(resolve_fut);
                }
                selection => {
                    let (type_condition, selection_set) = match selection {
                        Selection::Field(_) => unreachable!(),
                        Selection::FragmentSpread(spread) => {
                            let fragment =
                                ctx.query_env.fragments.get(&spread.node.fragment_name.node);
                            let fragment = match fragment {
                                Some(fragment) => fragment,
                                None => {
                                    return Err(ServerError::new(
                                        format!(
                                            r#"Unknown fragment "{}"."#,
                                            spread.node.fragment_name.node
                                        ),
                                        Some(spread.pos),
                                    ));
                                }
                            };
                            (
                                Some(&fragment.node.type_condition),
                                &fragment.node.selection_set,
                            )
                        }
                        Selection::InlineFragment(fragment) => (
                            fragment.node.type_condition.as_ref(),
                            &fragment.node.selection_set,
                        ),
                    };
                    let type_condition =
                        type_condition.map(|condition| condition.node.on.node.as_str());
                    let introspection_type_name = root.introspection_type_name();
                    let applies_concrete_object = type_condition.is_some_and(|condition| {
                        introspection_type_name == condition
                            || ctx
                                .schema_env
                                .registry
                                .implements
                                .get(&*introspection_type_name)
                                .is_some_and(|interfaces| interfaces.contains(condition))
                    });
                    if applies_concrete_object {
                        root.collect_all_fields(&ctx.with_selection_set(selection_set), self)?;
                    } else if type_condition.is_none_or(|condition| T::type_name() == condition) {
                        self.add_set(&ctx.with_selection_set(selection_set), root)?;
                    }
                }
            }
        }
        Ok(())
    }
}
