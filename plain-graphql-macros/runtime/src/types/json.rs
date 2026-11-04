use crate::{
    ContextSelectionSet, InputType, InputValueResult, OutputType, Positioned, ServerResult, Value,
    from_value,
    parser::types::Field,
    registry::{MetaType, MetaTypeId, Registry},
    to_value,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};
#[doc = " A scalar that can represent any JSON value."]
#[doc = ""]
#[doc = " If the inner type cannot be serialized as JSON (e.g. it has non-string keys)"]
#[doc = " it will be `null`."]
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash, Default)]
#[serde(transparent)]
pub struct Json<T>(pub T);
impl<T> Deref for Json<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> DerefMut for Json<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<T> From<T> for Json<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}
impl<T: DeserializeOwned + Serialize + Send + Sync> InputType for Json<T> {
    type RawValueType = T;
    fn type_name() -> Cow<'static, str> {
        Cow::Borrowed("JSON")
    }
    fn create_type_info(registry: &mut Registry) -> String {
        registry.create_input_type::<Json<T>, _>(MetaTypeId::Scalar, |_| MetaType::Scalar {
            name: <Self as InputType>::type_name().to_string(),
            description: Some("A scalar that can represent any JSON value.".to_string()),
            is_valid: None,

            specified_by_url: None,
        })
    }
    fn parse(value: Option<Value>) -> InputValueResult<Self> {
        Ok(from_value(value.unwrap_or_default())?)
    }
    fn to_value(&self) -> Value {
        Value::String(serde_json::to_string(&self.0).unwrap_or_default())
    }
    fn as_raw_value(&self) -> Option<&Self::RawValueType> {
        Some(&self.0)
    }
}
impl<T: Serialize + Send + Sync> OutputType for Json<T> {
    fn type_name() -> Cow<'static, str> {
        Cow::Borrowed("JSON")
    }
    fn create_type_info(registry: &mut Registry) -> String {
        registry.create_output_type::<Json<T>, _>(MetaTypeId::Scalar, |_| MetaType::Scalar {
            name: <Self as OutputType>::type_name().to_string(),
            description: Some("A scalar that can represent any JSON value.".to_string()),
            is_valid: None,

            specified_by_url: None,
        })
    }
    async fn resolve(
        &self,
        _ctx: &ContextSelectionSet<'_>,
        _field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        Ok(to_value(&self.0).ok().unwrap_or_default())
    }
}
impl InputType for serde_json::Value {
    type RawValueType = serde_json::Value;
    fn type_name() -> Cow<'static, str> {
        Cow::Borrowed("JSON")
    }
    fn create_type_info(registry: &mut Registry) -> String {
        registry.create_input_type::<serde_json::Value, _>(MetaTypeId::Scalar, |_| {
            MetaType::Scalar {
                name: <Self as InputType>::type_name().to_string(),
                description: Some("A scalar that can represent any JSON value.".to_string()),
                is_valid: None,

                specified_by_url: None,
            }
        })
    }
    fn parse(value: Option<Value>) -> InputValueResult<Self> {
        Ok(from_value(value.unwrap_or_default())?)
    }
    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
    fn as_raw_value(&self) -> Option<&Self::RawValueType> {
        Some(&self)
    }
}
impl OutputType for serde_json::Value {
    fn type_name() -> Cow<'static, str> {
        Cow::Borrowed("JSON")
    }
    fn create_type_info(registry: &mut Registry) -> String {
        registry.create_output_type::<serde_json::Value, _>(MetaTypeId::Scalar, |_| {
            MetaType::Scalar {
                name: <Self as OutputType>::type_name().to_string(),
                description: Some("A scalar that can represent any JSON value.".to_string()),
                is_valid: None,

                specified_by_url: None,
            }
        })
    }
    async fn resolve(
        &self,
        _ctx: &ContextSelectionSet<'_>,
        _field: &Positioned<Field>,
    ) -> ServerResult<Value> {
        Ok(to_value(self).ok().unwrap_or_default())
    }
}
#[cfg(test)]
#[path = "../../../../tests/plain-graphql/unit/types/json_test.rs"]
mod test;
