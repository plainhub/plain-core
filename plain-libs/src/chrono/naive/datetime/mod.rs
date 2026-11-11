use crate::chrono::format::DelayedFormat;
use crate::chrono::format::{Fixed, Item, Numeric, Pad};
use crate::chrono::format::{
    ParseError, ParseResult, Parsed, StrftimeItems, parse, parse_and_remainder,
};
use crate::chrono::naive::{Days, IsoWeek, NaiveDate, NaiveTime};
use crate::chrono::offset::Utc;
use crate::chrono::time_delta::NANOS_PER_SEC;
use crate::chrono::{
    DateTime, Datelike, FixedOffset, MappedLocalTime, Months, TimeDelta, TimeZone, Timelike,
    Weekday, expect, try_opt,
};
use core::borrow::Borrow;
use core::fmt::Write;
use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::time::Duration;
use core::{fmt, str};
pub(crate) mod serde;
pub const MIN_DATETIME: NaiveDateTime = NaiveDateTime::MIN;
pub const MAX_DATETIME: NaiveDateTime = NaiveDateTime::MAX;
#[derive(PartialEq, Eq, Hash, PartialOrd, Ord, Copy, Clone)]
#[cfg_attr(all(any(), all()), derive(arbitrary::Arbitrary))]
pub struct NaiveDateTime {
    date: NaiveDate,
    time: NaiveTime,
}
impl NaiveDateTime {
    #[inline]
    pub const fn new(date: NaiveDate, time: NaiveTime) -> NaiveDateTime {
        NaiveDateTime { date, time }
    }
    #[inline]
    #[must_use]
    pub const fn from_timestamp(secs: i64, nsecs: u32) -> NaiveDateTime {
        let datetime = expect(
            DateTime::from_timestamp(secs, nsecs),
            "invalid or out-of-range datetime",
        );
        datetime.naive_utc()
    }
    #[inline]
    #[must_use]
    pub const fn from_timestamp_millis(millis: i64) -> Option<NaiveDateTime> {
        Some(try_opt!(DateTime::from_timestamp_millis(millis)).naive_utc())
    }
    #[inline]
    #[must_use]
    pub const fn from_timestamp_micros(micros: i64) -> Option<NaiveDateTime> {
        let secs = micros.div_euclid(1_000_000);
        let nsecs = micros.rem_euclid(1_000_000) as u32 * 1000;
        Some(try_opt!(DateTime::<Utc>::from_timestamp(secs, nsecs)).naive_utc())
    }
    #[inline]
    #[must_use]
    pub const fn from_timestamp_nanos(nanos: i64) -> Option<NaiveDateTime> {
        let secs = nanos.div_euclid(NANOS_PER_SEC as i64);
        let nsecs = nanos.rem_euclid(NANOS_PER_SEC as i64) as u32;
        Some(try_opt!(DateTime::from_timestamp(secs, nsecs)).naive_utc())
    }

