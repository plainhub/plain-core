use serde::de::{self, Deserializer, Unexpected, Visitor};
use std::fmt;

pub(super) fn deserialize<'de, D, T>(
    deserializer: D,
    values: &[(u64, &'static str, T)],
) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Copy,
{
    struct EnumVisitor<'a, T>(&'a [(u64, &'static str, T)]);

    impl<'de, T: Copy> Visitor<'de> for EnumVisitor<'_, T> {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a preference enum name or a supported integer value")
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<T, E> {
            self.0
                .iter()
                .find(|(_, name, _)| *name == value)
                .map(|(_, _, value)| *value)
                .ok_or_else(|| E::invalid_value(Unexpected::Str(value), &self))
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<T, E> {
            self.0
                .iter()
                .find(|(number, _, _)| *number == value)
                .map(|(_, _, value)| *value)
                .ok_or_else(|| E::invalid_value(Unexpected::Unsigned(value), &self))
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> Result<T, E> {
            match u64::try_from(value) {
                Ok(value) => self.visit_u64(value),
                Err(_) => Err(E::invalid_value(Unexpected::Signed(value), &self)),
            }
        }
    }

    deserializer.deserialize_any(EnumVisitor(values))
}

#[cfg(test)]
#[path = "../../../tests/unit/prefs/settings/enum_value.rs"]
mod tests;
