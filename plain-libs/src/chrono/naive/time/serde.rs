use super::NaiveTime;
use core::fmt;
use serde::{de, ser};
impl ser::Serialize for NaiveTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.collect_str(&self)
    }
}
struct NaiveTimeVisitor;
impl de::Visitor<'_> for NaiveTimeVisitor {
    type Value = NaiveTime;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a formatted time string")
    }
    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}
impl<'de> de::Deserialize<'de> for NaiveTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_str(NaiveTimeVisitor)
    }
}