    pub fn parse_from_str(s: &str, fmt: &str) -> ParseResult<NaiveDateTime> {
        let mut parsed = Parsed::new();
        parse(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_naive_datetime_with_offset(0)
    }
    pub fn parse_and_remainder<'a>(s: &'a str, fmt: &str) -> ParseResult<(NaiveDateTime, &'a str)> {
        let mut parsed = Parsed::new();
        let remainder = parse_and_remainder(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed
            .to_naive_datetime_with_offset(0)
            .map(|d| (d, remainder))
    }
    #[inline]
    pub const fn date(&self) -> NaiveDate {
        self.date
    }
    #[inline]
    pub const fn time(&self) -> NaiveTime {
        self.time
    }
    #[inline]
    #[must_use]
    pub const fn timestamp(&self) -> i64 {
        self.and_utc().timestamp()
    }
    #[inline]
    #[must_use]
    pub const fn timestamp_millis(&self) -> i64 {
        self.and_utc().timestamp_millis()
    }
    #[inline]
    #[must_use]
    pub const fn timestamp_micros(&self) -> i64 {
        self.and_utc().timestamp_micros()
    }
    #[inline]
    #[must_use]
    pub const fn timestamp_nanos(&self) -> i64 {
        self.and_utc().timestamp_nanos()
    }
    #[inline]
    #[must_use]
    pub const fn timestamp_nanos_opt(&self) -> Option<i64> {
        self.and_utc().timestamp_nanos_opt()
    }
    #[inline]
    #[must_use]
    pub const fn timestamp_subsec_millis(&self) -> u32 {
        self.and_utc().timestamp_subsec_millis()
    }
    #[inline]
    #[must_use]
    pub const fn timestamp_subsec_micros(&self) -> u32 {
        self.and_utc().timestamp_subsec_micros()
    }
    pub const fn timestamp_subsec_nanos(&self) -> u32 {
        self.and_utc().timestamp_subsec_nanos()
    }
    #[must_use]
    pub const fn checked_add_signed(self, rhs: TimeDelta) -> Option<NaiveDateTime> {
        let (time, remainder) = self.time.overflowing_add_signed(rhs);
        let remainder = try_opt!(TimeDelta::try_seconds(remainder));
        let date = try_opt!(self.date.checked_add_signed(remainder));
        Some(NaiveDateTime { date, time })
    }
    #[must_use]
    pub const fn checked_add_months(self, rhs: Months) -> Option<NaiveDateTime> {
        Some(Self {
            date: try_opt!(self.date.checked_add_months(rhs)),
            time: self.time,
        })
    }
    #[must_use]
    pub const fn checked_add_offset(self, rhs: FixedOffset) -> Option<NaiveDateTime> {
        let (time, days) = self.time.overflowing_add_offset(rhs);
        let date = match days {
            -1 => try_opt!(self.date.pred_opt()),
            1 => try_opt!(self.date.succ_opt()),
            _ => self.date,
        };
        Some(NaiveDateTime { date, time })
    }
    pub const fn checked_sub_offset(self, rhs: FixedOffset) -> Option<NaiveDateTime> {
        let (time, days) = self.time.overflowing_sub_offset(rhs);
        let date = match days {
            -1 => try_opt!(self.date.pred_opt()),
            1 => try_opt!(self.date.succ_opt()),
            _ => self.date,
        };
        Some(NaiveDateTime { date, time })
    }
    #[must_use]
    pub(crate) fn overflowing_add_offset(self, rhs: FixedOffset) -> NaiveDateTime {
        let (time, days) = self.time.overflowing_add_offset(rhs);
        let date = match days {
            -1 => self.date.pred_opt().unwrap_or(NaiveDate::BEFORE_MIN),
            1 => self.date.succ_opt().unwrap_or(NaiveDate::AFTER_MAX),
            _ => self.date,
        };
        NaiveDateTime { date, time }
    }
    #[must_use]
    pub(crate) fn overflowing_sub_offset(self, rhs: FixedOffset) -> NaiveDateTime {
        let (time, days) = self.time.overflowing_sub_offset(rhs);
        let date = match days {
            -1 => self.date.pred_opt().unwrap_or(NaiveDate::BEFORE_MIN),
            1 => self.date.succ_opt().unwrap_or(NaiveDate::AFTER_MAX),
            _ => self.date,
        };
        NaiveDateTime { date, time }
    }
    #[must_use]
    pub const fn checked_sub_signed(self, rhs: TimeDelta) -> Option<NaiveDateTime> {
        let (time, remainder) = self.time.overflowing_sub_signed(rhs);
        let remainder = try_opt!(TimeDelta::try_seconds(remainder));
        let date = try_opt!(self.date.checked_sub_signed(remainder));
        Some(NaiveDateTime { date, time })
    }
    #[must_use]
    pub const fn checked_sub_months(self, rhs: Months) -> Option<NaiveDateTime> {
        Some(Self {
            date: try_opt!(self.date.checked_sub_months(rhs)),
            time: self.time,
        })
    }
    #[must_use]
    pub const fn checked_add_days(self, days: Days) -> Option<Self> {
        Some(Self {
            date: try_opt!(self.date.checked_add_days(days)),
            ..self
        })
    }
    #[must_use]
    pub const fn checked_sub_days(self, days: Days) -> Option<Self> {
        Some(Self {
            date: try_opt!(self.date.checked_sub_days(days)),
            ..self
        })
    }
    #[must_use]
    pub const fn signed_duration_since(self, rhs: NaiveDateTime) -> TimeDelta {
        expect(
            self.date
                .signed_duration_since(rhs.date)
                .checked_add(&self.time.signed_duration_since(rhs.time)),
            "always in range",
        )
    }
    #[inline]
    #[must_use]
    pub fn format_with_items<'a, I, B>(&self, items: I) -> DelayedFormat<I>
    where
        I: Iterator<Item = B> + Clone,
        B: Borrow<Item<'a>>,
    {
        DelayedFormat::new(Some(self.date), Some(self.time), items)
    }
    #[inline]
    #[must_use]
    pub fn format<'a>(&self, fmt: &'a str) -> DelayedFormat<StrftimeItems<'a>> {
        self.format_with_items(StrftimeItems::new(fmt))
    }
    #[must_use]
    pub fn and_local_timezone<Tz: TimeZone>(&self, tz: Tz) -> MappedLocalTime<DateTime<Tz>> {
        tz.from_local_datetime(self)
    }
    #[must_use]
    pub const fn and_utc(&self) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(*self, Utc)
    }
    pub const MIN: Self = Self {
        date: NaiveDate::MIN,
        time: NaiveTime::MIN,
    };
    pub const MAX: Self = Self {
        date: NaiveDate::MAX,
        time: NaiveTime::MAX,
    };
    pub const UNIX_EPOCH: Self = DateTime::UNIX_EPOCH.naive_utc();
}
impl From<NaiveDate> for NaiveDateTime {
    fn from(date: NaiveDate) -> Self {
        date.and_hms_opt(0, 0, 0).unwrap()
    }
}
impl Datelike for NaiveDateTime {
    #[inline]
    fn year(&self) -> i32 {
        self.date.year()
    }
    #[inline]
    fn month(&self) -> u32 {
        self.date.month()
    }
    #[inline]
    fn month0(&self) -> u32 {
        self.date.month0()
    }
    #[inline]
    fn day(&self) -> u32 {
        self.date.day()
    }
    #[inline]
    fn day0(&self) -> u32 {
        self.date.day0()
    }
    #[inline]
    fn ordinal(&self) -> u32 {
        self.date.ordinal()
    }
    #[inline]
    fn ordinal0(&self) -> u32 {
        self.date.ordinal0()
    }
    #[inline]
    fn weekday(&self) -> Weekday {
        self.date.weekday()
    }
    #[inline]
    fn iso_week(&self) -> IsoWeek {
        self.date.iso_week()
    }
    #[inline]
    fn with_year(&self, year: i32) -> Option<NaiveDateTime> {
        self.date
            .with_year(year)
            .map(|d| NaiveDateTime { date: d, ..*self })
    }
    #[inline]
    fn with_month(&self, month: u32) -> Option<NaiveDateTime> {
        self.date
            .with_month(month)
            .map(|d| NaiveDateTime { date: d, ..*self })
    }
    #[inline]
    fn with_month0(&self, month0: u32) -> Option<NaiveDateTime> {
        self.date
            .with_month0(month0)
            .map(|d| NaiveDateTime { date: d, ..*self })
    }
    #[inline]
    fn with_day(&self, day: u32) -> Option<NaiveDateTime> {
        self.date
            .with_day(day)
            .map(|d| NaiveDateTime { date: d, ..*self })
    }
    #[inline]
    fn with_day0(&self, day0: u32) -> Option<NaiveDateTime> {
        self.date
            .with_day0(day0)
            .map(|d| NaiveDateTime { date: d, ..*self })
    }
    #[inline]
    fn with_ordinal(&self, ordinal: u32) -> Option<NaiveDateTime> {
        self.date
            .with_ordinal(ordinal)
            .map(|d| NaiveDateTime { date: d, ..*self })
    }
    #[inline]
    fn with_ordinal0(&self, ordinal0: u32) -> Option<NaiveDateTime> {
        self.date
            .with_ordinal0(ordinal0)
            .map(|d| NaiveDateTime { date: d, ..*self })
    }
}
impl Timelike for NaiveDateTime {
    #[inline]
    fn hour(&self) -> u32 {
        self.time.hour()
    }
    #[inline]
    fn minute(&self) -> u32 {
        self.time.minute()
    }
    #[inline]
    fn second(&self) -> u32 {
        self.time.second()
    }
    #[inline]
    fn nanosecond(&self) -> u32 {
        self.time.nanosecond()
    }
    #[inline]
    fn with_hour(&self, hour: u32) -> Option<NaiveDateTime> {
        self.time
            .with_hour(hour)
            .map(|t| NaiveDateTime { time: t, ..*self })
    }
    #[inline]
    fn with_minute(&self, min: u32) -> Option<NaiveDateTime> {
        self.time
            .with_minute(min)
            .map(|t| NaiveDateTime { time: t, ..*self })
    }
    #[inline]
    fn with_second(&self, sec: u32) -> Option<NaiveDateTime> {
        self.time
            .with_second(sec)
            .map(|t| NaiveDateTime { time: t, ..*self })
    }
    #[inline]
    fn with_nanosecond(&self, nano: u32) -> Option<NaiveDateTime> {
        self.time
            .with_nanosecond(nano)
            .map(|t| NaiveDateTime { time: t, ..*self })
    }
}
impl Add<TimeDelta> for NaiveDateTime {
    type Output = NaiveDateTime;
    #[inline]
    #[track_caller]
    fn add(self, rhs: TimeDelta) -> NaiveDateTime {
        self.checked_add_signed(rhs)
            .expect("`NaiveDateTime + TimeDelta` overflowed")
    }
}
impl Add<Duration> for NaiveDateTime {
    type Output = NaiveDateTime;
    #[inline]
    #[track_caller]
    fn add(self, rhs: Duration) -> NaiveDateTime {
        let rhs = TimeDelta::from_std(rhs)
            .expect("overflow converting from core::time::Duration to TimeDelta");
        self.checked_add_signed(rhs)
            .expect("`NaiveDateTime + TimeDelta` overflowed")
    }
}
impl AddAssign<TimeDelta> for NaiveDateTime {
    #[inline]
    #[track_caller]
    fn add_assign(&mut self, rhs: TimeDelta) {
        *self = self.add(rhs);
    }
}
impl AddAssign<Duration> for NaiveDateTime {
    #[inline]
    #[track_caller]
    fn add_assign(&mut self, rhs: Duration) {
        *self = self.add(rhs);
    }
}
impl Add<FixedOffset> for NaiveDateTime {
    type Output = NaiveDateTime;
    #[inline]
    #[track_caller]
    fn add(self, rhs: FixedOffset) -> NaiveDateTime {
        self.checked_add_offset(rhs)
            .expect("`NaiveDateTime + FixedOffset` out of range")
    }
}
impl Add<Months> for NaiveDateTime {
    type Output = NaiveDateTime;
    #[track_caller]
    fn add(self, rhs: Months) -> Self::Output {
        self.checked_add_months(rhs)
            .expect("`NaiveDateTime + Months` out of range")
    }
}
impl Sub<TimeDelta> for NaiveDateTime {
    type Output = NaiveDateTime;
    #[inline]
    #[track_caller]
    fn sub(self, rhs: TimeDelta) -> NaiveDateTime {
        self.checked_sub_signed(rhs)
            .expect("`NaiveDateTime - TimeDelta` overflowed")
    }
}
impl Sub<Duration> for NaiveDateTime {
    type Output = NaiveDateTime;
    #[inline]
    #[track_caller]
    fn sub(self, rhs: Duration) -> NaiveDateTime {
        let rhs = TimeDelta::from_std(rhs)
            .expect("overflow converting from core::time::Duration to TimeDelta");
        self.checked_sub_signed(rhs)
            .expect("`NaiveDateTime - TimeDelta` overflowed")
    }
}
impl SubAssign<TimeDelta> for NaiveDateTime {
    #[inline]
    #[track_caller]
    fn sub_assign(&mut self, rhs: TimeDelta) {
        *self = self.sub(rhs);
    }
}
impl SubAssign<Duration> for NaiveDateTime {
    #[inline]
    #[track_caller]
    fn sub_assign(&mut self, rhs: Duration) {
        *self = self.sub(rhs);
    }
}
impl Sub<FixedOffset> for NaiveDateTime {
    type Output = NaiveDateTime;
    #[inline]
    #[track_caller]
    fn sub(self, rhs: FixedOffset) -> NaiveDateTime {
        self.checked_sub_offset(rhs)
            .expect("`NaiveDateTime - FixedOffset` out of range")
    }
}
impl Sub<Months> for NaiveDateTime {
    type Output = NaiveDateTime;
    #[track_caller]
    fn sub(self, rhs: Months) -> Self::Output {
        self.checked_sub_months(rhs)
            .expect("`NaiveDateTime - Months` out of range")
    }
}
impl Sub<NaiveDateTime> for NaiveDateTime {
    type Output = TimeDelta;
    #[inline]
    fn sub(self, rhs: NaiveDateTime) -> TimeDelta {
        self.signed_duration_since(rhs)
    }
}
impl Add<Days> for NaiveDateTime {
    type Output = NaiveDateTime;
    #[track_caller]
    fn add(self, days: Days) -> Self::Output {
        self.checked_add_days(days)
            .expect("`NaiveDateTime + Days` out of range")
    }
}
impl Sub<Days> for NaiveDateTime {
    type Output = NaiveDateTime;
    #[track_caller]
    fn sub(self, days: Days) -> Self::Output {
        self.checked_sub_days(days)
            .expect("`NaiveDateTime - Days` out of range")
    }
}
impl fmt::Debug for NaiveDateTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.date.fmt(f)?;
        f.write_char('T')?;
        self.time.fmt(f)
    }
}
impl fmt::Display for NaiveDateTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.date.fmt(f)?;
        f.write_char(' ')?;
        self.time.fmt(f)
    }
}
impl str::FromStr for NaiveDateTime {
    type Err = ParseError;
    fn from_str(s: &str) -> ParseResult<NaiveDateTime> {
        const ITEMS: &[Item<'static>] = &[
            Item::Numeric(Numeric::Year, Pad::Zero),
            Item::Space(""),
            Item::Literal("-"),
            Item::Numeric(Numeric::Month, Pad::Zero),
            Item::Space(""),
            Item::Literal("-"),
            Item::Numeric(Numeric::Day, Pad::Zero),
            Item::Space(""),
            Item::Literal("T"),
            Item::Numeric(Numeric::Hour, Pad::Zero),
            Item::Space(""),
            Item::Literal(":"),
            Item::Numeric(Numeric::Minute, Pad::Zero),
            Item::Space(""),
            Item::Literal(":"),
            Item::Numeric(Numeric::Second, Pad::Zero),
            Item::Fixed(Fixed::Nanosecond),
            Item::Space(""),
        ];
        let mut parsed = Parsed::new();
        parse(&mut parsed, s, ITEMS.iter())?;
        parsed.to_naive_datetime_with_offset(0)
    }
}
impl Default for NaiveDateTime {
    fn default() -> Self {
        DateTime::UNIX_EPOCH.naive_local()
    }
}
