use crate::{InputType, Pos, Value, parser};
use serde::{Deserialize, Serialize};
use std::{
    any::Any,
    collections::BTreeMap,
    fmt::{self, Debug, Display, Formatter},
    marker::PhantomData,
    sync::Arc,
};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct ErrorExtensionValues(BTreeMap<String, Value>);
impl ErrorExtensionValues {
    pub fn set(&mut self, name: impl AsRef<str>, value: impl Into<Value>) {
        self.0.insert(name.as_ref().to_string(), value.into());
    }
    pub fn unset(&mut self, name: impl AsRef<str>) {
        self.0.remove(name.as_ref());
    }
    pub fn get(&self, name: impl AsRef<str>) -> Option<&Value> {
        self.0.get(name.as_ref())
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ServerError {
    pub message: String,
    #[serde(skip)]
    pub source: Option<Arc<dyn Any + Send + Sync>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub locations: Vec<Pos>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub path: Vec<PathSegment>,
    #[serde(skip_serializing_if = "error_extensions_is_empty", default)]
    pub extensions: Option<ErrorExtensionValues>,
}
fn error_extensions_is_empty(values: &Option<ErrorExtensionValues>) -> bool {
    values.as_ref().is_none_or(|values| values.0.is_empty())
}
impl Debug for ServerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerError")
            .field("message", &self.message)
            .field("locations", &self.locations)
            .field("path", &self.path)
            .field("extensions", &self.extensions)
            .finish()
    }
}
impl PartialEq for ServerError {
    fn eq(&self, other: &Self) -> bool {
        self.message.eq(&other.message)
            && self.locations.eq(&other.locations)
            && self.path.eq(&other.path)
            && self.extensions.eq(&other.extensions)
    }
}
impl ServerError {
    pub fn new(message: impl Into<String>, pos: Option<Pos>) -> Self {
        Self {
            message: message.into(),
            source: None,
            locations: pos.map(|pos| vec![pos]).unwrap_or_default(),
            path: Vec::new(),
            extensions: None,
        }
    }
    pub fn source<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.source.as_ref().map(|err| err.downcast_ref()).flatten()
    }
    #[doc(hidden)]
    #[must_use]
    pub fn with_path(self, path: Vec<PathSegment>) -> Self {
        Self { path, ..self }
    }
}
impl Display for ServerError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl From<ServerError> for Vec<ServerError> {
    fn from(single: ServerError) -> Self {
        vec![single]
    }
}
impl From<parser::Error> for ServerError {
    fn from(e: parser::Error) -> Self {
        Self {
            message: e.to_string(),
            source: None,
            locations: e.positions().collect(),
            path: Vec::new(),
            extensions: None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PathSegment {
    Field(String),
    Index(usize),
}
pub type ServerResult<T> = std::result::Result<T, ServerError>;
#[derive(Debug)]
pub struct InputValueError<T> {
    message: String,
    extensions: Option<ErrorExtensionValues>,
    phantom: PhantomData<T>,
}
impl<T: InputType> InputValueError<T> {
    fn new(message: String, extensions: Option<ErrorExtensionValues>) -> Self {
        Self {
            message,
            extensions,
            phantom: PhantomData,
        }
    }
    #[must_use]
    pub fn expected_type(actual: Value) -> Self {
        Self::new(
            format!(
                r#"Expected input type "{}", found {}."#,
                T::type_name(),
                actual
            ),
            None,
        )
    }
    #[must_use]
    pub fn custom(msg: impl Display) -> Self {
        Self::new(
            format!(r#"Failed to parse "{}": {}"#, T::type_name(), msg),
            None,
        )
    }
    pub fn propagate<U: InputType>(self) -> InputValueError<U> {
        if T::type_name() != U::type_name() {
            InputValueError::new(
                format!(
                    r#"{} (occurred while parsing "{}")"#,
                    self.message,
                    U::type_name()
                ),
                self.extensions,
            )
        } else {
            InputValueError::new(self.message, self.extensions)
        }
    }
    pub fn with_extension(mut self, name: impl AsRef<str>, value: impl Into<Value>) -> Self {
        self.extensions
            .get_or_insert_with(ErrorExtensionValues::default)
            .set(name, value);
        self
    }
    pub fn into_server_error(self, pos: Pos) -> ServerError {
        let mut err = ServerError::new(self.message, Some(pos));
        err.extensions = self.extensions;
        err
    }
}
impl<T: InputType, E: Display> From<E> for InputValueError<T> {
    fn from(error: E) -> Self {
        Self::custom(error)
    }
}
pub type InputValueResult<T> = Result<T, InputValueError<T>>;
#[derive(Clone, Serialize)]
pub struct Error {
    pub message: String,
    #[serde(skip)]
    pub source: Option<Arc<dyn Any + Send + Sync>>,
    #[serde(skip_serializing_if = "error_extensions_is_empty")]
    pub extensions: Option<ErrorExtensionValues>,
}
impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("message", &self.message)
            .field("extensions", &self.extensions)
            .finish()
    }
}
impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        self.message.eq(&other.message) && self.extensions.eq(&other.extensions)
    }
}
impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
            extensions: None,
        }
    }
    pub fn new_with_source(source: impl Display + Send + Sync + 'static) -> Self {
        Self {
            message: source.to_string(),
            source: Some(Arc::new(source)),
            extensions: None,
        }
    }
    #[must_use]
    pub fn into_server_error(self, pos: Pos) -> ServerError {
        ServerError {
            message: self.message,
            source: self.source,
            locations: vec![pos],
            path: Vec::new(),
            extensions: self.extensions,
        }
    }
}
impl<T: Display + Send + Sync + 'static> From<T> for Error {
    fn from(e: T) -> Self {
        Self {
            message: e.to_string(),
            source: Some(Arc::new(e)),
            extensions: None,
        }
    }
}
pub type Result<T, E = Error> = std::result::Result<T, E>;
pub trait ErrorExtensions: Sized {
    fn extend(&self) -> Error;
    fn extend_with<C>(self, cb: C) -> Error
    where
        C: FnOnce(&Self, &mut ErrorExtensionValues),
    {
        let mut new_extensions = Default::default();
        cb(&self, &mut new_extensions);
        let Error {
            message,
            source,
            extensions,
        } = self.extend();
        let mut extensions = extensions.unwrap_or_default();
        extensions.0.extend(new_extensions.0);
        Error {
            message,
            source,
            extensions: Some(extensions),
        }
    }
}
impl ErrorExtensions for Error {
    fn extend(&self) -> Error {
        self.clone()
    }
}
impl<E: Display> ErrorExtensions for &E {
    fn extend(&self) -> Error {
        Error {
            message: self.to_string(),
            source: None,
            extensions: None,
        }
    }
}
pub trait ResultExt<T, E>: Sized {
    fn extend_err<C>(self, cb: C) -> Result<T>
    where
        C: FnOnce(&E, &mut ErrorExtensionValues);
    fn extend(self) -> Result<T>;
}
impl<T, E> ResultExt<T, E> for std::result::Result<T, E>
where
    E: ErrorExtensions + Send + Sync + 'static,
{
    fn extend_err<C>(self, cb: C) -> Result<T>
    where
        C: FnOnce(&E, &mut ErrorExtensionValues),
    {
        match self {
            Err(err) => Err(err.extend_with(|e, ee| cb(e, ee))),
            Ok(value) => Ok(value),
        }
    }
    fn extend(self) -> Result<T> {
        match self {
            Err(err) => Err(err.extend()),
            Ok(value) => Ok(value),
        }
    }
}
