use super::internals::{Mdf, YearFlags};
use crate::chrono::datetime::UNIX_EPOCH_DAY;
use crate::chrono::format::DelayedFormat;
use crate::chrono::format::{
    Item, Numeric, Pad, ParseError, ParseResult, Parsed, StrftimeItems, parse, parse_and_remainder,
    write_hundreds,
};
use crate::chrono::month::Months;
use crate::chrono::naive::{Days, IsoWeek, NaiveDateTime, NaiveTime, NaiveWeek};
use crate::chrono::{Datelike, TimeDelta, Weekday};
use crate::chrono::{expect, try_opt};
use core::borrow::Borrow;
use core::iter::FusedIterator;
use core::num::NonZeroI32;
use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::{fmt, str};
#[derive(PartialEq, Eq, Hash, PartialOrd, Ord, Copy, Clone)]
pub struct NaiveDate {
    yof: NonZeroI32,
}
pub const MIN_DATE: NaiveDate = NaiveDate::MIN;
pub const MAX_DATE: NaiveDate = NaiveDate::MAX;
impl NaiveDate {
    pub(crate) fn weeks_from(&self, day: Weekday) -> i32 {
        (self.ordinal() as i32 - self.weekday().days_since(day) as i32 + 6) / 7
    }
    const fn from_ordinal_and_flags(
        year: i32,
        ordinal: u32,
        flags: YearFlags,
    ) -> Option<NaiveDate> {
        if year < MIN_YEAR || year > MAX_YEAR {
            return None;
        }
        if ordinal == 0 || ordinal > 366 {
            return None;
        }
        debug_assert!(YearFlags::from_year(year).0 == flags.0);
        let yof = (year << 13) | (ordinal << 4) as i32 | flags.0 as i32;
        match yof & OL_MASK <= MAX_OL {
            true => Some(NaiveDate::from_yof(yof)),
            false => None,
        }
    }
    const fn from_mdf(year: i32, mdf: Mdf) -> Option<NaiveDate> {
        if year < MIN_YEAR || year > MAX_YEAR {
            return None;
        }
        Some(NaiveDate::from_yof(
            (year << 13) | try_opt!(mdf.ordinal_and_flags()),
        ))
    }

    #[must_use]
    pub const fn from_ymd_opt(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
        let flags = YearFlags::from_year(year);
        if let Some(mdf) = Mdf::new(month, day, flags) {
            NaiveDate::from_mdf(year, mdf)
        } else {
            None
        }
    }

    #[must_use]
    pub const fn from_yo_opt(year: i32, ordinal: u32) -> Option<NaiveDate> {
        let flags = YearFlags::from_year(year);
        NaiveDate::from_ordinal_and_flags(year, ordinal, flags)
    }

    #[must_use]
    pub const fn from_isoywd_opt(year: i32, week: u32, weekday: Weekday) -> Option<NaiveDate> {
        let flags = YearFlags::from_year(year);
        let nweeks = flags.nisoweeks();
        if week == 0 || week > nweeks {
            return None;
        }
        let weekord = week * 7 + weekday as u32;
        let delta = flags.isoweek_delta();
        let (year, ordinal, flags) = if weekord <= delta {
            let prevflags = YearFlags::from_year(year - 1);
            (year - 1, weekord + prevflags.ndays() - delta, prevflags)
        } else {
            let ordinal = weekord - delta;
            let ndays = flags.ndays();
            if ordinal <= ndays {
                (year, ordinal, flags)
            } else {
                let nextflags = YearFlags::from_year(year + 1);
                (year + 1, ordinal - ndays, nextflags)
            }
        };
        NaiveDate::from_ordinal_and_flags(year, ordinal, flags)
    }

