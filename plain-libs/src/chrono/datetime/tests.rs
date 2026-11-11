use super::DateTime;
use crate::chrono::naive::{NaiveDate, NaiveTime};
use crate::chrono::offset::{FixedOffset, Offset, TimeZone, Utc};
use crate::chrono::{
    Datelike, Days, MappedLocalTime, Months, NaiveDateTime, TimeDelta, Timelike, Weekday,
};
#[derive(Clone)]
struct DstTester;
impl DstTester {
    fn winter_offset() -> FixedOffset {
        FixedOffset::east_opt(8 * 60 * 60).unwrap()
    }
    fn summer_offset() -> FixedOffset {
        FixedOffset::east_opt(9 * 60 * 60).unwrap()
    }
    const TO_WINTER_MONTH_DAY: (u32, u32) = (4, 15);
    const TO_SUMMER_MONTH_DAY: (u32, u32) = (9, 15);
    fn transition_start_local() -> NaiveTime {
        NaiveTime::from_hms_opt(2, 0, 0).unwrap()
    }
}
impl TimeZone for DstTester {
    type Offset = FixedOffset;
    fn from_offset(_: &Self::Offset) -> Self {
        DstTester
    }
    fn offset_from_local_date(
        &self,
        _: &NaiveDate,
    ) -> crate::chrono::MappedLocalTime<Self::Offset> {
        unimplemented!()
    }
    fn offset_from_local_datetime(
        &self,
        local: &NaiveDateTime,
    ) -> crate::chrono::MappedLocalTime<Self::Offset> {
        let local_to_winter_transition_start = NaiveDate::from_ymd_opt(
            local.year(),
            DstTester::TO_WINTER_MONTH_DAY.0,
            DstTester::TO_WINTER_MONTH_DAY.1,
        )
        .unwrap()
        .and_time(DstTester::transition_start_local());
        let local_to_winter_transition_end = NaiveDate::from_ymd_opt(
            local.year(),
            DstTester::TO_WINTER_MONTH_DAY.0,
            DstTester::TO_WINTER_MONTH_DAY.1,
        )
        .unwrap()
        .and_time(DstTester::transition_start_local() - TimeDelta::try_hours(1).unwrap());
        let local_to_summer_transition_start = NaiveDate::from_ymd_opt(
            local.year(),
            DstTester::TO_SUMMER_MONTH_DAY.0,
            DstTester::TO_SUMMER_MONTH_DAY.1,
        )
        .unwrap()
        .and_time(DstTester::transition_start_local());
        let local_to_summer_transition_end = NaiveDate::from_ymd_opt(
            local.year(),
            DstTester::TO_SUMMER_MONTH_DAY.0,
            DstTester::TO_SUMMER_MONTH_DAY.1,
        )
        .unwrap()
        .and_time(DstTester::transition_start_local() + TimeDelta::try_hours(1).unwrap());
        if *local < local_to_winter_transition_end || *local >= local_to_summer_transition_end {
            MappedLocalTime::Single(DstTester::summer_offset())
        } else if *local >= local_to_winter_transition_start
            && *local < local_to_summer_transition_start
        {
            MappedLocalTime::Single(DstTester::winter_offset())
        } else if *local >= local_to_winter_transition_end
            && *local < local_to_winter_transition_start
        {
            MappedLocalTime::Ambiguous(DstTester::winter_offset(), DstTester::summer_offset())
        } else if *local >= local_to_summer_transition_start
            && *local < local_to_summer_transition_end
        {
            MappedLocalTime::None
        } else {
            panic!("Unexpected local time {local}")
        }
    }
    fn offset_from_utc_date(&self, _: &NaiveDate) -> Self::Offset {
        unimplemented!()
    }
    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> Self::Offset {
        let utc_to_winter_transition = NaiveDate::from_ymd_opt(
            utc.year(),
            DstTester::TO_WINTER_MONTH_DAY.0,
            DstTester::TO_WINTER_MONTH_DAY.1,
        )
        .unwrap()
        .and_time(DstTester::transition_start_local())
            - DstTester::summer_offset();
        let utc_to_summer_transition = NaiveDate::from_ymd_opt(
            utc.year(),
            DstTester::TO_SUMMER_MONTH_DAY.0,
            DstTester::TO_SUMMER_MONTH_DAY.1,
        )
        .unwrap()
        .and_time(DstTester::transition_start_local())
            - DstTester::winter_offset();
        if *utc < utc_to_winter_transition || *utc >= utc_to_summer_transition {
            DstTester::summer_offset()
        } else if *utc >= utc_to_winter_transition && *utc < utc_to_summer_transition {
            DstTester::winter_offset()
        } else {
            panic!("Unexpected utc time {utc}")
        }
    }
}
fn ymdhms(
    fixedoffset: &FixedOffset,
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
) -> DateTime<FixedOffset> {
    fixedoffset
        .with_ymd_and_hms(year, month, day, hour, min, sec)
        .unwrap()
}
fn ymdhms_milli(
    fixedoffset: &FixedOffset,
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
    milli: u32,
) -> DateTime<FixedOffset> {
    fixedoffset
        .with_ymd_and_hms(year, month, day, hour, min, sec)
        .unwrap()
        .with_nanosecond(milli * 1_000_000)
        .unwrap()
}
fn ymdhms_micro(
    fixedoffset: &FixedOffset,
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
    micro: u32,
) -> DateTime<FixedOffset> {
    fixedoffset
        .with_ymd_and_hms(year, month, day, hour, min, sec)
        .unwrap()
        .with_nanosecond(micro * 1000)
        .unwrap()
}
fn ymdhms_nano(
    fixedoffset: &FixedOffset,
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
    nano: u32,
) -> DateTime<FixedOffset> {
    fixedoffset
        .with_ymd_and_hms(year, month, day, hour, min, sec)
        .unwrap()
        .with_nanosecond(nano)
        .unwrap()
}
fn ymdhms_utc(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, min, sec)
        .unwrap()
}
fn ymdhms_milli_utc(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
    milli: u32,
) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, min, sec)
        .unwrap()
        .with_nanosecond(milli * 1_000_000)
        .unwrap()
}
