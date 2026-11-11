// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cmp::Ordering;
use std::mem::MaybeUninit;
use std::ptr;

use super::win_bindings::{GetTimeZoneInformationForYear, SystemTime, TimeZoneInformation};

use super::{Transition, lookup_with_dst_transitions, shift_seconds};
use crate::chrono::{
    Datelike, FixedOffset, MappedLocalTime, NaiveDate, NaiveDateTime, NaiveTime, Weekday,
};

// We don't use `SystemTimeToTzSpecificLocalTime` because it doesn't support the same range of dates
// as Chrono. Also it really isn't that difficult to work out the correct offset from the provided
// DST rules.
//
// This method uses `overflowing_sub_offset` because it is no problem if the transition time in UTC
// falls a couple of hours inside the buffer space around the `NaiveDateTime` range (although it is
// very theoretical to have a transition at midnight around `NaiveDate::(MIN|MAX)`.
pub(super) fn offset_from_utc_datetime(utc: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
    // Using a `TzInfo` based on the year of an UTC datetime is technically wrong, we should be
    // using the rules for the year of the corresponding local time. But this matches what
    // `SystemTimeToTzSpecificLocalTime` is documented to do.
    let tz_info = match TzInfo::for_year(utc.year()) {
        Some(tz_info) => tz_info,
        None => return MappedLocalTime::None,
    };
    let utc = &shift_seconds(*utc, 0);
    let offset = match (tz_info.std_transition, tz_info.dst_transition) {
        (Some(std_transition), Some(dst_transition)) => {
            let std_transition_utc =
                shift_seconds(std_transition, -tz_info.dst_offset.local_minus_utc());
            let dst_transition_utc =
                shift_seconds(dst_transition, -tz_info.std_offset.local_minus_utc());
            if dst_transition_utc < std_transition_utc {
                match utc >= &dst_transition_utc && utc < &std_transition_utc {
                    true => tz_info.dst_offset,
                    false => tz_info.std_offset,
                }
            } else {
                match utc >= &std_transition_utc && utc < &dst_transition_utc {
                    true => tz_info.std_offset,
                    false => tz_info.dst_offset,
                }
            }
        }
        (Some(std_transition), None) => {
            let std_transition_utc =
                shift_seconds(std_transition, -tz_info.dst_offset.local_minus_utc());
            match utc < &std_transition_utc {
                true => tz_info.dst_offset,
                false => tz_info.std_offset,
            }
        }
        (None, Some(dst_transition)) => {
            let dst_transition_utc =
                shift_seconds(dst_transition, -tz_info.std_offset.local_minus_utc());
            match utc < &dst_transition_utc {
                true => tz_info.std_offset,
                false => tz_info.dst_offset,
            }
        }
        (None, None) => tz_info.std_offset,
    };
    MappedLocalTime::Single(offset)
}

// We don't use `TzSpecificLocalTimeToSystemTime` because it doesn't let us choose how to handle
// ambiguous cases (during a DST transition). Instead we get the timezone information for the
// current year and compute it ourselves, like we do on Unix.
pub(super) fn offset_from_local_datetime(local: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
    let tz_info = match TzInfo::for_year(local.year()) {
        Some(tz_info) => tz_info,
        None => return MappedLocalTime::None,
    };
    // Create a sorted slice of transitions and use `lookup_with_dst_transitions`.
    match (tz_info.std_transition, tz_info.dst_transition) {
        (Some(std_transition), Some(dst_transition)) => {
            let std_transition =
                Transition::new(std_transition, tz_info.dst_offset, tz_info.std_offset);
            let dst_transition =
                Transition::new(dst_transition, tz_info.std_offset, tz_info.dst_offset);
            let transitions = match std_transition.cmp(&dst_transition) {
                Ordering::Less => [std_transition, dst_transition],
                Ordering::Greater => [dst_transition, std_transition],
                Ordering::Equal => {
                    // This doesn't make sense. Let's just return the standard offset.
                    return MappedLocalTime::Single(tz_info.std_offset);
                }
            };
            lookup_with_dst_transitions(&transitions, *local)
        }
        (Some(std_transition), None) => {
            let transitions = [Transition::new(
                std_transition,
                tz_info.dst_offset,
                tz_info.std_offset,
            )];
            lookup_with_dst_transitions(&transitions, *local)
        }
        (None, Some(dst_transition)) => {
            let transitions = [Transition::new(
                dst_transition,
                tz_info.std_offset,
                tz_info.dst_offset,
            )];
            lookup_with_dst_transitions(&transitions, *local)
        }
        (None, None) => MappedLocalTime::Single(tz_info.std_offset),
    }
}

