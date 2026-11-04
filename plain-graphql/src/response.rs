use crate::{CacheControl, Result, ServerError, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[non_exhaustive]
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Response {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub errors: Vec<ServerError>,
    #[serde(default)]
    pub data: Value,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub extensions: BTreeMap<String, Value>,
    #[serde(skip)]
    pub cache_control: CacheControl,
}
impl Response {
    #[must_use]
    pub fn new(data: impl Into<Value>) -> Self {
        Self {
            data: data.into(),
            ..Default::default()
        }
    }
    #[must_use]
    pub fn from_errors(errors: Vec<ServerError>) -> Self {
        Self {
            errors,
            ..Default::default()
        }
    }
    #[must_use]
    pub fn extension(mut self, name: impl Into<String>, value: Value) -> Self {
        self.extensions.insert(name.into(), value);
        self
    }

    #[must_use]
    pub fn cache_control(self, cache_control: CacheControl) -> Self {
        Self {
            cache_control,
            ..self
        }
    }
    #[inline]
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
    #[inline]
    pub fn is_err(&self) -> bool {
        !self.is_ok()
    }
    #[inline]
    pub fn into_result(self) -> Result<Self, Vec<ServerError>> {
        if self.is_err() {
            Err(self.errors)
        } else {
            Ok(self)
        }
    }
}
#[cfg(test)]
#[path = "../../tests/plain-graphql/unit/response_tests.rs"]
mod tests;
