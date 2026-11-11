use crate::chrono::Date;
use crate::chrono::format::{DelayedFormat, SecondsFormat, write_rfc2822, write_rfc3339};
use crate::chrono::format::{
    Fixed, Item, ParseError, ParseResult, Parsed, StrftimeItems, parse, parse_and_remainder,
    parse_rfc3339,
};
use crate::chrono::naive::{Days, IsoWeek, NaiveDate, NaiveDateTime, NaiveTime};
use crate::chrono::offset::{FixedOffset, LocalResult, Offset, TimeZone, Utc};
use crate::chrono::{Datelike, Months, TimeDelta, Timelike, Weekday};
use crate::chrono::{expect, try_opt};
use core::borrow::Borrow;
use core::cmp::Ordering;
use core::fmt::Write;
use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::time::Duration;
use core::{fmt, hash, str};
use std::time::{SystemTime, UNIX_EPOCH};
pub(super) mod serde;
#[derive(Clone)]
pub struct DateTime<Tz: TimeZone> {
    datetime: NaiveDateTime,
    offset: Tz::Offset,
}
pub const MIN_DATETIME: DateTime<Utc> = DateTime::<Utc>::MIN_UTC;
pub const MAX_DATETIME: DateTime<Utc> = DateTime::<Utc>::MAX_UTC;
impl<Tz: TimeZone> DateTime<Tz> {
    #[inline]
    #[must_use]
    pub const fn from_naive_utc_and_offset(
        datetime: NaiveDateTime,
        offset: Tz::Offset,
    ) -> DateTime<Tz> {
        DateTime { datetime, offset }
    }
    #[inline]
    #[must_use]
    pub fn from_utc(datetime: NaiveDateTime, offset: Tz::Offset) -> DateTime<Tz> {
        DateTime { datetime, offset }
    }

    #[inline]
    #[must_use]
    pub fn date(&self) -> Date<Tz> {
        Date::from_utc(self.naive_local().date(), self.offset.clone())
    }
    #[inline]
    #[must_use]
    pub fn date_naive(&self) -> NaiveDate {
        self.naive_local().date()
    }
    #[inline]
    #[must_use]
    pub fn time(&self) -> NaiveTime {
        self.datetime.time() + self.offset.fix()
    }
    #[inline]
    #[must_use]
    pub const fn timestamp(&self) -> i64 {
        let gregorian_day = self.datetime.date().num_days_from_ce() as i64;
        let seconds_from_midnight = self.datetime.time().num_seconds_from_midnight() as i64;
        (gregorian_day - UNIX_EPOCH_DAY) * 86_400 + seconds_from_midnight
    }
    #[inline]
    #[must_use]
    pub const fn timestamp_millis(&self) -> i64 {
        let as_ms = self.timestamp() * 1000;
        as_ms + self.timestamp_subsec_millis() as i64
    }
    #[inline]
    #[must_use]
    pub const fn timestamp_micros(&self) -> i64 {
        let as_us = self.timestamp() * 1_000_000;
        as_us + self.timestamp_subsec_micros() as i64
    }
    #[inline]
    #[must_use]
    pub const fn timestamp_nanos(&self) -> i64 {
        expect(
            self.timestamp_nanos_opt(),
            "value can not be represented in a timestamp with nanosecond precision.",
        )
    }
    #[inline]
    #[must_use]
    pub const fn timestamp_nanos_opt(&self) -> Option<i64> {
        let mut timestamp = self.timestamp();
        let mut subsec_nanos = self.timestamp_subsec_nanos() as i64;
        if timestamp < 0 {
            subsec_nanos -= 1_000_000_000;
            timestamp += 1;
        }
        try_opt!(timestamp.checked_mul(1_000_000_000)).checked_add(subsec_nanos)
    }
    #[inline]
    #[must_use]
    pub const fn timestamp_subsec_millis(&self) -> u32 {
        self.timestamp_subsec_nanos() / 1_000_000
    }
    #[inline]
    #[must_use]
    pub const fn timestamp_subsec_micros(&self) -> u32 {
        self.timestamp_subsec_nanos() / 1_000
    }
    #[inline]
    #[must_use]
    pub const fn timestamp_subsec_nanos(&self) -> u32 {
        self.datetime.time().nanosecond()
    }
    #[inline]
    #[must_use]
    pub const fn offset(&self) -> &Tz::Offset {
        &self.offset
    }
    #[inline]
    #[must_use]
    pub fn timezone(&self) -> Tz {
        TimeZone::from_offset(&self.offset)
    }
    #[inline]
    #[must_use]
    pub fn with_timezone<Tz2: TimeZone>(&self, tz: &Tz2) -> DateTime<Tz2> {
        tz.from_utc_datetime(&self.datetime)
    }

