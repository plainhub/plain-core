use crate::value::{ConstValue, Name};
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    collections::BTreeMap,
    fmt::{self, Display, Formatter},
    ops::{Deref, DerefMut},
};
#[derive(Debug, Clone, Default, Serialize, Eq, PartialEq)]
#[serde(transparent)]
pub struct Variables(BTreeMap<Name, ConstValue>);
impl Display for Variables {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("{")?;
        for (i, (name, value)) in self.0.iter().enumerate() {
            write!(f, "{}{}: {}", if i == 0 { "" } else { ", " }, name, value)?;
        }
        f.write_str("}")
    }
}
impl<'de> Deserialize<'de> for Variables {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self(
            <Option<BTreeMap<Name, ConstValue>>>::deserialize(deserializer)?.unwrap_or_default(),
        ))
    }
}
impl Variables {
    #[must_use]
    pub fn from_value(value: ConstValue) -> Self {
        match value {
            ConstValue::Object(obj) => Self(obj.into_iter().collect()),
            _ => Self::default(),
        }
    }
    #[must_use]
    pub fn from_json(value: plain_libs::serde_json::Value) -> Self {
        ConstValue::from_json(value)
            .map(Self::from_value)
            .unwrap_or_default()
    }
    #[must_use]
    pub fn into_value(self) -> ConstValue {
        ConstValue::Object(self.0.into_iter().collect())
    }
}
impl From<Variables> for ConstValue {
    fn from(variables: Variables) -> Self {
        variables.into_value()
    }
}
impl Deref for Variables {
    type Target = BTreeMap<Name, ConstValue>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Variables {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
