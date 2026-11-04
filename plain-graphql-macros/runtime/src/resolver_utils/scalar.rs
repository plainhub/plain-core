use crate::{InputValueResult, Value};
pub trait ScalarType: Sized + Send {
    fn parse(value: Value) -> InputValueResult<Self>;
    fn is_valid(_value: &Value) -> bool {
        true
    }
    fn to_value(&self) -> Value;
}
#[macro_export]
macro_rules! scalar {
    ($ ty : ty , $ name : literal , $ desc : literal , $ specified_by_url : literal) => {
        $crate::scalar_internal!(
            $ty,
            $name,
            ::std::option::Option::Some(::std::string::ToString::to_string($desc)),
            ::std::option::Option::Some(::std::string::ToString::to_string($specified_by_url))
        );
    };
    ($ ty : ty , $ name : literal , $ desc : literal) => {
        $crate::scalar_internal!(
            $ty,
            $name,
            ::std::option::Option::Some(::std::string::ToString::to_string($desc)),
            ::std::option::Option::None
        );
    };
    ($ ty : ty , $ name : literal) => {
        $crate::scalar_internal!(
            $ty,
            $name,
            ::std::option::Option::None,
            ::std::option::Option::None
        );
    };
    ($ ty : ty) => {
        $crate::scalar_internal!(
            $ty,
            ::std::stringify!($ty),
            ::std::option::Option::None,
            ::std::option::Option::None
        );
    };
}
#[macro_export]
#[doc(hidden)]
macro_rules! scalar_internal {
    ($ ty : ty , $ name : expr , $ desc : expr , $ specified_by_url : expr) => {
        impl $crate::ScalarType for $ty {
            fn parse(value: $crate::Value) -> $crate::InputValueResult<Self> {
                ::std::result::Result::Ok($crate::from_value(value)?)
            }
            fn to_value(&self) -> $crate::Value {
                $crate::to_value(self).unwrap_or_else(|_| $crate::Value::Null)
            }
        }
        impl $crate::InputType for $ty {
            type RawValueType = Self;
            fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                ::std::borrow::Cow::Borrowed($name)
            }
            fn create_type_info(
                registry: &mut $crate::registry::Registry,
            ) -> ::std::string::String {
                registry.create_input_type::<$ty, _>($crate::registry::MetaTypeId::Scalar, |_| {
                    $crate::registry::MetaType::Scalar {
                        name: ::std::borrow::ToOwned::to_owned($name),
                        description: $desc,
                        is_valid: ::std::option::Option::Some(::std::sync::Arc::new(|value| {
                            <$ty as $crate::ScalarType>::is_valid(value)
                        })),

                        inaccessible: false,
                        tags: ::std::default::Default::default(),
                        specified_by_url: $specified_by_url,
                        directive_invocations: ::std::vec::Vec::new(),
                        requires_scopes: ::std::vec::Vec::new(),
                    }
                })
            }
            fn parse(
                value: ::std::option::Option<$crate::Value>,
            ) -> $crate::InputValueResult<Self> {
                <$ty as $crate::ScalarType>::parse(value.unwrap_or_default())
            }
            fn to_value(&self) -> $crate::Value {
                <$ty as $crate::ScalarType>::to_value(self)
            }
            fn as_raw_value(&self) -> ::std::option::Option<&Self::RawValueType> {
                ::std::option::Option::Some(self)
            }
        }
        $crate::scalar_internal_output!($ty, $name, $desc, $specified_by_url);
    };
}
#[macro_export]
#[doc(hidden)]
macro_rules! scalar_internal_output {
    ($ ty : ty , $ name : expr , $ desc : expr , $ specified_by_url : expr) => {
        impl $crate::OutputType for $ty {
            fn type_name() -> ::std::borrow::Cow<'static, ::std::primitive::str> {
                ::std::borrow::Cow::Borrowed($name)
            }
            fn create_type_info(
                registry: &mut $crate::registry::Registry,
            ) -> ::std::string::String {
                registry.create_output_type::<$ty, _>($crate::registry::MetaTypeId::Scalar, |_| {
                    $crate::registry::MetaType::Scalar {
                        name: ::std::borrow::ToOwned::to_owned($name),
                        description: $desc,
                        is_valid: ::std::option::Option::Some(::std::sync::Arc::new(|value| {
                            <$ty as $crate::ScalarType>::is_valid(value)
                        })),

                        inaccessible: false,
                        tags: ::std::default::Default::default(),
                        specified_by_url: $specified_by_url,
                        directive_invocations: ::std::vec::Vec::new(),
                        requires_scopes: ::std::vec::Vec::new(),
                    }
                })
            }
            async fn resolve(
                &self,
                _: &$crate::ContextSelectionSet<'_>,
                _field: &$crate::Positioned<$crate::parser::types::Field>,
            ) -> $crate::ServerResult<$crate::Value> {
                ::std::result::Result::Ok($crate::ScalarType::to_value(self))
            }
        }
    };
}
