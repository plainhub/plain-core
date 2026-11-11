use super::NaiveDateTime;
use core::fmt;
use serde::{de, ser};
impl ser::Serialize for NaiveDateTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        struct FormatWrapped<'a, D: 'a> {
            inner: &'a D,
        }
        impl<D: fmt::Debug> fmt::Display for FormatWrapped<'_, D> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                self.inner.fmt(f)
            }
        }
        serializer.collect_str(&FormatWrapped { inner: &self })
    }
}
struct NaiveDateTimeVisitor;
impl de::Visitor<'_> for NaiveDateTimeVisitor {
    type Value = NaiveDateTime;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a formatted date and time string")
    }
    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}
impl<'de> de::Deserialize<'de> for NaiveDateTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_str(NaiveDateTimeVisitor)
    }
}
pub mod ts_nanoseconds {
    use crate::chrono::serde::invalid_ts;
    use crate::chrono::{DateTime, NaiveDateTime};
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(dt: &NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(
            dt.and_utc()
                .timestamp_nanos_opt()
                .ok_or(ser::Error::custom(
                    "value out of range for a timestamp with nanosecond precision",
                ))?,
        )
    }
    pub fn deserialize<'de, D>(d: D) -> Result<NaiveDateTime, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(NanoSecondsTimestampVisitor)
    }
    pub(super) struct NanoSecondsTimestampVisitor;
    impl de::Visitor<'_> for NanoSecondsTimestampVisitor {
        type Value = NaiveDateTime;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp")
        }
        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp(
                value.div_euclid(1_000_000_000),
                (value.rem_euclid(1_000_000_000)) as u32,
            )
            .map(|dt| dt.naive_utc())
            .ok_or_else(|| invalid_ts(value))
        }
        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp(
                (value / 1_000_000_000) as i64,
                (value % 1_000_000_000) as u32,
            )
            .map(|dt| dt.naive_utc())
            .ok_or_else(|| invalid_ts(value))
        }
    }
}
pub mod ts_nanoseconds_option {
    use super::ts_nanoseconds::NanoSecondsTimestampVisitor;
    use crate::chrono::NaiveDateTime;
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(opt: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.and_utc().timestamp_nanos_opt().ok_or(
                ser::Error::custom("value out of range for a timestamp with nanosecond precision"),
            )?),
            None => serializer.serialize_none(),
        }
    }
    pub fn deserialize<'de, D>(d: D) -> Result<Option<NaiveDateTime>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionNanoSecondsTimestampVisitor)
    }
    struct OptionNanoSecondsTimestampVisitor;
    impl<'de> de::Visitor<'de> for OptionNanoSecondsTimestampVisitor {
        type Value = Option<NaiveDateTime>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in nanoseconds or none")
        }
        fn visit_some<D>(self, d: D) -> Result<Self::Value, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            d.deserialize_i64(NanoSecondsTimestampVisitor).map(Some)
        }
        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }
}
pub mod ts_microseconds {
    use crate::chrono::serde::invalid_ts;
    use crate::chrono::{DateTime, NaiveDateTime};
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(dt: &NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(dt.and_utc().timestamp_micros())
    }
    pub fn deserialize<'de, D>(d: D) -> Result<NaiveDateTime, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(MicroSecondsTimestampVisitor)
    }
    pub(super) struct MicroSecondsTimestampVisitor;
    impl de::Visitor<'_> for MicroSecondsTimestampVisitor {
        type Value = NaiveDateTime;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp")
        }
        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp_micros(value)
                .map(|dt| dt.naive_utc())
                .ok_or_else(|| invalid_ts(value))
        }
        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp(
                (value / 1_000_000) as i64,
                ((value % 1_000_000) * 1_000) as u32,
            )
            .map(|dt| dt.naive_utc())
            .ok_or_else(|| invalid_ts(value))
        }
    }
}
pub mod ts_microseconds_option {
    use super::ts_microseconds::MicroSecondsTimestampVisitor;
    use crate::chrono::NaiveDateTime;
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(opt: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.and_utc().timestamp_micros()),
            None => serializer.serialize_none(),
        }
    }
    pub fn deserialize<'de, D>(d: D) -> Result<Option<NaiveDateTime>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionMicroSecondsTimestampVisitor)
    }
    struct OptionMicroSecondsTimestampVisitor;
    impl<'de> de::Visitor<'de> for OptionMicroSecondsTimestampVisitor {
        type Value = Option<NaiveDateTime>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in microseconds or none")
        }
        fn visit_some<D>(self, d: D) -> Result<Self::Value, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            d.deserialize_i64(MicroSecondsTimestampVisitor).map(Some)
        }
        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }
}
pub mod ts_milliseconds {
    use crate::chrono::serde::invalid_ts;
    use crate::chrono::{DateTime, NaiveDateTime};
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(dt: &NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(dt.and_utc().timestamp_millis())
    }
    pub fn deserialize<'de, D>(d: D) -> Result<NaiveDateTime, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(MilliSecondsTimestampVisitor)
    }
    pub(super) struct MilliSecondsTimestampVisitor;
    impl de::Visitor<'_> for MilliSecondsTimestampVisitor {
        type Value = NaiveDateTime;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp")
        }
        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp_millis(value)
                .map(|dt| dt.naive_utc())
                .ok_or_else(|| invalid_ts(value))
        }
        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp((value / 1000) as i64, ((value % 1000) * 1_000_000) as u32)
                .map(|dt| dt.naive_utc())
                .ok_or_else(|| invalid_ts(value))
        }
    }
}
pub mod ts_milliseconds_option {
    use super::ts_milliseconds::MilliSecondsTimestampVisitor;
    use crate::chrono::NaiveDateTime;
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(opt: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.and_utc().timestamp_millis()),
            None => serializer.serialize_none(),
        }
    }
    pub fn deserialize<'de, D>(d: D) -> Result<Option<NaiveDateTime>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionMilliSecondsTimestampVisitor)
    }
    struct OptionMilliSecondsTimestampVisitor;
    impl<'de> de::Visitor<'de> for OptionMilliSecondsTimestampVisitor {
        type Value = Option<NaiveDateTime>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in milliseconds or none")
        }
        fn visit_some<D>(self, d: D) -> Result<Self::Value, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            d.deserialize_i64(MilliSecondsTimestampVisitor).map(Some)
        }
        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }
}
pub mod ts_seconds {
    use crate::chrono::serde::invalid_ts;
    use crate::chrono::{DateTime, NaiveDateTime};
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(dt: &NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(dt.and_utc().timestamp())
    }
    pub fn deserialize<'de, D>(d: D) -> Result<NaiveDateTime, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(SecondsTimestampVisitor)
    }
    pub(super) struct SecondsTimestampVisitor;
    impl de::Visitor<'_> for SecondsTimestampVisitor {
        type Value = NaiveDateTime;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp")
        }
        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp_secs(value)
                .map(|dt| dt.naive_utc())
                .ok_or_else(|| invalid_ts(value))
        }
        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if value > i64::MAX as u64 {
                Err(invalid_ts(value))
            } else {
                DateTime::from_timestamp_secs(value as i64)
                    .map(|dt| dt.naive_utc())
                    .ok_or_else(|| invalid_ts(value))
            }
        }
    }
}
pub mod ts_seconds_option {
    use super::ts_seconds::SecondsTimestampVisitor;
    use crate::chrono::NaiveDateTime;
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(opt: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.and_utc().timestamp()),
            None => serializer.serialize_none(),
        }
    }
    pub fn deserialize<'de, D>(d: D) -> Result<Option<NaiveDateTime>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionSecondsTimestampVisitor)
    }
    struct OptionSecondsTimestampVisitor;
    impl<'de> de::Visitor<'de> for OptionSecondsTimestampVisitor {
        type Value = Option<NaiveDateTime>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in seconds or none")
        }
        fn visit_some<D>(self, d: D) -> Result<Self::Value, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            d.deserialize_i64(SecondsTimestampVisitor).map(Some)
        }
        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }
}
