#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Instant(pub plain_libs::chrono::DateTime<plain_libs::chrono::Utc>);

#[cfg(feature = "graphql")]
#[plain_graphql::Scalar]
impl plain_graphql::ScalarType for Instant {
    fn parse(value: plain_graphql::Value) -> plain_graphql::InputValueResult<Self> {
        if let plain_graphql::Value::String(s) = &value
            && let Ok(dt) = plain_libs::chrono::DateTime::parse_from_rfc3339(s)
        {
            return Ok(Instant(dt.with_timezone(&plain_libs::chrono::Utc)));
        }
        Err(plain_graphql::InputValueError::expected_type(value))
    }

    fn to_value(&self) -> plain_graphql::Value {
        plain_graphql::Value::String(self.wire())
    }
}

impl Instant {
    fn wire(&self) -> String {
        self.0.to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true)
    }
}
impl serde::Serialize for Instant {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.wire())
    }
}

impl<'de> serde::Deserialize<'de> for Instant {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        plain_libs::chrono::DateTime::parse_from_rfc3339(&value)
            .map(|value| Self(value.with_timezone(&plain_libs::chrono::Utc)))
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Long(pub i64);

#[cfg(feature = "graphql")]
#[plain_graphql::Scalar]
impl plain_graphql::ScalarType for Long {
    fn parse(value: plain_graphql::Value) -> plain_graphql::InputValueResult<Self> {
        if let plain_graphql::Value::Number(n) = &value
            && let Some(i) = n.as_i64()
        {
            return Ok(Long(i));
        }
        Err(plain_graphql::InputValueError::expected_type(value))
    }

    fn to_value(&self) -> plain_graphql::Value {
        plain_graphql::Value::from(self.0)
    }
}
