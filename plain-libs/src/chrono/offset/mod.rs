use crate::chrono::Weekday;
use crate::chrono::format::{ParseResult, Parsed, StrftimeItems, parse};
use crate::chrono::naive::{NaiveDate, NaiveDateTime, NaiveTime};
use crate::chrono::{Date, DateTime};
use core::fmt;
pub(crate) mod fixed;
pub use self::fixed::FixedOffset;
pub(crate) mod utc;
pub use self::utc::Utc;
pub type MappedLocalTime<T> = LocalResult<T>;
#[derive(Clone, PartialEq, Debug, Copy, Eq, Hash)]
#[cfg_attr(any(), derive(defmt::Format))]
pub enum LocalResult<T> {
    Single(T),
    Ambiguous(T, T),
    None,
}
impl<T> MappedLocalTime<T> {
    #[must_use]
    pub fn single(self) -> Option<T> {
        match self {
            MappedLocalTime::Single(t) => Some(t),
            _ => None,
        }
    }
    #[must_use]
    pub fn earliest(self) -> Option<T> {
        match self {
            MappedLocalTime::Single(t) | MappedLocalTime::Ambiguous(t, _) => Some(t),
            _ => None,
        }
    }
    #[must_use]
    pub fn latest(self) -> Option<T> {
        match self {
            MappedLocalTime::Single(t) | MappedLocalTime::Ambiguous(_, t) => Some(t),
            _ => None,
        }
    }
    #[must_use]
    pub fn map<U, F: FnMut(T) -> U>(self, mut f: F) -> MappedLocalTime<U> {
        match self {
            MappedLocalTime::None => MappedLocalTime::None,
            MappedLocalTime::Single(v) => MappedLocalTime::Single(f(v)),
            MappedLocalTime::Ambiguous(min, max) => MappedLocalTime::Ambiguous(f(min), f(max)),
        }
    }
    #[must_use]
    pub(crate) fn and_then<U, F: FnMut(T) -> Option<U>>(self, mut f: F) -> MappedLocalTime<U> {
        match self {
            MappedLocalTime::None => MappedLocalTime::None,
            MappedLocalTime::Single(v) => match f(v) {
                Some(new) => MappedLocalTime::Single(new),
                None => MappedLocalTime::None,
            },
            MappedLocalTime::Ambiguous(min, max) => match (f(min), f(max)) {
                (Some(min), Some(max)) => MappedLocalTime::Ambiguous(min, max),
                _ => MappedLocalTime::None,
            },
        }
    }
}
impl<Tz: TimeZone> MappedLocalTime<Date<Tz>> {
    #[inline]
    #[must_use]
    pub fn and_time(self, time: NaiveTime) -> MappedLocalTime<DateTime<Tz>> {
        match self {
            MappedLocalTime::Single(d) => d
                .and_time(time)
                .map_or(MappedLocalTime::None, MappedLocalTime::Single),
            _ => MappedLocalTime::None,
        }
    }
    #[inline]
    #[must_use]
    pub fn and_hms_opt(self, hour: u32, min: u32, sec: u32) -> MappedLocalTime<DateTime<Tz>> {
        match self {
            MappedLocalTime::Single(d) => d
                .and_hms_opt(hour, min, sec)
                .map_or(MappedLocalTime::None, MappedLocalTime::Single),
            _ => MappedLocalTime::None,
        }
    }
    #[inline]
    #[must_use]
    pub fn and_hms_milli_opt(
        self,
        hour: u32,
        min: u32,
        sec: u32,
        milli: u32,
    ) -> MappedLocalTime<DateTime<Tz>> {
        match self {
            MappedLocalTime::Single(d) => d
                .and_hms_milli_opt(hour, min, sec, milli)
                .map_or(MappedLocalTime::None, MappedLocalTime::Single),
            _ => MappedLocalTime::None,
        }
    }
    #[inline]
    #[must_use]
    pub fn and_hms_micro_opt(
        self,
        hour: u32,
        min: u32,
        sec: u32,
        micro: u32,
    ) -> MappedLocalTime<DateTime<Tz>> {
        match self {
            MappedLocalTime::Single(d) => d
                .and_hms_micro_opt(hour, min, sec, micro)
                .map_or(MappedLocalTime::None, MappedLocalTime::Single),
            _ => MappedLocalTime::None,
        }
    }
    #[inline]
    #[must_use]
    pub fn and_hms_nano_opt(
        self,
        hour: u32,
        min: u32,
        sec: u32,
        nano: u32,
    ) -> MappedLocalTime<DateTime<Tz>> {
        match self {
            MappedLocalTime::Single(d) => d
                .and_hms_nano_opt(hour, min, sec, nano)
                .map_or(MappedLocalTime::None, MappedLocalTime::Single),
            _ => MappedLocalTime::None,
        }
    }
}
impl<T: fmt::Debug> MappedLocalTime<T> {
    #[must_use]
    #[track_caller]
    pub fn unwrap(self) -> T {
        match self {
            MappedLocalTime::None => panic!("No such local time"),
            MappedLocalTime::Single(t) => t,
            MappedLocalTime::Ambiguous(t1, t2) => {
                panic!("Ambiguous local time, ranging from {t1:?} to {t2:?}")
            }
        }
    }
}
pub trait Offset: Sized + Clone + fmt::Debug {
    fn fix(&self) -> FixedOffset;
}
pub trait TimeZone: Sized + Clone {
    type Offset: Offset;
    fn with_ymd_and_hms(
        &self,
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        min: u32,
        sec: u32,
    ) -> MappedLocalTime<DateTime<Self>> {
        match NaiveDate::from_ymd_opt(year, month, day).and_then(|d| d.and_hms_opt(hour, min, sec))
        {
            Some(dt) => self.from_local_datetime(&dt),
            None => MappedLocalTime::None,
        }
    }
    fn ymd(&self, year: i32, month: u32, day: u32) -> Date<Self> {
        self.ymd_opt(year, month, day).unwrap()
    }
    fn ymd_opt(&self, year: i32, month: u32, day: u32) -> MappedLocalTime<Date<Self>> {
        match NaiveDate::from_ymd_opt(year, month, day) {
            Some(d) => self.from_local_date(&d),
            None => MappedLocalTime::None,
        }
    }
    fn yo(&self, year: i32, ordinal: u32) -> Date<Self> {
        self.yo_opt(year, ordinal).unwrap()
    }
    fn yo_opt(&self, year: i32, ordinal: u32) -> MappedLocalTime<Date<Self>> {
        match NaiveDate::from_yo_opt(year, ordinal) {
            Some(d) => self.from_local_date(&d),
            None => MappedLocalTime::None,
        }
    }
    fn isoywd(&self, year: i32, week: u32, weekday: Weekday) -> Date<Self> {
        self.isoywd_opt(year, week, weekday).unwrap()
    }
    fn isoywd_opt(&self, year: i32, week: u32, weekday: Weekday) -> MappedLocalTime<Date<Self>> {
        match NaiveDate::from_isoywd_opt(year, week, weekday) {
            Some(d) => self.from_local_date(&d),
            None => MappedLocalTime::None,
        }
    }
    fn timestamp(&self, secs: i64, nsecs: u32) -> DateTime<Self> {
        self.timestamp_opt(secs, nsecs).unwrap()
    }
    fn timestamp_opt(&self, secs: i64, nsecs: u32) -> MappedLocalTime<DateTime<Self>> {
        match DateTime::from_timestamp(secs, nsecs) {
            Some(dt) => MappedLocalTime::Single(self.from_utc_datetime(&dt.naive_utc())),
            None => MappedLocalTime::None,
        }
    }
    fn timestamp_millis(&self, millis: i64) -> DateTime<Self> {
        self.timestamp_millis_opt(millis).unwrap()
    }
    fn timestamp_millis_opt(&self, millis: i64) -> MappedLocalTime<DateTime<Self>> {
        match DateTime::from_timestamp_millis(millis) {
            Some(dt) => MappedLocalTime::Single(self.from_utc_datetime(&dt.naive_utc())),
            None => MappedLocalTime::None,
        }
    }
    fn timestamp_nanos(&self, nanos: i64) -> DateTime<Self> {
        self.from_utc_datetime(&DateTime::from_timestamp_nanos(nanos).naive_utc())
    }
    fn timestamp_micros(&self, micros: i64) -> MappedLocalTime<DateTime<Self>> {
        match DateTime::from_timestamp_micros(micros) {
            Some(dt) => MappedLocalTime::Single(self.from_utc_datetime(&dt.naive_utc())),
            None => MappedLocalTime::None,
        }
    }
    fn datetime_from_str(&self, s: &str, fmt: &str) -> ParseResult<DateTime<Self>> {
        let mut parsed = Parsed::new();
        parse(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_datetime_with_timezone(self)
    }
    fn from_offset(offset: &Self::Offset) -> Self;
    fn offset_from_local_date(&self, local: &NaiveDate) -> MappedLocalTime<Self::Offset>;
    fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> MappedLocalTime<Self::Offset>;
    fn from_local_date(&self, local: &NaiveDate) -> MappedLocalTime<Date<Self>> {
        self.offset_from_local_date(local)
            .map(|offset| Date::from_utc(*local, offset))
    }
    fn from_local_datetime(&self, local: &NaiveDateTime) -> MappedLocalTime<DateTime<Self>> {
        self.offset_from_local_datetime(local).and_then(|off| {
            local
                .checked_sub_offset(off.fix())
                .map(|dt| DateTime::from_naive_utc_and_offset(dt, off))
        })
    }
    fn offset_from_utc_date(&self, utc: &NaiveDate) -> Self::Offset;
    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> Self::Offset;
    fn from_utc_date(&self, utc: &NaiveDate) -> Date<Self> {
        Date::from_utc(*utc, self.offset_from_utc_date(utc))
    }
    fn from_utc_datetime(&self, utc: &NaiveDateTime) -> DateTime<Self> {
        DateTime::from_naive_utc_and_offset(*utc, self.offset_from_utc_datetime(utc))
    }
}
