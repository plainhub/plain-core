use crate::{
    Data, Extensions, ServerError, Variables,
    parser::{parse_query, types::ExecutableDocument},
    schema::IntrospectionMode,
};
use serde::{Deserialize, Serialize};
use std::{
    any::Any,
    fmt::{self, Debug, Formatter},
};
#[non_exhaustive]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    #[serde(default)]
    pub query: String,
    #[serde(default, rename = "operationName")]
    pub operation_name: Option<String>,
    #[serde(default)]
    pub variables: Variables,
    #[serde(skip)]
    pub data: Data,
    #[serde(default)]
    pub extensions: Extensions,
    #[serde(skip)]
    pub(crate) parsed_query: Option<ExecutableDocument>,
    #[serde(skip)]
    pub introspection_mode: IntrospectionMode,
}
impl Request {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            operation_name: None,
            variables: Variables::default(),
            data: Data::default(),
            extensions: Default::default(),
            parsed_query: None,
            introspection_mode: IntrospectionMode::Enabled,
        }
    }
    #[must_use]
    pub fn operation_name<T: Into<String>>(self, name: T) -> Self {
        Self {
            operation_name: Some(name.into()),
            ..self
        }
    }
    #[must_use]
    pub fn variables(self, variables: Variables) -> Self {
        Self { variables, ..self }
    }
    #[must_use]
    pub fn data<D: Any + Send + Sync>(mut self, data: D) -> Self {
        self.data.insert(data);
        self
    }
    #[must_use]
    pub fn disable_introspection(mut self) -> Self {
        self.introspection_mode = IntrospectionMode::Disabled;
        self
    }
    #[must_use]
    pub fn only_introspection(mut self) -> Self {
        self.introspection_mode = IntrospectionMode::IntrospectionOnly;
        self
    }
    #[inline]
    pub fn parsed_query(&mut self) -> Result<&ExecutableDocument, ServerError> {
        if self.parsed_query.is_none() {
            match parse_query(&self.query) {
                Ok(parsed) => self.parsed_query = Some(parsed),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(self.parsed_query.as_ref().unwrap())
    }
    pub fn set_parsed_query(&mut self, doc: ExecutableDocument) {
        self.parsed_query = Some(doc);
    }
}
impl<T: Into<String>> From<T> for Request {
    fn from(query: T) -> Self {
        Self::new(query)
    }
}
impl Debug for Request {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Request")
            .field("query", &self.query)
            .field("operation_name", &self.operation_name)
            .field("variables", &self.variables)
            .field("extensions", &self.extensions)
            .finish()
    }
}
#[cfg(test)]
#[path = "../../tests/plain-graphql/unit/request_tests.rs"]
mod tests;
