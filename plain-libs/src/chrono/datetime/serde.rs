use super::DateTime;
use crate::chrono::format::{SecondsFormat, write_rfc3339};
use crate::chrono::offset::{FixedOffset, Offset, TimeZone, Utc};
use core::fmt;
use serde::{de, ser};
#[derive(Debug)]
pub struct SecondsTimestampVisitor;
#[derive(Debug)]
pub struct NanoSecondsTimestampVisitor;
#[derive(Debug)]
pub struct MicroSecondsTimestampVisitor;
#[derive(Debug)]
pub struct MilliSecondsTimestampVisitor;
impl<Tz: TimeZone> ser::Serialize for DateTime<Tz> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        struct FormatIso8601<'a, Tz: TimeZone> {
            inner: &'a DateTime<Tz>,
        }
        impl<Tz: TimeZone> fmt::Display for FormatIso8601<'_, Tz> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let naive = self.inner.naive_local();
                let offset = self.inner.offset.fix();
                write_rfc3339(f, naive, offset, SecondsFormat::AutoSi, true)
            }
        }
        serializer.collect_str(&FormatIso8601 { inner: self })
    }
}
struct DateTimeVisitor;
impl de::Visitor<'_> for DateTimeVisitor {
    type Value = DateTime<FixedOffset>;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an RFC 3339 formatted date and time string")
    }
    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}
impl<'de> de::Deserialize<'de> for DateTime<FixedOffset> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_str(DateTimeVisitor)
    }
}
impl<'de> de::Deserialize<'de> for DateTime<Utc> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer
            .deserialize_str(DateTimeVisitor)
            .map(|dt| dt.with_timezone(&Utc))
    }
}
pub mod ts_nanoseconds {
    use super::NanoSecondsTimestampVisitor;
    use crate::chrono::serde::invalid_ts;
    use crate::chrono::{DateTime, Utc};
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(dt.timestamp_nanos_opt().ok_or(ser::Error::custom(
            "value out of range for a timestamp with nanosecond precision",
        ))?)
    }
    pub fn deserialize<'de, D>(d: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(NanoSecondsTimestampVisitor)
    }
    impl de::Visitor<'_> for NanoSecondsTimestampVisitor {
        type Value = DateTime<Utc>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in nanoseconds")
        }
        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(DateTime::from_timestamp_nanos(value))
        }
        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp(
                (value / 1_000_000_000) as i64,
                (value % 1_000_000_000) as u32,
            )
            .ok_or_else(|| invalid_ts(value))
        }
    }
}
pub mod ts_nanoseconds_option {
    use super::NanoSecondsTimestampVisitor;
    use crate::chrono::{DateTime, Utc};
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(opt: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.timestamp_nanos_opt().ok_or(
                ser::Error::custom("value out of range for a timestamp with nanosecond precision"),
            )?),
            None => serializer.serialize_none(),
        }
    }
    pub fn deserialize<'de, D>(d: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionNanoSecondsTimestampVisitor)
    }
    struct OptionNanoSecondsTimestampVisitor;
    impl<'de> de::Visitor<'de> for OptionNanoSecondsTimestampVisitor {
        type Value = Option<DateTime<Utc>>;
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
    use super::MicroSecondsTimestampVisitor;
    use crate::chrono::serde::invalid_ts;
    use crate::chrono::{DateTime, Utc};
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(dt.timestamp_micros())
    }
    pub fn deserialize<'de, D>(d: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(MicroSecondsTimestampVisitor)
    }
    impl de::Visitor<'_> for MicroSecondsTimestampVisitor {
        type Value = DateTime<Utc>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in microseconds")
        }
        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp_micros(value).ok_or_else(|| invalid_ts(value))
        }
        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp(
                (value / 1_000_000) as i64,
                ((value % 1_000_000) * 1_000) as u32,
            )
            .ok_or_else(|| invalid_ts(value))
        }
    }
}
pub mod ts_microseconds_option {
    use super::MicroSecondsTimestampVisitor;
    use crate::chrono::{DateTime, Utc};
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(opt: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.timestamp_micros()),
            None => serializer.serialize_none(),
        }
    }
    pub fn deserialize<'de, D>(d: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionMicroSecondsTimestampVisitor)
    }
    struct OptionMicroSecondsTimestampVisitor;
    impl<'de> de::Visitor<'de> for OptionMicroSecondsTimestampVisitor {
        type Value = Option<DateTime<Utc>>;
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
    use super::MilliSecondsTimestampVisitor;
    use crate::chrono::serde::invalid_ts;
    use crate::chrono::{DateTime, Utc};
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(dt.timestamp_millis())
    }
    pub fn deserialize<'de, D>(d: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(MilliSecondsTimestampVisitor)
            .map(|dt| dt.with_timezone(&Utc))
    }
    impl de::Visitor<'_> for MilliSecondsTimestampVisitor {
        type Value = DateTime<Utc>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in milliseconds")
        }
        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp_millis(value).ok_or_else(|| invalid_ts(value))
        }
        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp((value / 1000) as i64, ((value % 1000) * 1_000_000) as u32)
                .ok_or_else(|| invalid_ts(value))
        }
    }
}
pub mod ts_milliseconds_option {
    use super::MilliSecondsTimestampVisitor;
    use crate::chrono::{DateTime, Utc};
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(opt: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.timestamp_millis()),
            None => serializer.serialize_none(),
        }
    }
    pub fn deserialize<'de, D>(d: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionMilliSecondsTimestampVisitor)
            .map(|opt| opt.map(|dt| dt.with_timezone(&Utc)))
    }
    struct OptionMilliSecondsTimestampVisitor;
    impl<'de> de::Visitor<'de> for OptionMilliSecondsTimestampVisitor {
        type Value = Option<DateTime<Utc>>;
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
    use super::SecondsTimestampVisitor;
    use crate::chrono::serde::invalid_ts;
    use crate::chrono::{DateTime, Utc};
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(dt.timestamp())
    }
    pub fn deserialize<'de, D>(d: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(SecondsTimestampVisitor)
    }
    impl de::Visitor<'_> for SecondsTimestampVisitor {
        type Value = DateTime<Utc>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in seconds")
        }
        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp_secs(value).ok_or_else(|| invalid_ts(value))
        }
        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if value > i64::MAX as u64 {
                Err(invalid_ts(value))
            } else {
                DateTime::from_timestamp_secs(value as i64).ok_or_else(|| invalid_ts(value))
            }
        }
    }
}
pub mod ts_seconds_option {
    use super::SecondsTimestampVisitor;
    use crate::chrono::{DateTime, Utc};
    use core::fmt;
    use serde::{de, ser};
    pub fn serialize<S>(opt: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.timestamp()),
            None => serializer.serialize_none(),
        }
    }
    pub fn deserialize<'de, D>(d: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionSecondsTimestampVisitor)
    }
    struct OptionSecondsTimestampVisitor;
    impl<'de> de::Visitor<'de> for OptionSecondsTimestampVisitor {
        type Value = Option<DateTime<Utc>>;
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
