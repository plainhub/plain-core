use crate::parser::types::ExecutableDocument;
use crate::{
    CacheControl, ContextBase, EmptyMutation, EmptySubscription, InputType, ObjectType, OutputType,
    QueryEnv, Request, Response, ServerError, ServerResult, SubscriptionType, Variables,
    context::{Data, QueryEnvInner},
    parser::{
        Positioned, parse_query,
        types::{Directive, DocumentOperations, OperationType, Selection, SelectionSet},
    },
    registry::Registry,
    resolver_utils::{resolve_container, resolve_container_serial},
    subscription::collect_subscription_streams,
    types::QueryRoot,
    validation::check_rules,
};
use futures_util::stream::{self, BoxStream, StreamExt};
use std::{
    any::{Any, TypeId},
    ops::Deref,
    sync::Arc,
};
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum IntrospectionMode {
    IntrospectionOnly,
    #[default]
    Enabled,
    Disabled,
}
pub struct SchemaBuilder<Query, Mutation, Subscription> {
    query: QueryRoot<Query>,
    mutation: Mutation,
    subscription: Subscription,
    registry: Registry,
    data: Data,
    complexity: Option<usize>,
    depth: Option<usize>,
    recursive_depth: usize,
    max_directives: Option<usize>,
    max_aliases: Option<usize>,
}
impl<Query, Mutation, Subscription> SchemaBuilder<Query, Mutation, Subscription> {
    #[must_use]
    pub fn register_output_type<T: OutputType>(mut self) -> Self {
        T::create_type_info(&mut self.registry);
        self
    }
    #[must_use]
    pub fn disable_introspection(mut self) -> Self {
        self.registry.introspection_mode = IntrospectionMode::Disabled;
        self
    }
    #[must_use]
    pub fn introspection_only(mut self) -> Self {
        self.registry.introspection_mode = IntrospectionMode::IntrospectionOnly;
        self
    }
    #[must_use]
    pub fn limit_complexity(mut self, complexity: usize) -> Self {
        self.complexity = Some(complexity);
        self
    }
    #[must_use]
    pub fn limit_aliases(mut self, max_aliases: usize) -> Self {
        self.max_aliases = Some(max_aliases);
        self
    }
    #[must_use]
    pub fn limit_depth(mut self, depth: usize) -> Self {
        self.depth = Some(depth);
        self
    }
    #[must_use]
    pub fn limit_recursive_depth(mut self, depth: usize) -> Self {
        self.recursive_depth = depth;
        self
    }
    pub fn limit_directives(mut self, max_directives: usize) -> Self {
        self.max_directives = Some(max_directives);
        self
    }
    #[must_use]
    pub fn data<D: Any + Send + Sync>(mut self, data: D) -> Self {
        self.data.insert(data);
        self
    }