// The basis for Windows timezone and DST support has been in place since Windows 2000. It does not
// allow for complex rules like the IANA timezone database:
// - A timezone has the same base offset the whole year.
// - There seem to be either zero or two DST transitions (but we support having just one).
// - As of Vista(?) only years from 2004 until a few years into the future are supported.
// - All other years get the base settings, which seem to be that of the current year.
//
// These details don't matter much, we just work with the offsets and transition dates Windows
// returns through `GetTimeZoneInformationForYear` for a particular year.
struct TzInfo {
    // Offset from UTC during standard time.
    std_offset: FixedOffset,
    // Offset from UTC during daylight saving time.
    dst_offset: FixedOffset,
    // Transition from standard time to daylight saving time, given in local standard time.
    std_transition: Option<NaiveDateTime>,
    // Transition from daylight saving time to standard time, given in local daylight saving time.
    dst_transition: Option<NaiveDateTime>,
}

impl TzInfo {
    fn for_year(year: i32) -> Option<TzInfo> {
        // The API limits years to 1601..=30827.
        // Working with timezones and daylight saving time this far into the past or future makes
        // little sense. But whatever is extrapolated for 1601 or 30827 is what can be extrapolated
        // for years beyond.
        let ref_year = year.clamp(1601, 30827) as u16;
        let tz_info = unsafe {
            let mut tz_info = MaybeUninit::<TimeZoneInformation>::uninit();
            if GetTimeZoneInformationForYear(ref_year, ptr::null_mut(), tz_info.as_mut_ptr()) == 0 {
                return None;
            }
            tz_info.assume_init()
        };
        let std_offset = (tz_info.bias)
            .checked_add(tz_info.standard_bias)
            .and_then(|o| o.checked_mul(60))
            .and_then(FixedOffset::west_opt)?;
        let dst_offset = (tz_info.bias)
            .checked_add(tz_info.daylight_bias)
            .and_then(|o| o.checked_mul(60))
            .and_then(FixedOffset::west_opt)?;
        Some(TzInfo {
            std_offset,
            dst_offset,
            std_transition: naive_date_time_from_system_time(tz_info.standard_date, year).ok()?,
            dst_transition: naive_date_time_from_system_time(tz_info.daylight_date, year).ok()?,
        })
    }
}

/// Resolve a `SystemTime` object to an `Option<NaiveDateTime>`.
///
/// A `SystemTime` within a `TimeZoneInformation` struct can be zero to indicate there is no
/// transition.
/// If it has year, month and day values it is a concrete date.
/// If the year is missing the `SystemTime` is a rule, which this method resolves for the provided
/// year. A rule has a month, weekday, and nth weekday of the month as components.
///
/// Returns `Err` if any of the values is invalid, which should never happen.
fn naive_date_time_from_system_time(
    st: SystemTime,
    year: i32,
) -> Result<Option<NaiveDateTime>, ()> {
    if st.year == 0 && st.month == 0 {
        return Ok(None);
    }
    let time = NaiveTime::from_hms_milli_opt(
        st.hour as u32,
        st.minute as u32,
        st.second as u32,
        st.milliseconds as u32,
    )
    .ok_or(())?;

    if st.year != 0 {
        // We have a concrete date.
        let date =
            NaiveDate::from_ymd_opt(st.year as i32, st.month as u32, st.day as u32).ok_or(())?;
        return Ok(Some(date.and_time(time)));
    }

    // Resolve a rule with month, weekday, and nth weekday of the month to a date in the current
    // year.
    let weekday = match st.day_of_week {
        0 => Weekday::Sun,
        1 => Weekday::Mon,
        2 => Weekday::Tue,
        3 => Weekday::Wed,
        4 => Weekday::Thu,
        5 => Weekday::Fri,
        6 => Weekday::Sat,
        _ => return Err(()),
    };
    let nth_day = match st.day {
        1..=5 => st.day as u8,
        _ => return Err(()),
    };
    let date = NaiveDate::from_weekday_of_month_opt(year, st.month as u32, weekday, nth_day)
        .or_else(|| NaiveDate::from_weekday_of_month_opt(year, st.month as u32, weekday, 4))
        .ok_or(())?; // `st.month` must be invalid
    Ok(Some(date.and_time(time)))
}