    #[must_use]
    pub const fn from_num_days_from_ce_opt(days: i32) -> Option<NaiveDate> {
        let days = try_opt!(days.checked_add(365));
        let year_div_400 = days.div_euclid(146_097);
        let cycle = days.rem_euclid(146_097);
        let (year_mod_400, ordinal) = cycle_to_yo(cycle as u32);
        let flags = YearFlags::from_year_mod_400(year_mod_400 as i32);
        NaiveDate::from_ordinal_and_flags(year_div_400 * 400 + year_mod_400 as i32, ordinal, flags)
    }

    #[must_use]
    pub const fn from_weekday_of_month_opt(
        year: i32,
        month: u32,
        weekday: Weekday,
        n: u8,
    ) -> Option<NaiveDate> {
        if n == 0 {
            return None;
        }
        let first = try_opt!(NaiveDate::from_ymd_opt(year, month, 1)).weekday();
        let first_to_dow = (7 + weekday.number_from_monday() - first.number_from_monday()) % 7;
        let day = (n - 1) as u32 * 7 + first_to_dow + 1;
        NaiveDate::from_ymd_opt(year, month, day)
    }
    pub fn parse_from_str(s: &str, fmt: &str) -> ParseResult<NaiveDate> {
        let mut parsed = Parsed::new();
        parse(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_naive_date()
    }
    pub fn parse_and_remainder<'a>(s: &'a str, fmt: &str) -> ParseResult<(NaiveDate, &'a str)> {
        let mut parsed = Parsed::new();
        let remainder = parse_and_remainder(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_naive_date().map(|d| (d, remainder))
    }
    #[must_use]
    pub const fn checked_add_months(self, months: Months) -> Option<Self> {
        if months.0 == 0 {
            return Some(self);
        }
        match months.0 <= i32::MAX as u32 {
            true => self.diff_months(months.0 as i32),
            false => None,
        }
    }
    #[must_use]
    pub const fn checked_sub_months(self, months: Months) -> Option<Self> {
        if months.0 == 0 {
            return Some(self);
        }
        match months.0 <= i32::MAX as u32 {
            true => self.diff_months(-(months.0 as i32)),
            false => None,
        }
    }
    const fn diff_months(self, months: i32) -> Option<Self> {
        let months = try_opt!((self.year() * 12 + self.month() as i32 - 1).checked_add(months));
        let year = months.div_euclid(12);
        let month = months.rem_euclid(12) as u32 + 1;
        let flags = YearFlags::from_year(year);
        let feb_days = if flags.ndays() == 366 { 29 } else { 28 };
        let days = [31, feb_days, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let day_max = days[(month - 1) as usize];
        let mut day = self.day();
        if day > day_max {
            day = day_max;
        };
        NaiveDate::from_ymd_opt(year, month, day)
    }
    #[must_use]
    pub const fn checked_add_days(self, days: Days) -> Option<Self> {
        match days.0 <= i32::MAX as u64 {
            true => self.add_days(days.0 as i32),
            false => None,
        }
    }
    #[must_use]
    pub const fn checked_sub_days(self, days: Days) -> Option<Self> {
        match days.0 <= i32::MAX as u64 {
            true => self.add_days(-(days.0 as i32)),
            false => None,
        }
    }
    pub(crate) const fn add_days(self, days: i32) -> Option<Self> {
        const ORDINAL_MASK: i32 = 0b1_1111_1111_0000;
        if let Some(ordinal) = ((self.yof() & ORDINAL_MASK) >> 4).checked_add(days) {
            if ordinal > 0 && ordinal <= (365 + self.leap_year() as i32) {
                let year_and_flags = self.yof() & !ORDINAL_MASK;
                return Some(NaiveDate::from_yof(year_and_flags | (ordinal << 4)));
            }
        }
        let year = self.year();
        let (mut year_div_400, year_mod_400) = div_mod_floor(year, 400);
        let cycle = yo_to_cycle(year_mod_400 as u32, self.ordinal());
        let cycle = try_opt!((cycle as i32).checked_add(days));
        let (cycle_div_400y, cycle) = div_mod_floor(cycle, 146_097);
        year_div_400 += cycle_div_400y;
        let (year_mod_400, ordinal) = cycle_to_yo(cycle as u32);
        let flags = YearFlags::from_year_mod_400(year_mod_400 as i32);
        NaiveDate::from_ordinal_and_flags(year_div_400 * 400 + year_mod_400 as i32, ordinal, flags)
    }
    #[inline]
    #[must_use]
    pub const fn and_time(&self, time: NaiveTime) -> NaiveDateTime {
        NaiveDateTime::new(*self, time)
    }
    #[inline]
    #[must_use]
    pub const fn and_hms(&self, hour: u32, min: u32, sec: u32) -> NaiveDateTime {
        expect(self.and_hms_opt(hour, min, sec), "invalid time")
    }
    #[inline]
    #[must_use]
    pub const fn and_hms_opt(&self, hour: u32, min: u32, sec: u32) -> Option<NaiveDateTime> {
        let time = try_opt!(NaiveTime::from_hms_opt(hour, min, sec));
        Some(self.and_time(time))
    }
    #[inline]
    #[must_use]
    pub const fn and_hms_milli(&self, hour: u32, min: u32, sec: u32, milli: u32) -> NaiveDateTime {
        expect(
            self.and_hms_milli_opt(hour, min, sec, milli),
            "invalid time",
        )
    }
    #[inline]
    #[must_use]
    pub const fn and_hms_milli_opt(
        &self,
        hour: u32,
        min: u32,
        sec: u32,
        milli: u32,
    ) -> Option<NaiveDateTime> {
        let time = try_opt!(NaiveTime::from_hms_milli_opt(hour, min, sec, milli));
        Some(self.and_time(time))
    }
    #[inline]
    #[must_use]
    pub const fn and_hms_micro(&self, hour: u32, min: u32, sec: u32, micro: u32) -> NaiveDateTime {
        expect(
            self.and_hms_micro_opt(hour, min, sec, micro),
            "invalid time",
        )
    }
    #[inline]
    #[must_use]
    pub const fn and_hms_micro_opt(
        &self,
        hour: u32,
        min: u32,
        sec: u32,
        micro: u32,
    ) -> Option<NaiveDateTime> {
        let time = try_opt!(NaiveTime::from_hms_micro_opt(hour, min, sec, micro));
        Some(self.and_time(time))
    }
    #[inline]
    #[must_use]
    pub const fn and_hms_nano(&self, hour: u32, min: u32, sec: u32, nano: u32) -> NaiveDateTime {
        expect(self.and_hms_nano_opt(hour, min, sec, nano), "invalid time")
    }
    #[inline]
    #[must_use]
    pub const fn and_hms_nano_opt(
        &self,
        hour: u32,
        min: u32,
        sec: u32,
        nano: u32,
    ) -> Option<NaiveDateTime> {
        let time = try_opt!(NaiveTime::from_hms_nano_opt(hour, min, sec, nano));
        Some(self.and_time(time))
    }
    #[inline]
    const fn mdf(&self) -> Mdf {
        Mdf::from_ol((self.yof() & OL_MASK) >> 3, self.year_flags())
    }
    #[inline]
    const fn with_mdf(&self, mdf: Mdf) -> Option<NaiveDate> {
        debug_assert!(self.year_flags().0 == mdf.year_flags().0);
        match mdf.ordinal() {
            Some(ordinal) => Some(NaiveDate::from_yof(
                (self.yof() & !ORDINAL_MASK) | (ordinal << 4) as i32,
            )),
            None => None,
        }
    }
    #[inline]
    #[must_use]
    pub const fn succ(&self) -> NaiveDate {
        expect(self.succ_opt(), "out of bound")
    }
    #[inline]
    #[must_use]
    pub const fn succ_opt(&self) -> Option<NaiveDate> {
        let new_ol = (self.yof() & OL_MASK) + (1 << 4);
        match new_ol <= MAX_OL {
            true => Some(NaiveDate::from_yof(self.yof() & !OL_MASK | new_ol)),
            false => NaiveDate::from_yo_opt(self.year() + 1, 1),
        }
    }
    #[inline]
    #[must_use]
    pub const fn pred(&self) -> NaiveDate {
        expect(self.pred_opt(), "out of bound")
    }
    #[inline]
    #[must_use]
    pub const fn pred_opt(&self) -> Option<NaiveDate> {
        let new_shifted_ordinal = (self.yof() & ORDINAL_MASK) - (1 << 4);
        match new_shifted_ordinal > 0 {
            true => Some(NaiveDate::from_yof(
                self.yof() & !ORDINAL_MASK | new_shifted_ordinal,
            )),
            false => NaiveDate::from_ymd_opt(self.year() - 1, 12, 31),
        }
    }
    #[must_use]
    pub const fn checked_add_signed(self, rhs: TimeDelta) -> Option<NaiveDate> {
        let days = rhs.num_days();
        if days < i32::MIN as i64 || days > i32::MAX as i64 {
            return None;
        }
        self.add_days(days as i32)
    }
    #[must_use]
    pub const fn checked_sub_signed(self, rhs: TimeDelta) -> Option<NaiveDate> {
        let days = -rhs.num_days();
        if days < i32::MIN as i64 || days > i32::MAX as i64 {
            return None;
        }
        self.add_days(days as i32)
    }
    #[must_use]
    pub const fn signed_duration_since(self, rhs: Self) -> TimeDelta {
        let year1 = self.year();
        let year2 = rhs.year();
        let (year1_div_400, year1_mod_400) = div_mod_floor(year1, 400);
        let (year2_div_400, year2_mod_400) = div_mod_floor(year2, 400);
        let cycle1 = yo_to_cycle(year1_mod_400 as u32, self.ordinal()) as i64;
        let cycle2 = yo_to_cycle(year2_mod_400 as u32, rhs.ordinal()) as i64;
        let days = (year1_div_400 as i64 - year2_div_400 as i64) * 146_097 + (cycle1 - cycle2);
        expect(TimeDelta::try_days(days), "always in range")
    }
    pub const fn abs_diff(self, rhs: Self) -> Days {
        Days::new(i32::abs_diff(self.num_days_from_ce(), rhs.num_days_from_ce()) as u64)
    }
    #[must_use]
    pub const fn years_since(&self, base: Self) -> Option<u32> {
        let mut years = self.year() - base.year();
        if ((self.month() << 5) | self.day()) < ((base.month() << 5) | base.day()) {
            years -= 1;
        }
        match years >= 0 {
            true => Some(years as u32),
            false => None,
        }
    }
    #[inline]
    #[must_use]
    pub fn format_with_items<'a, I, B>(&self, items: I) -> DelayedFormat<I>
    where
        I: Iterator<Item = B> + Clone,
        B: Borrow<Item<'a>>,
    {
        DelayedFormat::new(Some(*self), None, items)
    }
    #[inline]
    #[must_use]
    pub fn format<'a>(&self, fmt: &'a str) -> DelayedFormat<StrftimeItems<'a>> {
        self.format_with_items(StrftimeItems::new(fmt))
    }