    #[inline]
    #[must_use]
    pub fn checked_add_signed(self, rhs: TimeDelta) -> Option<DateTime<Tz>> {
        let datetime = self.datetime.checked_add_signed(rhs)?;
        let tz = self.timezone();
        Some(tz.from_utc_datetime(&datetime))
    }
    #[must_use]
    pub fn checked_add_months(self, months: Months) -> Option<DateTime<Tz>> {
        self.overflowing_naive_local()
            .checked_add_months(months)?
            .and_local_timezone(Tz::from_offset(&self.offset))
            .single()
    }
    #[inline]
    #[must_use]
    pub fn checked_sub_signed(self, rhs: TimeDelta) -> Option<DateTime<Tz>> {
        let datetime = self.datetime.checked_sub_signed(rhs)?;
        let tz = self.timezone();
        Some(tz.from_utc_datetime(&datetime))
    }
    #[must_use]
    pub fn checked_sub_months(self, months: Months) -> Option<DateTime<Tz>> {
        self.overflowing_naive_local()
            .checked_sub_months(months)?
            .and_local_timezone(Tz::from_offset(&self.offset))
            .single()
    }
    #[must_use]
    pub fn checked_add_days(self, days: Days) -> Option<Self> {
        if days == Days::new(0) {
            return Some(self);
        }
        self.overflowing_naive_local()
            .checked_add_days(days)
            .and_then(|dt| self.timezone().from_local_datetime(&dt).single())
            .filter(|dt| dt <= &DateTime::<Utc>::MAX_UTC)
    }
    #[must_use]
    pub fn checked_sub_days(self, days: Days) -> Option<Self> {
        self.overflowing_naive_local()
            .checked_sub_days(days)
            .and_then(|dt| self.timezone().from_local_datetime(&dt).single())
            .filter(|dt| dt >= &DateTime::<Utc>::MIN_UTC)
    }
    #[inline]
    #[must_use]
    pub fn signed_duration_since<Tz2: TimeZone>(
        self,
        rhs: impl Borrow<DateTime<Tz2>>,
    ) -> TimeDelta {
        self.datetime.signed_duration_since(rhs.borrow().datetime)
    }
    #[inline]
    #[must_use]
    pub const fn naive_utc(&self) -> NaiveDateTime {
        self.datetime
    }
    #[inline]
    #[must_use]
    pub fn naive_local(&self) -> NaiveDateTime {
        self.datetime
            .checked_add_offset(self.offset.fix())
            .expect("Local time out of range for `NaiveDateTime`")
    }
    #[inline]
    #[must_use]
    pub(crate) fn overflowing_naive_local(&self) -> NaiveDateTime {
        self.datetime.overflowing_add_offset(self.offset.fix())
    }
    #[must_use]
    pub fn years_since(&self, base: Self) -> Option<u32> {
        let mut years = self.year() - base.year();
        let earlier_time =
            (self.month(), self.day(), self.time()) < (base.month(), base.day(), base.time());
        years -= match earlier_time {
            true => 1,
            false => 0,
        };
        match years >= 0 {
            true => Some(years as u32),
            false => None,
        }
    }

    #[must_use]
    pub fn to_rfc3339(&self) -> String {
        let mut result = String::with_capacity(32);
        let naive = self.overflowing_naive_local();
        let offset = self.offset.fix();
        write_rfc3339(&mut result, naive, offset, SecondsFormat::AutoSi, false)
            .expect("writing rfc3339 datetime to string should never fail");
        result
    }
    #[must_use]
    pub fn to_rfc3339_opts(&self, secform: SecondsFormat, use_z: bool) -> String {
        let mut result = String::with_capacity(38);
        write_rfc3339(
            &mut result,
            self.naive_local(),
            self.offset.fix(),
            secform,
            use_z,
        )
        .expect("writing rfc3339 datetime to string should never fail");
        result
    }

