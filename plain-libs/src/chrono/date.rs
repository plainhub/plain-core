use crate::chrono::format::{DelayedFormat, Item, StrftimeItems};
use crate::chrono::naive::{IsoWeek, NaiveDate, NaiveTime};
use crate::chrono::offset::{TimeZone, Utc};
use crate::chrono::{DateTime, Datelike, TimeDelta, Weekday};
use core::borrow::Borrow;
use core::cmp::Ordering;
use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::{fmt, hash};
#[derive(Clone)]
#[cfg_attr(any(), derive(Archive, Deserialize, Serialize))]
pub struct Date<Tz: TimeZone> {
    date: NaiveDate,
    offset: Tz::Offset,
}
pub const MIN_DATE: Date<Utc> = Date::<Utc>::MIN_UTC;
pub const MAX_DATE: Date<Utc> = Date::<Utc>::MAX_UTC;
impl<Tz: TimeZone> Date<Tz> {
    #[inline]
    #[must_use]
    pub fn from_utc(date: NaiveDate, offset: Tz::Offset) -> Date<Tz> {
        Date { date, offset }
    }
    #[inline]
    #[must_use]
    pub fn and_time(&self, time: NaiveTime) -> Option<DateTime<Tz>> {
        let localdt = self.naive_local().and_time(time);
        self.timezone().from_local_datetime(&localdt).single()
    }
    #[inline]
    #[must_use]
    pub fn and_hms(&self, hour: u32, min: u32, sec: u32) -> DateTime<Tz> {
        self.and_hms_opt(hour, min, sec).expect("invalid time")
    }
    #[inline]
    #[must_use]
    pub fn and_hms_opt(&self, hour: u32, min: u32, sec: u32) -> Option<DateTime<Tz>> {
        NaiveTime::from_hms_opt(hour, min, sec).and_then(|time| self.and_time(time))
    }
    #[inline]
    #[must_use]
    pub fn and_hms_milli(&self, hour: u32, min: u32, sec: u32, milli: u32) -> DateTime<Tz> {
        self.and_hms_milli_opt(hour, min, sec, milli)
            .expect("invalid time")
    }
    #[inline]
    #[must_use]
    pub fn and_hms_milli_opt(
        &self,
        hour: u32,
        min: u32,
        sec: u32,
        milli: u32,
    ) -> Option<DateTime<Tz>> {
        NaiveTime::from_hms_milli_opt(hour, min, sec, milli).and_then(|time| self.and_time(time))
    }
    #[inline]
    #[must_use]
    pub fn and_hms_micro(&self, hour: u32, min: u32, sec: u32, micro: u32) -> DateTime<Tz> {
        self.and_hms_micro_opt(hour, min, sec, micro)
            .expect("invalid time")
    }
    #[inline]
    #[must_use]
    pub fn and_hms_micro_opt(
        &self,
        hour: u32,
        min: u32,
        sec: u32,
        micro: u32,
    ) -> Option<DateTime<Tz>> {
        NaiveTime::from_hms_micro_opt(hour, min, sec, micro).and_then(|time| self.and_time(time))
    }
    #[inline]
    #[must_use]
    pub fn and_hms_nano(&self, hour: u32, min: u32, sec: u32, nano: u32) -> DateTime<Tz> {
        self.and_hms_nano_opt(hour, min, sec, nano)
            .expect("invalid time")
    }
    #[inline]
    #[must_use]
    pub fn and_hms_nano_opt(
        &self,
        hour: u32,
        min: u32,
        sec: u32,
        nano: u32,
    ) -> Option<DateTime<Tz>> {
        NaiveTime::from_hms_nano_opt(hour, min, sec, nano).and_then(|time| self.and_time(time))
    }
    #[inline]
    #[must_use]
    pub fn succ(&self) -> Date<Tz> {
        self.succ_opt().expect("out of bound")
    }
    #[inline]
    #[must_use]
    pub fn succ_opt(&self) -> Option<Date<Tz>> {
        self.date
            .succ_opt()
            .map(|date| Date::from_utc(date, self.offset.clone()))
    }
    #[inline]
    #[must_use]
    pub fn pred(&self) -> Date<Tz> {
        self.pred_opt().expect("out of bound")
    }
    #[inline]
    #[must_use]
    pub fn pred_opt(&self) -> Option<Date<Tz>> {
        self.date
            .pred_opt()
            .map(|date| Date::from_utc(date, self.offset.clone()))
    }
    #[inline]
    #[must_use]
    pub fn offset(&self) -> &Tz::Offset {
        &self.offset
    }
    #[inline]
    #[must_use]
    pub fn timezone(&self) -> Tz {
        TimeZone::from_offset(&self.offset)
    }
    #[inline]
    #[must_use]
    pub fn with_timezone<Tz2: TimeZone>(&self, tz: &Tz2) -> Date<Tz2> {
        tz.from_utc_date(&self.date)
    }
    #[inline]
    #[must_use]
    pub fn checked_add_signed(self, rhs: TimeDelta) -> Option<Date<Tz>> {
        let date = self.date.checked_add_signed(rhs)?;
        Some(Date {
            date,
            offset: self.offset,
        })
    }
    #[inline]
    #[must_use]
    pub fn checked_sub_signed(self, rhs: TimeDelta) -> Option<Date<Tz>> {
        let date = self.date.checked_sub_signed(rhs)?;
        Some(Date {
            date,
            offset: self.offset,
        })
    }
    #[inline]
    #[must_use]
    pub fn signed_duration_since<Tz2: TimeZone>(self, rhs: Date<Tz2>) -> TimeDelta {
        self.date.signed_duration_since(rhs.date)
    }
    #[inline]
    #[must_use]
    pub fn naive_utc(&self) -> NaiveDate {
        self.date
    }
    #[inline]
    #[must_use]
    pub fn naive_local(&self) -> NaiveDate {
        self.date
    }
    #[must_use]
    pub fn years_since(&self, base: Self) -> Option<u32> {
        self.date.years_since(base.date)
    }
    pub const MIN_UTC: Date<Utc> = Date {
        date: NaiveDate::MIN,
        offset: Utc,
    };
    pub const MAX_UTC: Date<Utc> = Date {
        date: NaiveDate::MAX,
        offset: Utc,
    };
}
fn map_local<Tz: TimeZone, F>(d: &Date<Tz>, mut f: F) -> Option<Date<Tz>>
where
    F: FnMut(NaiveDate) -> Option<NaiveDate>,
{
    f(d.naive_local()).and_then(|date| d.timezone().from_local_date(&date).single())
}
impl<Tz: TimeZone> Date<Tz>
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
        DelayedFormat::new_with_offset(Some(self.naive_local()), None, &self.offset, items)
    }
    #[inline]
    #[must_use]
    pub fn format<'a>(&self, fmt: &'a str) -> DelayedFormat<StrftimeItems<'a>> {
        self.format_with_items(StrftimeItems::new(fmt))
    }
}
impl<Tz: TimeZone> Datelike for Date<Tz> {
    #[inline]
    fn year(&self) -> i32 {
        self.naive_local().year()
    }
    #[inline]
    fn month(&self) -> u32 {
        self.naive_local().month()
    }
    #[inline]
    fn month0(&self) -> u32 {
        self.naive_local().month0()
    }
    #[inline]
    fn day(&self) -> u32 {
        self.naive_local().day()
    }
    #[inline]
    fn day0(&self) -> u32 {
        self.naive_local().day0()
    }
    #[inline]
    fn ordinal(&self) -> u32 {
        self.naive_local().ordinal()
    }
    #[inline]
    fn ordinal0(&self) -> u32 {
        self.naive_local().ordinal0()
    }
    #[inline]
    fn weekday(&self) -> Weekday {
        self.naive_local().weekday()
    }
    #[inline]
    fn iso_week(&self) -> IsoWeek {
        self.naive_local().iso_week()
    }
    #[inline]
    fn with_year(&self, year: i32) -> Option<Date<Tz>> {
        map_local(self, |date| date.with_year(year))
    }
    #[inline]
    fn with_month(&self, month: u32) -> Option<Date<Tz>> {
        map_local(self, |date| date.with_month(month))
    }
    #[inline]
    fn with_month0(&self, month0: u32) -> Option<Date<Tz>> {
        map_local(self, |date| date.with_month0(month0))
    }
    #[inline]
    fn with_day(&self, day: u32) -> Option<Date<Tz>> {
        map_local(self, |date| date.with_day(day))
    }
    #[inline]
    fn with_day0(&self, day0: u32) -> Option<Date<Tz>> {
        map_local(self, |date| date.with_day0(day0))
    }
    #[inline]
    fn with_ordinal(&self, ordinal: u32) -> Option<Date<Tz>> {
        map_local(self, |date| date.with_ordinal(ordinal))
    }
    #[inline]
    fn with_ordinal0(&self, ordinal0: u32) -> Option<Date<Tz>> {
        map_local(self, |date| date.with_ordinal0(ordinal0))
    }
}
impl<Tz: TimeZone> Copy for Date<Tz> where <Tz as TimeZone>::Offset: Copy {}
unsafe impl<Tz: TimeZone> Send for Date<Tz> where <Tz as TimeZone>::Offset: Send {}
impl<Tz: TimeZone, Tz2: TimeZone> PartialEq<Date<Tz2>> for Date<Tz> {
    fn eq(&self, other: &Date<Tz2>) -> bool {
        self.date == other.date
    }
}
impl<Tz: TimeZone> Eq for Date<Tz> {}
impl<Tz: TimeZone> PartialOrd for Date<Tz> {
    fn partial_cmp(&self, other: &Date<Tz>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<Tz: TimeZone> Ord for Date<Tz> {
    fn cmp(&self, other: &Date<Tz>) -> Ordering {
        self.date.cmp(&other.date)
    }
}
impl<Tz: TimeZone> hash::Hash for Date<Tz> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.date.hash(state)
    }
}
impl<Tz: TimeZone> Add<TimeDelta> for Date<Tz> {
    type Output = Date<Tz>;
    #[inline]
    #[track_caller]
    fn add(self, rhs: TimeDelta) -> Date<Tz> {
        self.checked_add_signed(rhs)
            .expect("`Date + TimeDelta` overflowed")
    }
}
impl<Tz: TimeZone> AddAssign<TimeDelta> for Date<Tz> {
    #[inline]
    #[track_caller]
    fn add_assign(&mut self, rhs: TimeDelta) {
        self.date = self
            .date
            .checked_add_signed(rhs)
            .expect("`Date + TimeDelta` overflowed");
    }
}
impl<Tz: TimeZone> Sub<TimeDelta> for Date<Tz> {
    type Output = Date<Tz>;
    #[inline]
    #[track_caller]
    fn sub(self, rhs: TimeDelta) -> Date<Tz> {
        self.checked_sub_signed(rhs)
            .expect("`Date - TimeDelta` overflowed")
    }
}
impl<Tz: TimeZone> SubAssign<TimeDelta> for Date<Tz> {
    #[inline]
    #[track_caller]
    fn sub_assign(&mut self, rhs: TimeDelta) {
        self.date = self
            .date
            .checked_sub_signed(rhs)
            .expect("`Date - TimeDelta` overflowed");
    }
}
impl<Tz: TimeZone> Sub<Date<Tz>> for Date<Tz> {
    type Output = TimeDelta;
    #[inline]
    fn sub(self, rhs: Date<Tz>) -> TimeDelta {
        self.signed_duration_since(rhs)
    }
}
impl<Tz: TimeZone> fmt::Debug for Date<Tz> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.naive_local().fmt(f)?;
        self.offset.fmt(f)
    }
}
impl<Tz: TimeZone> fmt::Display for Date<Tz>
where
    Tz::Offset: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.naive_local().fmt(f)?;
        self.offset.fmt(f)
    }
}