    #[inline]
    pub const fn week(&self, start: Weekday) -> NaiveWeek {
        NaiveWeek::new(*self, start)
    }
    pub const fn leap_year(&self) -> bool {
        self.yof() & (0b1000) == 0
    }
    #[inline]
    const fn year(&self) -> i32 {
        self.yof() >> 13
    }
    #[inline]
    const fn ordinal(&self) -> u32 {
        ((self.yof() & ORDINAL_MASK) >> 4) as u32
    }
    #[inline]
    const fn month(&self) -> u32 {
        self.mdf().month()
    }
    #[inline]
    const fn day(&self) -> u32 {
        self.mdf().day()
    }
    #[inline]
    pub(super) const fn weekday(&self) -> Weekday {
        match (((self.yof() & ORDINAL_MASK) >> 4) + (self.yof() & WEEKDAY_FLAGS_MASK)) % 7 {
            0 => Weekday::Mon,
            1 => Weekday::Tue,
            2 => Weekday::Wed,
            3 => Weekday::Thu,
            4 => Weekday::Fri,
            5 => Weekday::Sat,
            _ => Weekday::Sun,
        }
    }
    #[inline]
    const fn year_flags(&self) -> YearFlags {
        YearFlags((self.yof() & YEAR_FLAGS_MASK) as u8)
    }
    pub(crate) const fn num_days_from_ce(&self) -> i32 {
        let mut year = self.year() - 1;
        let mut ndays = 0;
        if year < 0 {
            let excess = 1 + (-year) / 400;
            year += excess * 400;
            ndays -= excess * 146_097;
        }
        let div_100 = year / 100;
        ndays += ((year * 1461) >> 2) - div_100 + (div_100 >> 2);
        ndays + self.ordinal() as i32
    }