    pub const MIN_UTC: DateTime<Utc> = DateTime {
        datetime: NaiveDateTime::MIN,
        offset: Utc,
    };
    pub const MAX_UTC: DateTime<Utc> = DateTime {
        datetime: NaiveDateTime::MAX,
        offset: Utc,
    };
}
impl DateTime<Utc> {
    #[inline]
    #[must_use]
    pub const fn from_timestamp_secs(secs: i64) -> Option<Self> {
        Self::from_timestamp(secs, 0)
    }
    #[inline]
    #[must_use]
    pub const fn from_timestamp(secs: i64, nsecs: u32) -> Option<Self> {
        let days = secs.div_euclid(86_400) + UNIX_EPOCH_DAY;
        let secs = secs.rem_euclid(86_400);
        if days < i32::MIN as i64 || days > i32::MAX as i64 {
            return None;
        }
        let date = try_opt!(NaiveDate::from_num_days_from_ce_opt(days as i32));
        let time = try_opt!(NaiveTime::from_num_seconds_from_midnight_opt(
            secs as u32,
            nsecs
        ));
        Some(date.and_time(time).and_utc())
    }
    #[inline]
    #[must_use]
    pub const fn from_timestamp_millis(millis: i64) -> Option<Self> {
        let secs = millis.div_euclid(1000);
        let nsecs = millis.rem_euclid(1000) as u32 * 1_000_000;
        Self::from_timestamp(secs, nsecs)
    }
    #[inline]
    #[must_use]
    pub const fn from_timestamp_micros(micros: i64) -> Option<Self> {
        let secs = micros.div_euclid(1_000_000);
        let nsecs = micros.rem_euclid(1_000_000) as u32 * 1000;
        Self::from_timestamp(secs, nsecs)
    }
    #[inline]
    #[must_use]
    pub const fn from_timestamp_nanos(nanos: i64) -> Self {
        let secs = nanos.div_euclid(1_000_000_000);
        let nsecs = nanos.rem_euclid(1_000_000_000) as u32;
        expect(
            Self::from_timestamp(secs, nsecs),
            "timestamp in nanos is always in range",
        )
    }
    pub const UNIX_EPOCH: Self = expect(NaiveDate::from_ymd_opt(1970, 1, 1), "")
        .and_time(NaiveTime::MIN)
        .and_utc();
}
impl Default for DateTime<Utc> {
    fn default() -> Self {
        Utc.from_utc_datetime(&NaiveDateTime::default())
    }
}
impl Default for DateTime<FixedOffset> {
    fn default() -> Self {
        FixedOffset::west_opt(0)
            .unwrap()
            .from_utc_datetime(&NaiveDateTime::default())
    }
}
impl From<DateTime<Utc>> for DateTime<FixedOffset> {
    fn from(src: DateTime<Utc>) -> Self {
        src.with_timezone(&FixedOffset::east_opt(0).unwrap())
    }
}
impl From<DateTime<FixedOffset>> for DateTime<Utc> {
    fn from(src: DateTime<FixedOffset>) -> Self {
        src.with_timezone(&Utc)
    }
}
fn map_local<Tz: TimeZone, F>(dt: &DateTime<Tz>, mut f: F) -> Option<DateTime<Tz>>
where
    F: FnMut(NaiveDateTime) -> Option<NaiveDateTime>,
{
    f(dt.overflowing_naive_local())
        .and_then(|datetime| dt.timezone().from_local_datetime(&datetime).single())
        .filter(|dt| dt >= &DateTime::<Utc>::MIN_UTC && dt <= &DateTime::<Utc>::MAX_UTC)
}
impl DateTime<FixedOffset> {
    pub fn parse_from_rfc2822(s: &str) -> ParseResult<DateTime<FixedOffset>> {
        const ITEMS: &[Item<'static>] = &[Item::Fixed(Fixed::RFC2822)];
        let mut parsed = Parsed::new();
        parse(&mut parsed, s, ITEMS.iter())?;
        parsed.to_datetime()
    }
    pub fn parse_from_rfc3339(s: &str) -> ParseResult<DateTime<FixedOffset>> {
        parse_rfc3339(s)
    }
    pub fn parse_from_str(s: &str, fmt: &str) -> ParseResult<DateTime<FixedOffset>> {
        let mut parsed = Parsed::new();
        parse(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_datetime()
    }
    pub fn parse_and_remainder<'a>(
        s: &'a str,
        fmt: &str,
    ) -> ParseResult<(DateTime<FixedOffset>, &'a str)> {
        let mut parsed = Parsed::new();
        let remainder = parse_and_remainder(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_datetime().map(|d| (d, remainder))
    }
}
impl<Tz: TimeZone> DateTime<Tz>
where
    Tz::Offset: fmt::Display,
{
    #[inline]
    #[must_use]
    pub fn format_with_items<'a, I, B>(&self, items: I) -> DelayedFormat<I>
    where
        I: Iterator<Item = B> + Clone,
        B: Borrow<Item<'a>>,
    {
        let local = self.overflowing_naive_local();
        DelayedFormat::new_with_offset(Some(local.date()), Some(local.time()), &self.offset, items)
    }
    #[inline]
    #[must_use]
    pub fn format<'a>(&self, fmt: &'a str) -> DelayedFormat<StrftimeItems<'a>> {
        self.format_with_items(StrftimeItems::new(fmt))
    }
}
impl<Tz: TimeZone> Datelike for DateTime<Tz> {
    #[inline]
    fn year(&self) -> i32 {
        self.overflowing_naive_local().year()
    }
    #[inline]
    fn month(&self) -> u32 {
        self.overflowing_naive_local().month()
    }
    #[inline]
    fn month0(&self) -> u32 {
        self.overflowing_naive_local().month0()
    }
    #[inline]
    fn day(&self) -> u32 {
        self.overflowing_naive_local().day()
    }
    #[inline]
    fn day0(&self) -> u32 {
        self.overflowing_naive_local().day0()
    }
    #[inline]
    fn ordinal(&self) -> u32 {
        self.overflowing_naive_local().ordinal()
    }
    #[inline]
    fn ordinal0(&self) -> u32 {
        self.overflowing_naive_local().ordinal0()
    }
    #[inline]
    fn weekday(&self) -> Weekday {
        self.overflowing_naive_local().weekday()
    }
    #[inline]
    fn iso_week(&self) -> IsoWeek {
        self.overflowing_naive_local().iso_week()
    }
    #[inline]
    fn with_year(&self, year: i32) -> Option<DateTime<Tz>> {
        map_local(self, |dt| match dt.year() == year {
            true => Some(dt),
            false => dt.with_year(year),
        })
    }
    #[inline]
    fn with_month(&self, month: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_month(month))
    }
    #[inline]
    fn with_month0(&self, month0: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_month0(month0))
    }
    #[inline]
    fn with_day(&self, day: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_day(day))
    }
    #[inline]
    fn with_day0(&self, day0: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_day0(day0))
    }
    #[inline]
    fn with_ordinal(&self, ordinal: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_ordinal(ordinal))
    }
    #[inline]
    fn with_ordinal0(&self, ordinal0: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_ordinal0(ordinal0))
    }
}
impl<Tz: TimeZone> Timelike for DateTime<Tz> {
    #[inline]
    fn hour(&self) -> u32 {
        self.overflowing_naive_local().hour()
    }
    #[inline]
    fn minute(&self) -> u32 {
        self.overflowing_naive_local().minute()
    }
    #[inline]
    fn second(&self) -> u32 {
        self.overflowing_naive_local().second()
    }
    #[inline]
    fn nanosecond(&self) -> u32 {
        self.overflowing_naive_local().nanosecond()
    }
    #[inline]
    fn with_hour(&self, hour: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_hour(hour))
    }
    #[inline]
    fn with_minute(&self, min: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_minute(min))
    }
    #[inline]
    fn with_second(&self, sec: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_second(sec))
    }
    #[inline]
    fn with_nanosecond(&self, nano: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_nanosecond(nano))
    }
}
impl<Tz: TimeZone> Copy for DateTime<Tz>
where
    <Tz as TimeZone>::Offset: Copy,
    NaiveDateTime: Copy,
{
}
impl<Tz: TimeZone, Tz2: TimeZone> PartialEq<DateTime<Tz2>> for DateTime<Tz> {
    fn eq(&self, other: &DateTime<Tz2>) -> bool {
        self.datetime == other.datetime
    }
}
impl<Tz: TimeZone> Eq for DateTime<Tz> {}
impl<Tz: TimeZone, Tz2: TimeZone> PartialOrd<DateTime<Tz2>> for DateTime<Tz> {
    fn partial_cmp(&self, other: &DateTime<Tz2>) -> Option<Ordering> {
        self.datetime.partial_cmp(&other.datetime)
    }
}
impl<Tz: TimeZone> Ord for DateTime<Tz> {
    fn cmp(&self, other: &DateTime<Tz>) -> Ordering {
        self.datetime.cmp(&other.datetime)
    }
}
impl<Tz: TimeZone> hash::Hash for DateTime<Tz> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.datetime.hash(state)
    }
}
impl<Tz: TimeZone> Add<TimeDelta> for DateTime<Tz> {
    type Output = DateTime<Tz>;
    #[inline]
    #[track_caller]
    fn add(self, rhs: TimeDelta) -> DateTime<Tz> {
        self.checked_add_signed(rhs)
            .expect("`DateTime + TimeDelta` overflowed")
    }
}
impl<Tz: TimeZone> Add<Duration> for DateTime<Tz> {
    type Output = DateTime<Tz>;
    #[inline]
    #[track_caller]
    fn add(self, rhs: Duration) -> DateTime<Tz> {
        let rhs = TimeDelta::from_std(rhs)
            .expect("overflow converting from core::time::Duration to TimeDelta");
        self.checked_add_signed(rhs)
            .expect("`DateTime + TimeDelta` overflowed")
    }
}
impl<Tz: TimeZone> AddAssign<TimeDelta> for DateTime<Tz> {
    #[inline]
    #[track_caller]
    fn add_assign(&mut self, rhs: TimeDelta) {
        let datetime = self
            .datetime
            .checked_add_signed(rhs)
            .expect("`DateTime + TimeDelta` overflowed");
        let tz = self.timezone();
        *self = tz.from_utc_datetime(&datetime);
    }
}
impl<Tz: TimeZone> AddAssign<Duration> for DateTime<Tz> {
    #[inline]
    #[track_caller]
    fn add_assign(&mut self, rhs: Duration) {
        let rhs = TimeDelta::from_std(rhs)
            .expect("overflow converting from core::time::Duration to TimeDelta");
        *self += rhs;
    }
}
impl<Tz: TimeZone> Add<FixedOffset> for DateTime<Tz> {
    type Output = DateTime<Tz>;
    #[inline]
    #[track_caller]
    fn add(mut self, rhs: FixedOffset) -> DateTime<Tz> {
        self.datetime = self
            .naive_utc()
            .checked_add_offset(rhs)
            .expect("`DateTime + FixedOffset` overflowed");
        self
    }
}
impl<Tz: TimeZone> Add<Months> for DateTime<Tz> {
    type Output = DateTime<Tz>;
    #[track_caller]
    fn add(self, rhs: Months) -> Self::Output {
        self.checked_add_months(rhs)
            .expect("`DateTime + Months` out of range")
    }
}
impl<Tz: TimeZone> Sub<TimeDelta> for DateTime<Tz> {
    type Output = DateTime<Tz>;
    #[inline]
    #[track_caller]
    fn sub(self, rhs: TimeDelta) -> DateTime<Tz> {
        self.checked_sub_signed(rhs)
            .expect("`DateTime - TimeDelta` overflowed")
    }
}
impl<Tz: TimeZone> Sub<Duration> for DateTime<Tz> {
    type Output = DateTime<Tz>;
    #[inline]
    #[track_caller]
    fn sub(self, rhs: Duration) -> DateTime<Tz> {
        let rhs = TimeDelta::from_std(rhs)
            .expect("overflow converting from core::time::Duration to TimeDelta");
        self.checked_sub_signed(rhs)
            .expect("`DateTime - TimeDelta` overflowed")
    }
}
impl<Tz: TimeZone> SubAssign<TimeDelta> for DateTime<Tz> {
    #[inline]
    #[track_caller]
    fn sub_assign(&mut self, rhs: TimeDelta) {
        let datetime = self
            .datetime
            .checked_sub_signed(rhs)
            .expect("`DateTime - TimeDelta` overflowed");
        let tz = self.timezone();
        *self = tz.from_utc_datetime(&datetime)
    }
}
impl<Tz: TimeZone> SubAssign<Duration> for DateTime<Tz> {
    #[inline]
    #[track_caller]
    fn sub_assign(&mut self, rhs: Duration) {
        let rhs = TimeDelta::from_std(rhs)
            .expect("overflow converting from core::time::Duration to TimeDelta");
        *self -= rhs;
    }
}
impl<Tz: TimeZone> Sub<FixedOffset> for DateTime<Tz> {
    type Output = DateTime<Tz>;
    #[inline]
    #[track_caller]
    fn sub(mut self, rhs: FixedOffset) -> DateTime<Tz> {
        self.datetime = self
            .naive_utc()
            .checked_sub_offset(rhs)
            .expect("`DateTime - FixedOffset` overflowed");
        self
    }
}
impl<Tz: TimeZone> Sub<Months> for DateTime<Tz> {
    type Output = DateTime<Tz>;
    #[track_caller]
    fn sub(self, rhs: Months) -> Self::Output {
        self.checked_sub_months(rhs)
            .expect("`DateTime - Months` out of range")
    }
}
impl<Tz: TimeZone> Sub<DateTime<Tz>> for DateTime<Tz> {
    type Output = TimeDelta;
    #[inline]
    fn sub(self, rhs: DateTime<Tz>) -> TimeDelta {
        self.signed_duration_since(rhs)
    }
}
impl<Tz: TimeZone> Sub<&DateTime<Tz>> for DateTime<Tz> {
    type Output = TimeDelta;
    #[inline]
    fn sub(self, rhs: &DateTime<Tz>) -> TimeDelta {
        self.signed_duration_since(rhs)
    }
}
impl<Tz: TimeZone> Add<Days> for DateTime<Tz> {
    type Output = DateTime<Tz>;
    #[track_caller]
    fn add(self, days: Days) -> Self::Output {
        self.checked_add_days(days)
            .expect("`DateTime + Days` out of range")
    }
}
impl<Tz: TimeZone> Sub<Days> for DateTime<Tz> {
    type Output = DateTime<Tz>;
    #[track_caller]
    fn sub(self, days: Days) -> Self::Output {
        self.checked_sub_days(days)
            .expect("`DateTime - Days` out of range")
    }
}
impl<Tz: TimeZone> fmt::Debug for DateTime<Tz> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.overflowing_naive_local().fmt(f)?;
        self.offset.fmt(f)
    }
}
impl<Tz: TimeZone> fmt::Display for DateTime<Tz>
where
    Tz::Offset: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.overflowing_naive_local().fmt(f)?;
        f.write_char(' ')?;
        self.offset.fmt(f)
    }
}
impl str::FromStr for DateTime<Utc> {
    type Err = ParseError;
    fn from_str(s: &str) -> ParseResult<DateTime<Utc>> {
        s.parse::<DateTime<FixedOffset>>()
            .map(|dt| dt.with_timezone(&Utc))
    }
}
impl From<SystemTime> for DateTime<Utc> {
    fn from(t: SystemTime) -> DateTime<Utc> {
        let (sec, nsec) = match t.duration_since(UNIX_EPOCH) {
            Ok(dur) => (dur.as_secs() as i64, dur.subsec_nanos()),
            Err(e) => {
                let dur = e.duration();
                let (sec, nsec) = (dur.as_secs() as i64, dur.subsec_nanos());
                if nsec == 0 {
                    (-sec, 0)
                } else {
                    (-sec - 1, 1_000_000_000 - nsec)
                }
            }
        };
        Utc.timestamp_opt(sec, nsec).unwrap()
    }
}
impl<Tz: TimeZone> From<DateTime<Tz>> for SystemTime {
    fn from(dt: DateTime<Tz>) -> SystemTime {
        let sec = dt.timestamp();
        let nsec = dt.timestamp_subsec_nanos();
        if sec < 0 {
            UNIX_EPOCH - Duration::new(-sec as u64, 0) + Duration::new(0, nsec)
        } else {
            UNIX_EPOCH + Duration::new(sec as u64, nsec)
        }
    }
}
#[cfg(all(
    target_arch = "wasm32",
    all(),
    not(any(target_os = "emscripten", target_os = "wasi", target_os = "linux"))
))]
impl From<js_sys::Date> for DateTime<Utc> {
    fn from(date: js_sys::Date) -> DateTime<Utc> {
        DateTime::<Utc>::from(&date)
    }
}
#[cfg(all(
    target_arch = "wasm32",
    all(),
    not(any(target_os = "emscripten", target_os = "wasi", target_os = "linux"))
))]
impl From<&js_sys::Date> for DateTime<Utc> {
    fn from(date: &js_sys::Date) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(date.get_time() as i64).unwrap()
    }
}
#[cfg(all(
    target_arch = "wasm32",
    all(),
    not(any(target_os = "emscripten", target_os = "wasi", target_os = "linux"))
))]
impl From<DateTime<Utc>> for js_sys::Date {
    fn from(date: DateTime<Utc>) -> js_sys::Date {
        let js_millis = wasm_bindgen::JsValue::from_f64(date.timestamp_millis() as f64);
        js_sys::Date::new(&js_millis)
    }
}
pub(crate) const UNIX_EPOCH_DAY: i64 = 719_163;