    pub fn finish(self) -> Schema<Query, Mutation, Subscription> {
        Schema(Arc::new(SchemaInner {
            query: self.query,
            mutation: self.mutation,
            subscription: self.subscription,
            complexity: self.complexity,
            depth: self.depth,
            recursive_depth: self.recursive_depth,
            max_directives: self.max_directives,
            max_aliases: self.max_aliases,
            env: SchemaEnv(Arc::new(SchemaEnvInner {
                registry: self.registry,
                data: self.data,
            })),
        }))
    }
}
#[doc(hidden)]
pub struct SchemaEnvInner {
    pub registry: Registry,
    pub data: Data,
}
#[doc(hidden)]
#[derive(Clone)]
pub struct SchemaEnv(pub(crate) Arc<SchemaEnvInner>);
impl Deref for SchemaEnv {
    type Target = SchemaEnvInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
#[doc(hidden)]
pub struct SchemaInner<Query, Mutation, Subscription> {
    pub(crate) query: QueryRoot<Query>,
    pub(crate) mutation: Mutation,
    pub(crate) subscription: Subscription,
    pub(crate) complexity: Option<usize>,
    pub(crate) depth: Option<usize>,
    pub(crate) recursive_depth: usize,
    pub(crate) max_directives: Option<usize>,
    pub(crate) max_aliases: Option<usize>,
    pub(crate) env: SchemaEnv,
}
pub struct Schema<Query, Mutation, Subscription>(
    pub(crate) Arc<SchemaInner<Query, Mutation, Subscription>>,
);
impl<Query, Mutation, Subscription> Clone for Schema<Query, Mutation, Subscription> {
    fn clone(&self) -> Self {
        Schema(self.0.clone())
    }
}
impl<Query, Mutation, Subscription> Default for Schema<Query, Mutation, Subscription>
where
    Query: Default + ObjectType + 'static,
    Mutation: Default + ObjectType + 'static,
    Subscription: Default + SubscriptionType + 'static,
{
    fn default() -> Self {
        Schema::new(
            Query::default(),
            Mutation::default(),
            Subscription::default(),
        )
    }
}
impl<Query, Mutation, Subscription> Schema<Query, Mutation, Subscription>
where
    Query: ObjectType + 'static,
    Mutation: ObjectType + 'static,
    Subscription: SubscriptionType + 'static,
{
    pub fn build(
        query: Query,
        mutation: Mutation,
        subscription: Subscription,
    ) -> SchemaBuilder<Query, Mutation, Subscription> {
        SchemaBuilder {
            query: QueryRoot { inner: query },
            mutation,
            subscription,
            registry: Self::create_registry(),
            data: Default::default(),
            complexity: None,
            depth: None,
            recursive_depth: 32,
            max_directives: None,
            max_aliases: None,
        }
    }

    pub(crate) fn create_registry() -> Registry {
        let mut registry = Registry {
            types: Default::default(),
            directives: Default::default(),
            implements: Default::default(),
            query_type: Query::type_name().to_string(),
            mutation_type: if Mutation::is_empty() {
                None
            } else {
                Some(Mutation::type_name().to_string())
            },
            subscription_type: if Subscription::is_empty() {
                None
            } else {
                Some(Subscription::type_name().to_string())
            },
            introspection_mode: IntrospectionMode::Enabled,
            enable_suggestions: true,
        };
        registry.add_system_types();
        QueryRoot::<Query>::create_type_info(&mut registry);
        if !Mutation::is_empty() {
            Mutation::create_type_info(&mut registry);
        }
        if !Subscription::is_empty() {
            Subscription::create_type_info(&mut registry);
        }
        registry.remove_unused_types();
        registry
    }
    pub fn new(
        query: Query,
        mutation: Mutation,
        subscription: Subscription,
    ) -> Schema<Query, Mutation, Subscription> {
        Self::build(query, mutation, subscription).finish()
    }

    pub fn sdl(&self) -> String {
        self.0.env.registry.export_sdl()
    }

    async fn execute_once(&self, env: QueryEnv) -> Response {
        let ctx = ContextBase {
            path_node: None,
            is_for_introspection: false,
            item: &env.operation.node.selection_set,
            schema_env: &self.0.env,
            query_env: &env,
        };
        let res = match &env.operation.node.ty {
            OperationType::Query => resolve_container(&ctx, &self.0.query).await,
            OperationType::Mutation => {
                if self.0.env.registry.introspection_mode == IntrospectionMode::IntrospectionOnly
                    || env.introspection_mode == IntrospectionMode::IntrospectionOnly
                {
                    resolve_container_serial(&ctx, &EmptyMutation).await
                } else {
                    resolve_container_serial(&ctx, &self.0.mutation).await
                }
            }
            OperationType::Subscription => Err(ServerError::new(
                "Subscriptions are not supported on this transport.",
                None,
            )),
        };
        let mut resp = match res {
            Ok(value) => Response::new(value),
            Err(err) => Response::from_errors(vec![err]),
        };
        resp.errors
            .extend(std::mem::take(&mut *env.errors.lock().unwrap()));
        resp
    }
    pub async fn execute(&self, request: impl Into<Request>) -> Response {
        match prepare_request(
            request.into(),
            Default::default(),
            &self.0.env.registry,
            self.0.recursive_depth,
            self.0.max_directives,
            self.0.max_aliases,
            self.0.complexity,
            self.0.depth,
        ) {
            Ok((env, cache_control)) => self.execute_once(env).await.cache_control(cache_control),
            Err(errors) => Response::from_errors(errors),
        }
    }
    pub fn execute_stream_with_session_data(
        &self,
        request: impl Into<Request>,
        session_data: Arc<Data>,
    ) -> BoxStream<'static, Response> {
        let schema = self.clone();
        let request = request.into();
        asynk_strim::stream_fn(|mut yielder| async move {
            let (env, cache_control) = match prepare_request(
                request,
                session_data,
                &schema.0.env.registry,
                schema.0.recursive_depth,
                schema.0.max_directives,
                schema.0.max_aliases,
                schema.0.complexity,
                schema.0.depth,
            ) {
                Ok(res) => res,
                Err(errors) => {
                    yielder.yield_item(Response::from_errors(errors)).await;
                    return;
                }
            };
            if env.operation.node.ty != OperationType::Subscription {
                yielder
                    .yield_item(schema.execute_once(env).await.cache_control(cache_control))
                    .await;
                return;
            }
            let ctx = env.create_context(&schema.0.env, None, &env.operation.node.selection_set);
            let mut streams = Vec::new();
            let result = if schema.0.env.registry.introspection_mode
                == IntrospectionMode::IntrospectionOnly
                || env.introspection_mode == IntrospectionMode::IntrospectionOnly
            {
                collect_subscription_streams(&ctx, &EmptySubscription, &mut streams)
            } else {
                collect_subscription_streams(&ctx, &schema.0.subscription, &mut streams)
            };
            if let Err(err) = result {
                yielder.yield_item(Response::from_errors(vec![err])).await;
            }
            let mut stream = stream::select_all(streams);
            while let Some(resp) = stream.next().await {
                yielder.yield_item(resp).await;
            }
        })
        .boxed()
    }
    pub fn execute_stream(&self, request: impl Into<Request>) -> BoxStream<'static, Response> {
        self.execute_stream_with_session_data(request, Default::default())
    }
    pub fn data<D: Any + Send + Sync>(&self) -> Option<&D> {
        self.0
            .env
            .data
            .get(&TypeId::of::<D>())
            .and_then(|d| d.downcast_ref::<D>())
    }
}
fn check_max_directives(doc: &ExecutableDocument, max_directives: usize) -> ServerResult<()> {
    fn check_selection_set(
        doc: &ExecutableDocument,
        selection_set: &Positioned<SelectionSet>,
        limit_directives: usize,
    ) -> ServerResult<()> {
        for selection in &selection_set.node.items {
            match &selection.node {
                Selection::Field(field) => {
                    if field.node.directives.len() > limit_directives {
                        return Err(ServerError::new(
                            format!(
                                "The number of directives on the field `{}` cannot be greater than `{}`",
                                field.node.name.node, limit_directives
                            ),
                            Some(field.pos),
                        ));
                    }
                    check_selection_set(doc, &field.node.selection_set, limit_directives)?;
                }
                Selection::FragmentSpread(fragment_spread) => {
                    if let Some(fragment) =
                        doc.fragments.get(&fragment_spread.node.fragment_name.node)
                    {
                        check_selection_set(doc, &fragment.node.selection_set, limit_directives)?;
                    }
                }
                Selection::InlineFragment(inline_fragment) => {
                    check_selection_set(
                        doc,
                        &inline_fragment.node.selection_set,
                        limit_directives,
                    )?;
                }
            }
        }
        Ok(())
    }
    for (_, operation) in doc.operations.iter() {
        check_selection_set(doc, &operation.node.selection_set, max_directives)?;
    }
    Ok(())
}
fn check_alias_count(doc: &ExecutableDocument, max_aliases: usize) -> ServerResult<()> {
    fn check_selection_set(
        doc: &ExecutableDocument,
        selection_set: &Positioned<SelectionSet>,
        current_aliases: &mut usize,
        max_aliases: usize,
    ) -> ServerResult<()> {
        for selection in &selection_set.node.items {
            match &selection.node {
                Selection::Field(field) => {
                    if field.node.alias.is_some() {
                        *current_aliases += 1;
                    }
                    if !field.node.selection_set.node.items.is_empty() {
                        check_selection_set(
                            doc,
                            &field.node.selection_set,
                            current_aliases,
                            max_aliases,
                        )?;
                    }
                }
                Selection::FragmentSpread(fragment_spread) => {
                    if let Some(fragment) =
                        doc.fragments.get(&fragment_spread.node.fragment_name.node)
                    {
                        check_selection_set(
                            doc,
                            &fragment.node.selection_set,
                            current_aliases,
                            max_aliases,
                        )?;
                    }
                }
                Selection::InlineFragment(inline_fragment) => {
                    check_selection_set(
                        doc,
                        &inline_fragment.node.selection_set,
                        current_aliases,
                        max_aliases,
                    )?;
                }
            }
            if *current_aliases > max_aliases {
                return Err(ServerError::new(
                    format!(
                        "The amount of aliases of the query cannot be greater than `{}`",
                        max_aliases
                    ),
                    Some(selection_set.pos),
                ));
            }
        }
        Ok(())
    }
    let mut current_aliases = 0;
    for (_, operation) in doc.operations.iter() {
        check_selection_set(
            doc,
            &operation.node.selection_set,
            &mut current_aliases,
            max_aliases,
        )?;
    }
    Ok(())
}
fn check_recursive_depth(doc: &ExecutableDocument, max_depth: usize) -> ServerResult<()> {
    fn check_selection_set(
        doc: &ExecutableDocument,
        selection_set: &Positioned<SelectionSet>,
        current_depth: usize,
        max_depth: usize,
    ) -> ServerResult<()> {
        if current_depth > max_depth {
            return Err(ServerError::new(
                format!(
                    "The recursion depth of the query cannot be greater than `{}`",
                    max_depth
                ),
                Some(selection_set.pos),
            ));
        }
        for selection in &selection_set.node.items {
            match &selection.node {
                Selection::Field(field) => {
                    if !field.node.selection_set.node.items.is_empty() {
                        check_selection_set(
                            doc,
                            &field.node.selection_set,
                            current_depth + 1,
                            max_depth,
                        )?;
                    }
                }
                Selection::FragmentSpread(fragment_spread) => {
                    if let Some(fragment) =
                        doc.fragments.get(&fragment_spread.node.fragment_name.node)
                    {
                        check_selection_set(
                            doc,
                            &fragment.node.selection_set,
                            current_depth + 1,
                            max_depth,
                        )?;
                    }
                }
                Selection::InlineFragment(inline_fragment) => {
                    check_selection_set(
                        doc,
                        &inline_fragment.node.selection_set,
                        current_depth + 1,
                        max_depth,
                    )?;
                }
            }
        }
        Ok(())
    }
    for (_, operation) in doc.operations.iter() {
        check_selection_set(doc, &operation.node.selection_set, 0, max_depth)?;
    }
    Ok(())
}
fn remove_skipped_selection(selection_set: &mut SelectionSet, variables: &Variables) {
    fn is_skipped(directives: &[Positioned<Directive>], variables: &Variables) -> bool {
        for directive in directives {
            let include = match &*directive.node.name.node {
                "skip" => false,
                "include" => true,
                _ => continue,
            };
            if let Some(condition_input) = directive.node.get_argument("if") {
                let value = condition_input
                    .node
                    .clone()
                    .into_const_with(|name| variables.get(&name).cloned().ok_or(()))
                    .unwrap_or_default();
                let value: bool = InputType::parse(Some(value)).unwrap_or_default();
                if include != value {
                    return true;
                }
            }
        }
        false
    }
    selection_set
        .items
        .retain(|selection| !is_skipped(selection.node.directives(), variables));
    for selection in &mut selection_set.items {
        selection.node.directives_mut().retain(|directive| {
            directive.node.name.node != "skip" && directive.node.name.node != "include"
        });
    }
    for selection in &mut selection_set.items {
        match &mut selection.node {
            Selection::Field(field) => {
                remove_skipped_selection(&mut field.node.selection_set.node, variables);
            }
            Selection::FragmentSpread(_) => {}
            Selection::InlineFragment(inline_fragment) => {
                remove_skipped_selection(&mut inline_fragment.node.selection_set.node, variables);
            }
        }
    }
}
pub(crate) fn prepare_request(
    mut request: Request,
    session_data: Arc<Data>,
    registry: &Registry,

    recursive_depth: usize,
    max_directives: Option<usize>,
    max_aliases: Option<usize>,
    complexity: Option<usize>,
    depth: Option<usize>,
) -> Result<(QueryEnv, CacheControl), Vec<ServerError>> {
    let query_data = Arc::new(std::mem::take(&mut request.data));
    let mut document = match request.parsed_query.take() {
        Some(document) => document,
        None => parse_query(&request.query).map_err(|error| vec![error.into()])?,
    };
    check_recursive_depth(&document, recursive_depth).map_err(|error| vec![error])?;
    if let Some(max) = max_directives {
        check_max_directives(&document, max).map_err(|error| vec![error])?;
    }
    if let Some(max) = max_aliases {
        check_alias_count(&document, max).map_err(|error| vec![error])?;
    }
    crate::validation::enum_literals::validate(registry, &document).map_err(|error| vec![error])?;
    let validation_result = check_rules(
        registry,
        &document,
        Some(&request.variables),
        request.operation_name.as_deref(),
        complexity,
        depth,
    )?;
    let operation = if let Some(operation_name) = &request.operation_name {
        match document.operations {
            DocumentOperations::Single(_) => None,
            DocumentOperations::Multiple(mut operations) => {
                operations.remove(operation_name.as_str())
            }
        }
        .ok_or_else(|| {
            ServerError::new(
                format!(r#"Unknown operation named "{}""#, operation_name),
                None,
            )
        })
    } else {
        match document.operations {
            DocumentOperations::Single(operation) => Ok(operation),
            DocumentOperations::Multiple(map) if map.len() == 1 => {
                let (_, operation) = map.into_iter().next().unwrap();
                Ok(operation)
            }
            DocumentOperations::Multiple(_) => Err(ServerError::new(
                "Operation name required in request.",
                None,
            )),
        }
    };
    let mut operation = operation.map_err(|err| vec![err])?;
    for fragment in document.fragments.values_mut() {
        remove_skipped_selection(&mut fragment.node.selection_set.node, &request.variables);
    }
    remove_skipped_selection(&mut operation.node.selection_set.node, &request.variables);
    let env = QueryEnvInner {
        variables: request.variables,
        operation,
        fragments: document.fragments,
        session_data,
        query_data,

        introspection_mode: request.introspection_mode,
        errors: Default::default(),
    };
    Ok((QueryEnv::new(env), validation_result.cache_control))
}

#[cfg(test)]
#[path = "../../tests/plain-graphql/unit/schema.rs"]
mod tests;
