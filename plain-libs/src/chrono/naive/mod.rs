use crate::chrono::Weekday;
use crate::chrono::expect;
use core::hash::{Hash, Hasher};
use core::ops::RangeInclusive;
pub(crate) mod date;
pub(crate) mod datetime;
mod internals;
pub(crate) mod isoweek;
pub(crate) mod time;
pub use self::date::{MAX_DATE, MIN_DATE};
pub use self::date::{NaiveDate, NaiveDateDaysIterator, NaiveDateWeeksIterator};
pub use self::datetime::{MAX_DATETIME, MIN_DATETIME, NaiveDateTime};
pub use self::isoweek::IsoWeek;
pub use self::time::NaiveTime;
#[derive(Clone, Copy, Debug, Eq)]
#[cfg_attr(any(), derive(defmt::Format))]
pub struct NaiveWeek {
    date: NaiveDate,
    start: Weekday,
}
impl NaiveWeek {
    pub(crate) const fn new(date: NaiveDate, start: Weekday) -> Self {
        Self { date, start }
    }
    #[inline]
    #[must_use]
    #[track_caller]
    pub const fn first_day(&self) -> NaiveDate {
        expect(
            self.checked_first_day(),
            "first weekday out of range for `NaiveDate`",
        )
    }
    #[inline]
    #[must_use]
    pub const fn checked_first_day(&self) -> Option<NaiveDate> {
        let start = self.start.num_days_from_monday() as i32;
        let ref_day = self.date.weekday().num_days_from_monday() as i32;
        let days = start - ref_day - if start > ref_day { 7 } else { 0 };
        self.date.add_days(days)
    }

    #[inline]
    #[must_use]
    pub const fn checked_last_day(&self) -> Option<NaiveDate> {
        let end = self.start.pred().num_days_from_monday() as i32;
        let ref_day = self.date.weekday().num_days_from_monday() as i32;
        let days = end - ref_day + if end < ref_day { 7 } else { 0 };
        self.date.add_days(days)
    }
    #[inline]
    #[must_use]
    #[track_caller]
    pub const fn days(&self) -> RangeInclusive<NaiveDate> {
        match self.checked_days() {
            Some(val) => val,
            None => panic!(
                "{}",
                "first or last weekday is out of range for `NaiveDate`"
            ),
        }
    }
    #[inline]
    #[must_use]
    pub const fn checked_days(&self) -> Option<RangeInclusive<NaiveDate>> {
        match (self.checked_first_day(), self.checked_last_day()) {
            (Some(first), Some(last)) => Some(first..=last),
            (_, _) => None,
        }
    }
}
impl PartialEq for NaiveWeek {
    fn eq(&self, other: &Self) -> bool {
        self.first_day() == other.first_day()
    }
}
impl Hash for NaiveWeek {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.first_day().hash(state);
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[cfg_attr(any(), derive(defmt::Format))]
pub struct Days(pub(crate) u64);
impl Days {
    pub const fn new(num: u64) -> Self {
        Self(num)
    }
}
pub mod serde {
    pub use super::datetime::serde::*;
}
