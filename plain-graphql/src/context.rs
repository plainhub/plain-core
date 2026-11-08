use crate::parser::types::ConstDirective;
use crate::value::{Value as InputValue, Variables};
use crate::{
    Error, InputType, Lookahead, Name, PathSegment, Pos, Positioned, Result, ServerError,
    ServerResult, Value,
    parser::types::{
        Directive, Field, FragmentDefinition, OperationDefinition, Selection, SelectionSet,
    },
    schema::{IntrospectionMode, SchemaEnv},
};
use indexmap::IndexMap;
use serde::{
    Serialize,
    ser::{SerializeSeq, Serializer},
};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt::{self, Debug, Display, Formatter},
    ops::Deref,
    sync::{Arc, Mutex},
};

#[derive(Default)]
pub struct Data(HashMap<TypeId, Box<dyn Any + Sync + Send>>);
impl Deref for Data {
    type Target = HashMap<TypeId, Box<dyn Any + Sync + Send>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Data {
    pub fn insert<D: Any + Send + Sync>(&mut self, data: D) {
        self.0.insert(TypeId::of::<D>(), Box::new(data));
    }
}
impl Debug for Data {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_tuple("Data").finish()
    }
}
pub type ContextSelectionSet<'a> = ContextBase<'a, &'a Positioned<SelectionSet>>;
pub type Context<'a> = ContextBase<'a, &'a Positioned<Field>>;
pub type ContextDirective<'a> = ContextBase<'a, &'a Positioned<Directive>>;
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(untagged)]
pub enum QueryPathSegment<'a> {
    Index(usize),
    Name(&'a str),
}
#[derive(Debug, Clone, Copy)]
pub struct QueryPathNode<'a> {
    pub parent: Option<&'a QueryPathNode<'a>>,
    pub segment: QueryPathSegment<'a>,
}
impl serde::Serialize for QueryPathNode<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(None)?;
        self.try_for_each(|segment| seq.serialize_element(segment))?;
        seq.end()
    }
}
impl Display for QueryPathNode<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut first = true;
        self.try_for_each(|segment| {
            if !first {
                write!(f, ".")?;
            }
            first = false;
            match segment {
                QueryPathSegment::Index(idx) => write!(f, "{}", *idx),
                QueryPathSegment::Name(name) => write!(f, "{}", name),
            }
        })
    }
}
impl<'a> QueryPathNode<'a> {
    pub fn field_name(&self) -> &str {
        std::iter::once(self)
            .chain(self.parents())
            .find_map(|node| match node.segment {
                QueryPathSegment::Name(name) => Some(name),
                QueryPathSegment::Index(_) => None,
            })
            .unwrap()
    }
    #[must_use]
    pub fn to_string_vec(self) -> Vec<String> {
        let mut res = Vec::new();
        self.for_each(|s| {
            res.push(match s {
                QueryPathSegment::Name(name) => (*name).to_string(),
                QueryPathSegment::Index(idx) => idx.to_string(),
            });
        });
        res
    }
    pub fn parents(&self) -> Parents<'_> {
        Parents(self)
    }
    pub(crate) fn for_each<F: FnMut(&QueryPathSegment<'a>)>(&self, mut f: F) {
        let _ = self.try_for_each::<std::convert::Infallible, _>(|segment| {
            f(segment);
            Ok(())
        });
    }
    pub(crate) fn try_for_each<E, F: FnMut(&QueryPathSegment<'a>) -> Result<(), E>>(
        &self,
        mut f: F,
    ) -> Result<(), E> {
        self.try_for_each_ref(&mut f)
    }
    fn try_for_each_ref<E, F: FnMut(&QueryPathSegment<'a>) -> Result<(), E>>(
        &self,
        f: &mut F,
    ) -> Result<(), E> {
        if let Some(parent) = &self.parent {
            parent.try_for_each_ref(f)?;
        }
        f(&self.segment)
    }
}
#[derive(Debug, Clone)]
pub struct Parents<'a>(&'a QueryPathNode<'a>);
impl<'a> Parents<'a> {
    #[must_use]
    pub fn current(&self) -> &'a QueryPathNode<'a> {
        self.0
    }
}
impl<'a> Iterator for Parents<'a> {
    type Item = &'a QueryPathNode<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let parent = self.0.parent;
        if let Some(parent) = parent {
            self.0 = parent;
        }
        parent
    }
}
impl std::iter::FusedIterator for Parents<'_> {}
#[derive(Clone)]
pub struct ContextBase<'a, T> {
    pub path_node: Option<QueryPathNode<'a>>,
    pub(crate) is_for_introspection: bool,
    #[doc(hidden)]
    pub item: T,
    #[doc(hidden)]
    pub schema_env: &'a SchemaEnv,
    #[doc(hidden)]
    pub query_env: &'a QueryEnv,
}
#[doc(hidden)]
pub struct QueryEnvInner {
    pub variables: Variables,
    pub operation: Positioned<OperationDefinition>,
    pub fragments: HashMap<Name, Positioned<FragmentDefinition>>,
    pub session_data: Arc<Data>,
    pub query_data: Arc<Data>,
    pub introspection_mode: IntrospectionMode,
    pub errors: Mutex<Vec<ServerError>>,
}
#[doc(hidden)]
#[derive(Clone)]
pub struct QueryEnv(Arc<QueryEnvInner>);
impl Deref for QueryEnv {
    type Target = QueryEnvInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl QueryEnv {
    #[doc(hidden)]
    pub fn new(inner: QueryEnvInner) -> QueryEnv {
        QueryEnv(Arc::new(inner))
    }
    #[doc(hidden)]
    pub fn create_context<'a, T>(
        &'a self,
        schema_env: &'a SchemaEnv,
        path_node: Option<QueryPathNode<'a>>,
        item: T,
    ) -> ContextBase<'a, T> {
        ContextBase {
            path_node,
            is_for_introspection: false,
            item,
            schema_env,
            query_env: self,
        }
    }
}
impl<'a, T> ContextBase<'a, T> {
    #[doc(hidden)]
    pub fn with_field(
        &'a self,
        field: &'a Positioned<Field>,
    ) -> ContextBase<'a, &'a Positioned<Field>> {
        ContextBase {
            path_node: Some(QueryPathNode {
                parent: self.path_node.as_ref(),
                segment: QueryPathSegment::Name(&field.node.response_key().node),
            }),
            is_for_introspection: self.is_for_introspection,
            item: field,
            schema_env: self.schema_env,
            query_env: self.query_env,
        }
    }
    #[doc(hidden)]
    pub fn with_selection_set(
        &self,
        selection_set: &'a Positioned<SelectionSet>,
    ) -> ContextBase<'a, &'a Positioned<SelectionSet>> {
        ContextBase {
            path_node: self.path_node,
            is_for_introspection: self.is_for_introspection,
            item: selection_set,
            schema_env: self.schema_env,
            query_env: self.query_env,
        }
    }
    #[doc(hidden)]
    pub fn set_error_path(&self, error: ServerError) -> ServerError {
        if let Some(node) = self.path_node {
            let mut path = Vec::new();
            node.for_each(|current_node| {
                path.push(match current_node {
                    QueryPathSegment::Name(name) => PathSegment::Field((*name).to_string()),
                    QueryPathSegment::Index(idx) => PathSegment::Index(*idx),
                })
            });
            ServerError { path, ..error }
        } else {
            error
        }
    }
    pub fn add_error(&self, error: ServerError) {
        self.query_env.errors.lock().unwrap().push(error);
    }
    pub fn data<D: Any + Send + Sync>(&self) -> Result<&'a D> {
        self.data_opt::<D>().ok_or_else(|| {
            Error::new(format!(
                "Data `{}` does not exist.",
                std::any::type_name::<D>()
            ))
        })
    }
    pub fn data_unchecked<D: Any + Send + Sync>(&self) -> &'a D {
        self.data_opt::<D>()
            .unwrap_or_else(|| panic!("Data `{}` does not exist.", std::any::type_name::<D>()))
    }
    pub fn data_opt<D: Any + Send + Sync>(&self) -> Option<&'a D> {
        self.query_env
            .query_data
            .0
            .get(&TypeId::of::<D>())
            .or_else(|| self.query_env.session_data.0.get(&TypeId::of::<D>()))
            .or_else(|| self.schema_env.data.0.get(&TypeId::of::<D>()))
            .and_then(|d| d.downcast_ref::<D>())
    }

    fn var_value(&self, name: &str, pos: Pos) -> ServerResult<Option<Value>> {
        let def = self
            .query_env
            .operation
            .node
            .variable_definitions
            .iter()
            .find(|def| def.node.name.node == name)
            .ok_or_else(|| {
                ServerError::new(format!("Variable {} is not defined.", name), Some(pos))
            })?;
        Ok(self
            .query_env
            .variables
            .get(&def.node.name.node)
            .cloned()
            .or_else(|| {
                def.node
                    .default_value
                    .as_ref()
                    .map(|value| value.node.clone())
            }))
    }
    fn resolve_input_value_inner(
        &self,
        value: InputValue,
        pos: Pos,
    ) -> ServerResult<Option<Value>> {
        Ok(match value {
            InputValue::Variable(name) => self.var_value(&name, pos)?,
            InputValue::Null => Some(Value::Null),
            InputValue::Number(num) => Some(Value::Number(num)),
            InputValue::String(value) => Some(Value::String(value)),
            InputValue::Boolean(value) => Some(Value::Boolean(value)),
            InputValue::Binary(value) => Some(Value::Binary(value)),
            InputValue::Enum(value) => Some(Value::Enum(value)),
            InputValue::List(items) => {
                let mut resolved_items = Vec::with_capacity(items.len());
                for item in items {
                    resolved_items.push(
                        self.resolve_input_value_inner(item, pos)?
                            .unwrap_or(Value::Null),
                    );
                }
                Some(Value::List(resolved_items))
            }
            InputValue::Object(object) => {
                let mut resolved_object = IndexMap::with_capacity(object.len());
                for (name, value) in object {
                    if let Some(value) = self.resolve_input_value_inner(value, pos)? {
                        resolved_object.insert(name, value);
                    }
                }
                Some(Value::Object(resolved_object))
            }
        })
    }
    pub(crate) fn resolve_input_value(
        &self,
        value: Positioned<InputValue>,
    ) -> ServerResult<Option<Value>> {
        self.resolve_input_value_inner(value.node, value.pos)
    }
    #[doc(hidden)]
    fn get_param_value<Q: InputType>(
        &self,
        arguments: &[(Positioned<Name>, Positioned<InputValue>)],
        name: &str,
        default: Option<fn() -> Q>,
    ) -> ServerResult<(Pos, Q)> {
        let value = arguments
            .iter()
            .find(|(n, _)| n.node.as_str() == name)
            .map(|(_, value)| value)
            .cloned();
        if value.is_none()
            && let Some(default) = default
        {
            return Ok((Pos::default(), default()));
        }
        let (pos, value) = match value {
            Some(value) => (value.pos, self.resolve_input_value(value)?),
            None => (Pos::default(), None),
        };
        InputType::parse(value)
            .map(|value| (pos, value))
            .map_err(|e| e.into_server_error(pos))
    }
    #[doc(hidden)]
    #[must_use]
    pub fn with_index(&'a self, idx: usize) -> ContextBase<'a, T>
    where
        T: Copy,
    {
        ContextBase {
            path_node: Some(QueryPathNode {
                parent: self.path_node.as_ref(),
                segment: QueryPathSegment::Index(idx),
            }),
            is_for_introspection: self.is_for_introspection,
            item: self.item,
            schema_env: self.schema_env,
            query_env: self.query_env,
        }
    }
}
impl<'a> ContextBase<'a, &'a Positioned<Field>> {
    #[doc(hidden)]
    pub fn param_value<T: InputType>(
        &self,
        name: &str,
        default: Option<fn() -> T>,
    ) -> ServerResult<(Pos, T)> {
        self.get_param_value(&self.item.node.arguments, name, default)
    }

    pub fn look_ahead(&self) -> Lookahead<'_> {
        Lookahead::new(&self.query_env.fragments, &self.item.node, self)
    }
    pub fn field(&self) -> SelectionField<'_> {
        SelectionField {
            fragments: &self.query_env.fragments,
            field: &self.item.node,
            context: self,
        }
    }
}
impl<'a> ContextBase<'a, &'a Positioned<Directive>> {
    #[doc(hidden)]
    pub fn param_value<T: InputType>(
        &self,
        name: &str,
        default: Option<fn() -> T>,
    ) -> ServerResult<(Pos, T)> {
        self.get_param_value(&self.item.node.arguments, name, default)
    }
}
#[derive(Clone, Copy)]
pub struct SelectionField<'a> {
    pub(crate) fragments: &'a HashMap<Name, Positioned<FragmentDefinition>>,
    pub(crate) field: &'a Field,
    pub(crate) context: &'a Context<'a>,
}
impl<'a> SelectionField<'a> {
    #[inline]
    pub fn name(&self) -> &'a str {
        self.field.name.node.as_str()
    }
    #[inline]
    pub fn alias(&self) -> Option<&'a str> {
        self.field.alias.as_ref().map(|alias| alias.node.as_str())
    }
    pub fn directives(&self) -> ServerResult<Vec<ConstDirective>> {
        let mut directives = Vec::with_capacity(self.field.directives.len());
        for directive in &self.field.directives {
            let directive = &directive.node;
            let mut arguments = Vec::with_capacity(directive.arguments.len());
            for (name, value) in &directive.arguments {
                if let Some(resolved_value) = self.context.resolve_input_value(value.clone())? {
                    arguments.push((name.clone(), value.position_node(resolved_value)));
                }
            }
            directives.push(ConstDirective {
                name: directive.name.clone(),
                arguments,
            });
        }
        Ok(directives)
    }
    pub fn arguments(&self) -> ServerResult<Vec<(Name, Value)>> {
        let mut arguments = Vec::with_capacity(self.field.arguments.len());
        for (name, value) in &self.field.arguments {
            if let Some(value) = self.context.resolve_input_value(value.clone())? {
                arguments.push((name.node.clone(), value));
            }
        }
        Ok(arguments)
    }
    pub fn selection_set(&self) -> impl Iterator<Item = SelectionField<'a>> {
        SelectionFieldsIter {
            fragments: self.fragments,
            iter: vec![self.field.selection_set.node.items.iter()],
            context: self.context,
        }
    }
}
impl Debug for SelectionField<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        struct DebugSelectionSet<'a>(Vec<SelectionField<'a>>);
        impl Debug for DebugSelectionSet<'_> {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.debug_list().entries(&self.0).finish()
            }
        }
        f.debug_struct(self.name())
            .field("name", &self.name())
            .field(
                "selection_set",
                &DebugSelectionSet(self.selection_set().collect()),
            )
            .finish()
    }
}
struct SelectionFieldsIter<'a> {
    fragments: &'a HashMap<Name, Positioned<FragmentDefinition>>,
    iter: Vec<std::slice::Iter<'a, Positioned<Selection>>>,
    context: &'a Context<'a>,
}
impl<'a> Iterator for SelectionFieldsIter<'a> {
    type Item = SelectionField<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let it = self.iter.last_mut()?;
            let item = it.next();
            match item {
                Some(selection) => match &selection.node {
                    Selection::Field(field) => {
                        return Some(SelectionField {
                            fragments: self.fragments,
                            field: &field.node,
                            context: self.context,
                        });
                    }
                    Selection::FragmentSpread(fragment_spread) => {
                        if let Some(fragment) =
                            self.fragments.get(&fragment_spread.node.fragment_name.node)
                        {
                            self.iter
                                .push(fragment.node.selection_set.node.items.iter());
                        }
                    }
                    Selection::InlineFragment(inline_fragment) => {
                        self.iter
                            .push(inline_fragment.node.selection_set.node.items.iter());
                    }
                },
                None => {
                    self.iter.pop();
                }
            }
        }
    }
}
