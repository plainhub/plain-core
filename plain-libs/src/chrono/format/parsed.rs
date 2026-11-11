use super::{IMPOSSIBLE, NOT_ENOUGH, OUT_OF_RANGE, ParseResult};
use crate::chrono::naive::{NaiveDate, NaiveDateTime, NaiveTime};
use crate::chrono::offset::{FixedOffset, MappedLocalTime, Offset, TimeZone};
use crate::chrono::{DateTime, Datelike, TimeDelta, Timelike, Weekday};
#[derive(Clone, PartialEq, Eq, Debug, Default, Hash)]
#[cfg_attr(any(), derive(defmt::Format))]
pub struct Parsed {
    pub year: Option<i32>,
    pub year_div_100: Option<i32>,
    pub year_mod_100: Option<i32>,
    pub isoyear: Option<i32>,
    pub isoyear_div_100: Option<i32>,
    pub isoyear_mod_100: Option<i32>,
    pub quarter: Option<u32>,
    pub month: Option<u32>,
    pub week_from_sun: Option<u32>,
    pub week_from_mon: Option<u32>,
    pub isoweek: Option<u32>,
    pub weekday: Option<Weekday>,
    pub ordinal: Option<u32>,
    pub day: Option<u32>,
    pub hour_div_12: Option<u32>,
    pub hour_mod_12: Option<u32>,
    pub minute: Option<u32>,
    pub second: Option<u32>,
    pub nanosecond: Option<u32>,
    pub timestamp: Option<i64>,
    pub offset: Option<i32>,
    _dummy: (),
}
#[inline]
fn set_if_consistent<T: PartialEq>(old: &mut Option<T>, new: T) -> ParseResult<()> {
    match old {
        Some(old) if *old != new => Err(IMPOSSIBLE),
        _ => {
            *old = Some(new);
            Ok(())
        }
    }
}
impl Parsed {
    #[must_use]
    pub fn new() -> Parsed {
        Parsed::default()
    }
    #[inline]
    pub fn set_year(&mut self, value: i64) -> ParseResult<()> {
        set_if_consistent(
            &mut self.year,
            i32::try_from(value).map_err(|_| OUT_OF_RANGE)?,
        )
    }
    #[inline]
    pub fn set_year_div_100(&mut self, value: i64) -> ParseResult<()> {
        if !(0..=i32::MAX as i64).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.year_div_100, value as i32)
    }
    #[inline]
    pub fn set_year_mod_100(&mut self, value: i64) -> ParseResult<()> {
        if !(0..100).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.year_mod_100, value as i32)
    }
    #[inline]
    pub fn set_isoyear(&mut self, value: i64) -> ParseResult<()> {
        set_if_consistent(
            &mut self.isoyear,
            i32::try_from(value).map_err(|_| OUT_OF_RANGE)?,
        )
    }
    #[inline]
    pub fn set_isoyear_div_100(&mut self, value: i64) -> ParseResult<()> {
        if !(0..=i32::MAX as i64).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.isoyear_div_100, value as i32)
    }
    #[inline]
    pub fn set_isoyear_mod_100(&mut self, value: i64) -> ParseResult<()> {
        if !(0..100).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.isoyear_mod_100, value as i32)
    }
    #[inline]
    pub fn set_quarter(&mut self, value: i64) -> ParseResult<()> {
        if !(1..=4).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.quarter, value as u32)
    }
    #[inline]
    pub fn set_month(&mut self, value: i64) -> ParseResult<()> {
        if !(1..=12).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.month, value as u32)
    }
    #[inline]
    pub fn set_week_from_sun(&mut self, value: i64) -> ParseResult<()> {
        if !(0..=53).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.week_from_sun, value as u32)
    }
    #[inline]
    pub fn set_week_from_mon(&mut self, value: i64) -> ParseResult<()> {
        if !(0..=53).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.week_from_mon, value as u32)
    }
    #[inline]
    pub fn set_isoweek(&mut self, value: i64) -> ParseResult<()> {
        if !(1..=53).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.isoweek, value as u32)
    }
    #[inline]
    pub fn set_weekday(&mut self, value: Weekday) -> ParseResult<()> {
        set_if_consistent(&mut self.weekday, value)
    }
    #[inline]
    pub fn set_ordinal(&mut self, value: i64) -> ParseResult<()> {
        if !(1..=366).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.ordinal, value as u32)
    }
    #[inline]
    pub fn set_day(&mut self, value: i64) -> ParseResult<()> {
        if !(1..=31).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.day, value as u32)
    }
    #[inline]
    pub fn set_ampm(&mut self, value: bool) -> ParseResult<()> {
        set_if_consistent(&mut self.hour_div_12, value as u32)
    }
    #[inline]
    pub fn set_hour12(&mut self, mut value: i64) -> ParseResult<()> {
        if !(1..=12).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        if value == 12 {
            value = 0
        }
        set_if_consistent(&mut self.hour_mod_12, value as u32)
    }
    #[inline]
    pub fn set_hour(&mut self, value: i64) -> ParseResult<()> {
        let (hour_div_12, hour_mod_12) = match value {
            hour @ 0..=11 => (0, hour as u32),
            hour @ 12..=23 => (1, hour as u32 - 12),
            _ => return Err(OUT_OF_RANGE),
        };
        set_if_consistent(&mut self.hour_div_12, hour_div_12)?;
        set_if_consistent(&mut self.hour_mod_12, hour_mod_12)
    }
    #[inline]
    pub fn set_minute(&mut self, value: i64) -> ParseResult<()> {
        if !(0..=59).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.minute, value as u32)
    }
    #[inline]
    pub fn set_second(&mut self, value: i64) -> ParseResult<()> {
        if !(0..=60).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.second, value as u32)
    }
    #[inline]
    pub fn set_nanosecond(&mut self, value: i64) -> ParseResult<()> {
        if !(0..=999_999_999).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.nanosecond, value as u32)
    }
    #[inline]
    pub fn set_timestamp(&mut self, value: i64) -> ParseResult<()> {
        set_if_consistent(&mut self.timestamp, value)
    }
    #[inline]
    pub fn set_offset(&mut self, value: i64) -> ParseResult<()> {
        set_if_consistent(
            &mut self.offset,
            i32::try_from(value).map_err(|_| OUT_OF_RANGE)?,
        )
    }
    pub fn to_naive_date(&self) -> ParseResult<NaiveDate> {
        fn resolve_year(
            y: Option<i32>,
            q: Option<i32>,
            r: Option<i32>,
        ) -> ParseResult<Option<i32>> {
            match (y, q, r) {
                (y, None, None) => Ok(y),
                (Some(y), q, r @ Some(0..=99)) | (Some(y), q, r @ None) => {
                    if y < 0 {
                        return Err(IMPOSSIBLE);
                    }
                    let q_ = y / 100;
                    let r_ = y % 100;
                    if q.unwrap_or(q_) == q_ && r.unwrap_or(r_) == r_ {
                        Ok(Some(y))
                    } else {
                        Err(IMPOSSIBLE)
                    }
                }
                (None, Some(q), Some(r @ 0..=99)) => {
                    if q < 0 {
                        return Err(IMPOSSIBLE);
                    }
                    let y = q.checked_mul(100).and_then(|v| v.checked_add(r));
                    Ok(Some(y.ok_or(OUT_OF_RANGE)?))
                }
                (None, None, Some(r @ 0..=99)) => Ok(Some(r + if r < 70 { 2000 } else { 1900 })),
                (None, Some(_), None) => Err(NOT_ENOUGH),
                (_, _, Some(_)) => Err(OUT_OF_RANGE),
            }
        }
        let given_year = resolve_year(self.year, self.year_div_100, self.year_mod_100)?;
        let given_isoyear = resolve_year(self.isoyear, self.isoyear_div_100, self.isoyear_mod_100)?;
        let verify_ymd = |date: NaiveDate| {
            let year = date.year();
            let (year_div_100, year_mod_100) = if year >= 0 {
                (Some(year / 100), Some(year % 100))
            } else {
                (None, None)
            };
            let month = date.month();
            let day = date.day();
            self.year.unwrap_or(year) == year
                && self.year_div_100.or(year_div_100) == year_div_100
                && self.year_mod_100.or(year_mod_100) == year_mod_100
                && self.month.unwrap_or(month) == month
                && self.day.unwrap_or(day) == day
        };
        let verify_isoweekdate = |date: NaiveDate| {
            let week = date.iso_week();
            let isoyear = week.year();
            let isoweek = week.week();
            let weekday = date.weekday();
            let (isoyear_div_100, isoyear_mod_100) = if isoyear >= 0 {
                (Some(isoyear / 100), Some(isoyear % 100))
            } else {
                (None, None)
            };
            self.isoyear.unwrap_or(isoyear) == isoyear
                && self.isoyear_div_100.or(isoyear_div_100) == isoyear_div_100
                && self.isoyear_mod_100.or(isoyear_mod_100) == isoyear_mod_100
                && self.isoweek.unwrap_or(isoweek) == isoweek
                && self.weekday.unwrap_or(weekday) == weekday
        };
        let verify_ordinal = |date: NaiveDate| {
            let ordinal = date.ordinal();
            let week_from_sun = date.weeks_from(Weekday::Sun);
            let week_from_mon = date.weeks_from(Weekday::Mon);
            self.ordinal.unwrap_or(ordinal) == ordinal
                && self.week_from_sun.map_or(week_from_sun, |v| v as i32) == week_from_sun
                && self.week_from_mon.map_or(week_from_mon, |v| v as i32) == week_from_mon
        };
        let (verified, parsed_date) = match (given_year, given_isoyear, self) {
            (
                Some(year),
                _,
                &Parsed {
                    month: Some(month),
                    day: Some(day),
                    ..
                },
            ) => {
                let date = NaiveDate::from_ymd_opt(year, month, day).ok_or(OUT_OF_RANGE)?;
                (verify_isoweekdate(date) && verify_ordinal(date), date)
            }
            (
                Some(year),
                _,
                &Parsed {
                    ordinal: Some(ordinal),
                    ..
                },
            ) => {
                let date = NaiveDate::from_yo_opt(year, ordinal).ok_or(OUT_OF_RANGE)?;
                (
                    verify_ymd(date) && verify_isoweekdate(date) && verify_ordinal(date),
                    date,
                )
            }
            (
                Some(year),
                _,
                &Parsed {
                    week_from_sun: Some(week),
                    weekday: Some(weekday),
                    ..
                },
            ) => {
                let date = resolve_week_date(year, week, weekday, Weekday::Sun)?;
                (
                    verify_ymd(date) && verify_isoweekdate(date) && verify_ordinal(date),
                    date,
                )
            }
            (
                Some(year),
                _,
                &Parsed {
                    week_from_mon: Some(week),
                    weekday: Some(weekday),
                    ..
                },
            ) => {
                let date = resolve_week_date(year, week, weekday, Weekday::Mon)?;
                (
                    verify_ymd(date) && verify_isoweekdate(date) && verify_ordinal(date),
                    date,
                )
            }
            (
                _,
                Some(isoyear),
                &Parsed {
                    isoweek: Some(isoweek),
                    weekday: Some(weekday),
                    ..
                },
            ) => {
                let date = NaiveDate::from_isoywd_opt(isoyear, isoweek, weekday);
                let date = date.ok_or(OUT_OF_RANGE)?;
                (verify_ymd(date) && verify_ordinal(date), date)
            }
            (_, _, _) => return Err(NOT_ENOUGH),
        };
        if !verified {
            return Err(IMPOSSIBLE);
        } else if let Some(parsed) = self.quarter {
            if parsed != parsed_date.quarter() {
                return Err(IMPOSSIBLE);
            }
        }
        Ok(parsed_date)
    }
    pub fn to_naive_time(&self) -> ParseResult<NaiveTime> {
        let hour_div_12 = match self.hour_div_12 {
            Some(v @ 0..=1) => v,
            Some(_) => return Err(OUT_OF_RANGE),
            None => return Err(NOT_ENOUGH),
        };
        let hour_mod_12 = match self.hour_mod_12 {
            Some(v @ 0..=11) => v,
            Some(_) => return Err(OUT_OF_RANGE),
            None => return Err(NOT_ENOUGH),
        };
        let hour = hour_div_12 * 12 + hour_mod_12;
        let minute = match self.minute {
            Some(v @ 0..=59) => v,
            Some(_) => return Err(OUT_OF_RANGE),
            None => return Err(NOT_ENOUGH),
        };
        let (second, mut nano) = match self.second.unwrap_or(0) {
            v @ 0..=59 => (v, 0),
            60 => (59, 1_000_000_000),
            _ => return Err(OUT_OF_RANGE),
        };
        nano += match self.nanosecond {
            Some(v @ 0..=999_999_999) if self.second.is_some() => v,
            Some(0..=999_999_999) => return Err(NOT_ENOUGH),
            Some(_) => return Err(OUT_OF_RANGE),
            None => 0,
        };
        NaiveTime::from_hms_nano_opt(hour, minute, second, nano).ok_or(OUT_OF_RANGE)
    }
    pub fn to_naive_datetime_with_offset(&self, offset: i32) -> ParseResult<NaiveDateTime> {
        let date = self.to_naive_date();
        let time = self.to_naive_time();
        if let (Ok(date), Ok(time)) = (date, time) {
            let datetime = date.and_time(time);
            let timestamp = datetime.and_utc().timestamp() - i64::from(offset);
            if let Some(given_timestamp) = self.timestamp {
                if given_timestamp != timestamp
                    && !(datetime.nanosecond() >= 1_000_000_000 && given_timestamp == timestamp + 1)
                {
                    return Err(IMPOSSIBLE);
                }
            }
            Ok(datetime)
        } else if let Some(timestamp) = self.timestamp {
            use super::ParseError as PE;
            use super::ParseErrorKind::{Impossible, OutOfRange};
            match (date, time) {
                (Err(PE(OutOfRange)), _) | (_, Err(PE(OutOfRange))) => return Err(OUT_OF_RANGE),
                (Err(PE(Impossible)), _) | (_, Err(PE(Impossible))) => return Err(IMPOSSIBLE),
                (_, _) => {}
            }
            let ts = timestamp
                .checked_add(i64::from(offset))
                .ok_or(OUT_OF_RANGE)?;
            let mut datetime = DateTime::from_timestamp_secs(ts)
                .ok_or(OUT_OF_RANGE)?
                .naive_utc();
            let mut parsed = self.clone();
            if parsed.second == Some(60) {
                match datetime.second() {
                    59 => {}
                    0 => {
                        datetime -= TimeDelta::try_seconds(1).unwrap();
                    }
                    _ => return Err(IMPOSSIBLE),
                }
            } else {
                parsed.set_second(i64::from(datetime.second()))?;
            }
            parsed.set_year(i64::from(datetime.year()))?;
            parsed.set_ordinal(i64::from(datetime.ordinal()))?;
            parsed.set_hour(i64::from(datetime.hour()))?;
            parsed.set_minute(i64::from(datetime.minute()))?;
            let date = parsed.to_naive_date()?;
            let time = parsed.to_naive_time()?;
            Ok(date.and_time(time))
        } else {
            date?;
            time?;
            unreachable!()
        }
    }

    pub fn to_datetime(&self) -> ParseResult<DateTime<FixedOffset>> {
        let offset = match (self.offset, self.timestamp) {
            (Some(off), _) => off,
            (None, Some(_)) => 0,
            (None, None) => return Err(NOT_ENOUGH),
        };
        let datetime = self.to_naive_datetime_with_offset(offset)?;
        let offset = FixedOffset::east_opt(offset).ok_or(OUT_OF_RANGE)?;
        match offset.from_local_datetime(&datetime) {
            MappedLocalTime::None => Err(IMPOSSIBLE),
            MappedLocalTime::Single(t) => Ok(t),
            MappedLocalTime::Ambiguous(..) => Err(NOT_ENOUGH),
        }
    }
    pub fn to_datetime_with_timezone<Tz: TimeZone>(&self, tz: &Tz) -> ParseResult<DateTime<Tz>> {
        let mut guessed_offset = 0;
        if let Some(timestamp) = self.timestamp {
            let nanosecond = self.nanosecond.unwrap_or(0);
            let dt = DateTime::from_timestamp(timestamp, nanosecond)
                .ok_or(OUT_OF_RANGE)?
                .naive_utc();
            guessed_offset = tz.offset_from_utc_datetime(&dt).fix().local_minus_utc();
        }
        let check_offset = |dt: &DateTime<Tz>| {
            if let Some(offset) = self.offset {
                dt.offset().fix().local_minus_utc() == offset
            } else {
                true
            }
        };
        let datetime = self.to_naive_datetime_with_offset(guessed_offset)?;
        match tz.from_local_datetime(&datetime) {
            MappedLocalTime::None => Err(IMPOSSIBLE),
            MappedLocalTime::Single(t) => {
                if check_offset(&t) {
                    Ok(t)
                } else {
                    Err(IMPOSSIBLE)
                }
            }
            MappedLocalTime::Ambiguous(min, max) => {
                match (check_offset(&min), check_offset(&max)) {
                    (false, false) => Err(IMPOSSIBLE),
                    (false, true) => Ok(max),
                    (true, false) => Ok(min),
                    (true, true) => Err(NOT_ENOUGH),
                }
            }
        }
    }
    #[inline]
    pub fn year(&self) -> Option<i32> {
        self.year
    }
    #[inline]
    pub fn year_div_100(&self) -> Option<i32> {
        self.year_div_100
    }
    #[inline]
    pub fn year_mod_100(&self) -> Option<i32> {
        self.year_mod_100
    }
    #[inline]
    pub fn isoyear(&self) -> Option<i32> {
        self.isoyear
    }
    #[inline]
    pub fn isoyear_div_100(&self) -> Option<i32> {
        self.isoyear_div_100
    }
    #[inline]
    pub fn isoyear_mod_100(&self) -> Option<i32> {
        self.isoyear_mod_100
    }
    #[inline]
    pub fn quarter(&self) -> Option<u32> {
        self.quarter
    }
    #[inline]
    pub fn month(&self) -> Option<u32> {
        self.month
    }
    #[inline]
    pub fn week_from_sun(&self) -> Option<u32> {
        self.week_from_sun
    }
    #[inline]
    pub fn week_from_mon(&self) -> Option<u32> {
        self.week_from_mon
    }
    #[inline]
    pub fn isoweek(&self) -> Option<u32> {
        self.isoweek
    }
    #[inline]
    pub fn weekday(&self) -> Option<Weekday> {
        self.weekday
    }
    #[inline]
    pub fn ordinal(&self) -> Option<u32> {
        self.ordinal
    }
    #[inline]
    pub fn day(&self) -> Option<u32> {
        self.day
    }
    #[inline]
    pub fn hour_div_12(&self) -> Option<u32> {
        self.hour_div_12
    }
    pub fn hour_mod_12(&self) -> Option<u32> {
        self.hour_mod_12
    }
    #[inline]
    pub fn minute(&self) -> Option<u32> {
        self.minute
    }
    #[inline]
    pub fn second(&self) -> Option<u32> {
        self.second
    }
    #[inline]
    pub fn nanosecond(&self) -> Option<u32> {
        self.nanosecond
    }
    #[inline]
    pub fn timestamp(&self) -> Option<i64> {
        self.timestamp
    }
    #[inline]
    pub fn offset(&self) -> Option<i32> {
        self.offset
    }
}
fn resolve_week_date(
    year: i32,
    week: u32,
    weekday: Weekday,
    week_start_day: Weekday,
) -> ParseResult<NaiveDate> {
    if week > 53 {
        return Err(OUT_OF_RANGE);
    }
    let first_day_of_year = NaiveDate::from_yo_opt(year, 1).ok_or(OUT_OF_RANGE)?;
    let first_week_start = 1 + week_start_day.days_since(first_day_of_year.weekday()) as i32;
    let weekday = weekday.days_since(week_start_day) as i32;
    let ordinal = first_week_start + (week as i32 - 1) * 7 + weekday;
    if ordinal <= 0 {
        return Err(IMPOSSIBLE);
    }
    first_day_of_year
        .with_ordinal(ordinal as u32)
        .ok_or(IMPOSSIBLE)
}
