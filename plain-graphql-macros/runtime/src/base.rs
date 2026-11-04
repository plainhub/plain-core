use crate::value::ConstValue;
use crate::{
    ContainerType, Context, ContextSelectionSet, Error, InputValueError, InputValueResult,
    Positioned, Result, ServerResult, Value,
    parser::types::Field,
    registry::{self, Registry},
};
use std::future::Future;
use std::{borrow::Cow, sync::Arc};

pub trait InputType: Send + Sync + Sized {
    type RawValueType: ?Sized;
    fn type_name() -> Cow<'static, str>;
    fn qualified_type_name() -> String {
        format!("{}!", Self::type_name())
    }
    fn create_type_info(registry: &mut registry::Registry) -> String;
    fn parse(value: Option<Value>) -> InputValueResult<Self>;
    fn to_value(&self) -> Value;
    fn as_raw_value(&self) -> Option<&Self::RawValueType>;
}
pub trait OutputType: Send + Sync {
    fn type_name() -> Cow<'static, str>;
    fn qualified_type_name() -> String {
        format!("{}!", Self::type_name())
    }
    fn introspection_type_name(&self) -> Cow<'static, str> {
        Self::type_name()
    }
    fn create_type_info(registry: &mut registry::Registry) -> String;
    fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> impl Future<Output = ServerResult<Value>> + Send;
}
impl<T: OutputType + ?Sized> OutputType for &T {
    fn type_name() -> Cow<'static, str> {
        T::type_name()
    }
    fn create_type_info(registry: &mut Registry) -> String {
        T::create_type_info(registry)
    }
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        T::resolve(*self, ctx, field).await
    }
}
impl<T: OutputType + Sync, E: Into<Error> + Send + Sync + Clone> OutputType for Result<T, E> {
    fn type_name() -> Cow<'static, str> {
        T::type_name()
    }
    fn create_type_info(registry: &mut Registry) -> String {
        T::create_type_info(registry)
    }
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        match self {
            Ok(value) => value.resolve(ctx, field).await,
            Err(err) => Err(ctx.set_error_path(err.clone().into().into_server_error(field.pos))),
        }
    }
}
pub trait ObjectType: ContainerType {}
impl<T: ObjectType + ?Sized> ObjectType for &T {}
impl<T: ObjectType + ?Sized> ObjectType for Box<T> {}
impl<T: ObjectType + ?Sized> ObjectType for Arc<T> {}
pub trait InterfaceType: ContainerType {}
pub trait UnionType: ContainerType {}
pub trait InputObjectType: InputType {}

impl<T: OutputType + ?Sized> OutputType for Box<T> {
    fn type_name() -> Cow<'static, str> {
        T::type_name()
    }
    fn create_type_info(registry: &mut Registry) -> String {
        T::create_type_info(registry)
    }
    fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> impl Future<Output = ServerResult<Value>> + Send {
        T::resolve(self.as_ref(), ctx, field)
    }
}
impl<T: InputType> InputType for Box<T> {
    type RawValueType = T::RawValueType;
    fn type_name() -> Cow<'static, str> {
        T::type_name()
    }
    fn create_type_info(registry: &mut Registry) -> String {
        T::create_type_info(registry)
    }
    fn parse(value: Option<ConstValue>) -> InputValueResult<Self> {
        T::parse(value)
            .map(Box::new)
            .map_err(InputValueError::propagate)
    }
    fn to_value(&self) -> ConstValue {
        T::to_value(&self)
    }
    fn as_raw_value(&self) -> Option<&Self::RawValueType> {
        self.as_ref().as_raw_value()
    }
}
impl<T: OutputType + ?Sized> OutputType for Arc<T> {
    fn type_name() -> Cow<'static, str> {
        T::type_name()
    }
    fn create_type_info(registry: &mut Registry) -> String {
        T::create_type_info(registry)
    }
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        T::resolve(&**self, ctx, field).await
    }
}
impl<T: InputType> InputType for Arc<T> {
    type RawValueType = T::RawValueType;
    fn type_name() -> Cow<'static, str> {
        T::type_name()
    }
    fn create_type_info(registry: &mut Registry) -> String {
        T::create_type_info(registry)
    }
    fn parse(value: Option<ConstValue>) -> InputValueResult<Self> {
        T::parse(value)
            .map(Arc::new)
            .map_err(InputValueError::propagate)
    }
    fn to_value(&self) -> ConstValue {
        T::to_value(&self)
    }
    fn as_raw_value(&self) -> Option<&Self::RawValueType> {
        self.as_ref().as_raw_value()
    }
}

#[doc(hidden)]
pub trait ComplexObject {
    fn fields(registry: &mut registry::Registry) -> Vec<(String, registry::MetaField)>;
    fn resolve_field(
        &self,
        ctx: &Context<'_>,
    ) -> impl Future<Output = ServerResult<Option<Value>>> + Send;
}
