use crate::{InputType, InputValueError, InputValueResult, Name, Value};
pub struct EnumItem<T> {
    pub name: &'static str,
    pub value: T,
}
pub trait EnumType: Sized + Eq + Send + Copy + Sized + 'static {
    fn items() -> &'static [EnumItem<Self>];
}
pub fn parse_enum<T: EnumType + InputType>(value: Value) -> InputValueResult<T> {
    let value = match &value {
        Value::Enum(s) => s,
        Value::String(s) => s.as_str(),
        _ => return Err(InputValueError::expected_type(value)),
    };
    T::items()
        .iter()
        .find(|item| item.name == value)
        .map(|item| item.value)
        .ok_or_else(|| {
            InputValueError::custom(format_args!(
                r#"Enumeration type does not contain value "{}"."#,
                value,
            ))
        })
}
pub fn enum_value<T: EnumType>(value: T) -> Value {
    let item = T::items().iter().find(|item| item.value == value).unwrap();
    Value::Enum(Name::new(item.name))
}