    #[inline]
    const fn from_yof(yof: i32) -> NaiveDate {
        debug_assert!(((yof & OL_MASK) >> 3) > 1);
        debug_assert!(((yof & OL_MASK) >> 3) <= MAX_OL);
        debug_assert!((yof & 0b111) != 000);
        NaiveDate {
            yof: unsafe { NonZeroI32::new_unchecked(yof) },
        }
    }
    #[inline]
    const fn yof(&self) -> i32 {
        self.yof.get()
    }
    pub const MIN: NaiveDate = NaiveDate::from_yof((MIN_YEAR << 13) | (1 << 4) | 0o12);
    pub const MAX: NaiveDate = NaiveDate::from_yof((MAX_YEAR << 13) | (365 << 4) | 0o16);
    pub(crate) const BEFORE_MIN: NaiveDate =
        NaiveDate::from_yof(((MIN_YEAR - 1) << 13) | (366 << 4) | 0o07);
    pub(crate) const AFTER_MAX: NaiveDate =
        NaiveDate::from_yof(((MAX_YEAR + 1) << 13) | (1 << 4) | 0o17);
}
impl Datelike for NaiveDate {
    #[inline]
    fn year(&self) -> i32 {
        self.year()
    }
    #[inline]
    fn month(&self) -> u32 {
        self.month()
    }
    #[inline]
    fn month0(&self) -> u32 {
        self.month() - 1
    }
    #[inline]
    fn day(&self) -> u32 {
        self.day()
    }
    #[inline]
    fn day0(&self) -> u32 {
        self.mdf().day() - 1
    }
    #[inline]
    fn ordinal(&self) -> u32 {
        ((self.yof() & ORDINAL_MASK) >> 4) as u32
    }
    #[inline]
    fn ordinal0(&self) -> u32 {
        self.ordinal() - 1
    }
    #[inline]
    fn weekday(&self) -> Weekday {
        self.weekday()
    }
    #[inline]
    fn iso_week(&self) -> IsoWeek {
        IsoWeek::from_yof(self.year(), self.ordinal(), self.year_flags())
    }
    #[inline]
    fn with_year(&self, year: i32) -> Option<NaiveDate> {
        let mdf = self.mdf();
        let flags = YearFlags::from_year(year);
        let mdf = mdf.with_flags(flags);
        NaiveDate::from_mdf(year, mdf)
    }
    #[inline]
    fn with_month(&self, month: u32) -> Option<NaiveDate> {
        self.with_mdf(self.mdf().with_month(month)?)
    }
    #[inline]
    fn with_month0(&self, month0: u32) -> Option<NaiveDate> {
        let month = month0.checked_add(1)?;
        self.with_mdf(self.mdf().with_month(month)?)
    }
    #[inline]
    fn with_day(&self, day: u32) -> Option<NaiveDate> {
        self.with_mdf(self.mdf().with_day(day)?)
    }
    #[inline]
    fn with_day0(&self, day0: u32) -> Option<NaiveDate> {
        let day = day0.checked_add(1)?;
        self.with_mdf(self.mdf().with_day(day)?)
    }
    #[inline]
    fn with_ordinal(&self, ordinal: u32) -> Option<NaiveDate> {
        if ordinal == 0 || ordinal > 366 {
            return None;
        }
        let yof = (self.yof() & !ORDINAL_MASK) | (ordinal << 4) as i32;
        match yof & OL_MASK <= MAX_OL {
            true => Some(NaiveDate::from_yof(yof)),
            false => None,
        }
    }
    #[inline]
    fn with_ordinal0(&self, ordinal0: u32) -> Option<NaiveDate> {
        let ordinal = ordinal0.checked_add(1)?;
        self.with_ordinal(ordinal)
    }
}
impl Add<TimeDelta> for NaiveDate {
    type Output = NaiveDate;
    #[inline]
    #[track_caller]
    fn add(self, rhs: TimeDelta) -> NaiveDate {
        self.checked_add_signed(rhs)
            .expect("`NaiveDate + TimeDelta` overflowed")
    }
}
impl AddAssign<TimeDelta> for NaiveDate {
    #[inline]
    #[track_caller]
    fn add_assign(&mut self, rhs: TimeDelta) {
        *self = self.add(rhs);
    }
}
impl Add<Months> for NaiveDate {
    type Output = NaiveDate;
    #[track_caller]
    fn add(self, months: Months) -> Self::Output {
        self.checked_add_months(months)
            .expect("`NaiveDate + Months` out of range")
    }
}
impl Sub<Months> for NaiveDate {
    type Output = NaiveDate;
    #[track_caller]
    fn sub(self, months: Months) -> Self::Output {
        self.checked_sub_months(months)
            .expect("`NaiveDate - Months` out of range")
    }
}
impl Add<Days> for NaiveDate {
    type Output = NaiveDate;
    #[track_caller]
    fn add(self, days: Days) -> Self::Output {
        self.checked_add_days(days)
            .expect("`NaiveDate + Days` out of range")
    }
}
impl Sub<Days> for NaiveDate {
    type Output = NaiveDate;
    #[track_caller]
    fn sub(self, days: Days) -> Self::Output {
        self.checked_sub_days(days)
            .expect("`NaiveDate - Days` out of range")
    }
}
impl Sub<TimeDelta> for NaiveDate {
    type Output = NaiveDate;
    #[inline]
    #[track_caller]
    fn sub(self, rhs: TimeDelta) -> NaiveDate {
        self.checked_sub_signed(rhs)
            .expect("`NaiveDate - TimeDelta` overflowed")
    }
}
impl SubAssign<TimeDelta> for NaiveDate {
    #[inline]
    #[track_caller]
    fn sub_assign(&mut self, rhs: TimeDelta) {
        *self = self.sub(rhs);
    }
}
impl Sub<NaiveDate> for NaiveDate {
    type Output = TimeDelta;
    #[inline]
    fn sub(self, rhs: NaiveDate) -> TimeDelta {
        self.signed_duration_since(rhs)
    }
}
impl From<NaiveDateTime> for NaiveDate {
    fn from(naive_datetime: NaiveDateTime) -> Self {
        naive_datetime.date()
    }
}
#[derive(Debug, Copy, Clone, Hash, PartialEq, PartialOrd, Eq, Ord)]
pub struct NaiveDateDaysIterator {
    value: NaiveDate,
}
impl Iterator for NaiveDateDaysIterator {
    type Item = NaiveDate;
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.value;
        self.value = current.succ_opt()?;
        Some(current)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let exact_size = NaiveDate::MAX.signed_duration_since(self.value).num_days();
        (exact_size as usize, Some(exact_size as usize))
    }
}
impl ExactSizeIterator for NaiveDateDaysIterator {}
impl DoubleEndedIterator for NaiveDateDaysIterator {
    fn next_back(&mut self) -> Option<Self::Item> {
        let current = self.value;
        self.value = current.pred_opt()?;
        Some(current)
    }
}
impl FusedIterator for NaiveDateDaysIterator {}
#[derive(Debug, Copy, Clone, Hash, PartialEq, PartialOrd, Eq, Ord)]
pub struct NaiveDateWeeksIterator {
    value: NaiveDate,
}
impl Iterator for NaiveDateWeeksIterator {
    type Item = NaiveDate;
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.value;
        self.value = current.checked_add_days(Days::new(7))?;
        Some(current)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let exact_size = NaiveDate::MAX.signed_duration_since(self.value).num_weeks();
        (exact_size as usize, Some(exact_size as usize))
    }
}
impl ExactSizeIterator for NaiveDateWeeksIterator {}
impl DoubleEndedIterator for NaiveDateWeeksIterator {
    fn next_back(&mut self) -> Option<Self::Item> {
        let current = self.value;
        self.value = current.checked_sub_days(Days::new(7))?;
        Some(current)
    }
}
impl FusedIterator for NaiveDateWeeksIterator {}
impl fmt::Debug for NaiveDate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use core::fmt::Write;
        let year = self.year();
        let mdf = self.mdf();
        if (0..=9999).contains(&year) {
            write_hundreds(f, (year / 100) as u8)?;
            write_hundreds(f, (year % 100) as u8)?;
        } else {
            write!(f, "{year:+05}")?;
        }
        f.write_char('-')?;
        write_hundreds(f, mdf.month() as u8)?;
        f.write_char('-')?;
        write_hundreds(f, mdf.day() as u8)
    }
}
impl fmt::Display for NaiveDate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
impl str::FromStr for NaiveDate {
    type Err = ParseError;
    fn from_str(s: &str) -> ParseResult<NaiveDate> {
        const ITEMS: &[Item<'static>] = &[
            Item::Numeric(Numeric::Year, Pad::Zero),
            Item::Space(""),
            Item::Literal("-"),
            Item::Numeric(Numeric::Month, Pad::Zero),
            Item::Space(""),
            Item::Literal("-"),
            Item::Numeric(Numeric::Day, Pad::Zero),
            Item::Space(""),
        ];
        let mut parsed = Parsed::new();
        parse(&mut parsed, s, ITEMS.iter())?;
        parsed.to_naive_date()
    }
}
impl Default for NaiveDate {
    fn default() -> Self {
        NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
    }
}
const fn cycle_to_yo(cycle: u32) -> (u32, u32) {
    let mut year_mod_400 = cycle / 365;
    let mut ordinal0 = cycle % 365;
    let delta = YEAR_DELTAS[year_mod_400 as usize] as u32;
    if ordinal0 < delta {
        year_mod_400 -= 1;
        ordinal0 += 365 - YEAR_DELTAS[year_mod_400 as usize] as u32;
    } else {
        ordinal0 -= delta;
    }
    (year_mod_400, ordinal0 + 1)
}
const fn yo_to_cycle(year_mod_400: u32, ordinal: u32) -> u32 {
    year_mod_400 * 365 + YEAR_DELTAS[year_mod_400 as usize] as u32 + ordinal - 1
}
const fn div_mod_floor(val: i32, div: i32) -> (i32, i32) {
    (val.div_euclid(div), val.rem_euclid(div))
}
pub(super) const MAX_YEAR: i32 = (i32::MAX >> 13) - 1;
pub(super) const MIN_YEAR: i32 = (i32::MIN >> 13) + 1;
const ORDINAL_MASK: i32 = 0b1_1111_1111_0000;
const LEAP_YEAR_MASK: i32 = 0b1000;
const OL_MASK: i32 = ORDINAL_MASK | LEAP_YEAR_MASK;
const MAX_OL: i32 = 366 << 4;
const WEEKDAY_FLAGS_MASK: i32 = 0b111;
const YEAR_FLAGS_MASK: i32 = LEAP_YEAR_MASK | WEEKDAY_FLAGS_MASK;
const YEAR_DELTAS: &[u8; 401] = &[
    0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7, 8, 8, 8,
    8, 9, 9, 9, 9, 10, 10, 10, 10, 11, 11, 11, 11, 12, 12, 12, 12, 13, 13, 13, 13, 14, 14, 14, 14,
    15, 15, 15, 15, 16, 16, 16, 16, 17, 17, 17, 17, 18, 18, 18, 18, 19, 19, 19, 19, 20, 20, 20, 20,
    21, 21, 21, 21, 22, 22, 22, 22, 23, 23, 23, 23, 24, 24, 24, 24, 25, 25, 25, 25, 25, 25, 25, 25,
    26, 26, 26, 26, 27, 27, 27, 27, 28, 28, 28, 28, 29, 29, 29, 29, 30, 30, 30, 30, 31, 31, 31, 31,
    32, 32, 32, 32, 33, 33, 33, 33, 34, 34, 34, 34, 35, 35, 35, 35, 36, 36, 36, 36, 37, 37, 37, 37,
    38, 38, 38, 38, 39, 39, 39, 39, 40, 40, 40, 40, 41, 41, 41, 41, 42, 42, 42, 42, 43, 43, 43, 43,
    44, 44, 44, 44, 45, 45, 45, 45, 46, 46, 46, 46, 47, 47, 47, 47, 48, 48, 48, 48, 49, 49, 49, 49,
    49, 49, 49, 49, 50, 50, 50, 50, 51, 51, 51, 51, 52, 52, 52, 52, 53, 53, 53, 53, 54, 54, 54, 54,
    55, 55, 55, 55, 56, 56, 56, 56, 57, 57, 57, 57, 58, 58, 58, 58, 59, 59, 59, 59, 60, 60, 60, 60,
    61, 61, 61, 61, 62, 62, 62, 62, 63, 63, 63, 63, 64, 64, 64, 64, 65, 65, 65, 65, 66, 66, 66, 66,
    67, 67, 67, 67, 68, 68, 68, 68, 69, 69, 69, 69, 70, 70, 70, 70, 71, 71, 71, 71, 72, 72, 72, 72,
    73, 73, 73, 73, 73, 73, 73, 73, 74, 74, 74, 74, 75, 75, 75, 75, 76, 76, 76, 76, 77, 77, 77, 77,
    78, 78, 78, 78, 79, 79, 79, 79, 80, 80, 80, 80, 81, 81, 81, 81, 82, 82, 82, 82, 83, 83, 83, 83,
    84, 84, 84, 84, 85, 85, 85, 85, 86, 86, 86, 86, 87, 87, 87, 87, 88, 88, 88, 88, 89, 89, 89, 89,
    90, 90, 90, 90, 91, 91, 91, 91, 92, 92, 92, 92, 93, 93, 93, 93, 94, 94, 94, 94, 95, 95, 95, 95,
    96, 96, 96, 96, 97, 97, 97, 97,
];
mod serde {
    use super::NaiveDate;
    use core::fmt;
    use serde::{de, ser};
    impl ser::Serialize for NaiveDate {
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
    struct NaiveDateVisitor;
    impl de::Visitor<'_> for NaiveDateVisitor {
        type Value = NaiveDate;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a formatted date string")
        }
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            value.parse().map_err(E::custom)
        }
    }
    impl<'de> de::Deserialize<'de> for NaiveDate {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            deserializer.deserialize_str(NaiveDateVisitor)
        }
    }
}
