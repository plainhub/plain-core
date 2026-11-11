#![forbid(unsafe_code)]
extern crate self as plain_graphql;
mod base;

mod error;

mod guard;
mod look_ahead;
mod model;
mod request;
mod response;
mod schema;
mod subscription;
mod validation;

pub mod context;

pub mod resolver_utils;

pub mod types;

#[doc(hidden)]
pub mod registry;

pub mod parser;
mod value;

pub use base::{
    ComplexObject, InputObjectType, InputType, InterfaceType, ObjectType, OutputType, UnionType,
};
#[doc(hidden)]
pub use context::ContextSelectionSet;
pub use context::*;

pub use error::{
    Error, ErrorExtensionValues, ErrorExtensions, InputValueError, InputValueResult, PathSegment,
    Result, ResultExt, ServerError, ServerResult,
};

#[doc(hidden)]
pub use futures_util;
pub use guard::Guard;
#[doc(hidden)]
pub use plain_libs::indexmap;
pub use look_ahead::Lookahead;
#[doc(no_inline)]
pub use parser::{Pos, Positioned};
pub use registry::CacheControl;
pub use request::Request;
#[doc(no_inline)]
pub use resolver_utils::{ContainerType, EnumType, ScalarType};
pub use response::Response;
pub use schema::{IntrospectionMode, Schema, SchemaBuilder, SchemaEnv};
#[doc(hidden)]
pub use plain_libs::assertions as static_assertions_next;
pub use subscription::SubscriptionType;
pub use types::*;
pub use validation::{ValidationResult, VisitorContext};

pub use value::ConstValue;
pub use value::{
    ConstValue as Value, DeserializerError, Extensions, Name, Number, SerializerError, Variables,
    from_value, to_value,
};

pub use plain_graphql_macros::{
    Enum, InputObject, Interface, MergedObject, MergedSubscription, SimpleObject, Union,
    complex_object as ComplexObject, object as Object, scalar as Scalar,
    subscription as Subscription,
};
