use crate::local_time::Local;
use chrono::{DateTime, NaiveDate, NaiveDateTime, Offset, TimeDelta, TimeZone};

pub fn parse(value: &str) -> Option<i64> {
    parse_in(value, &Local)
}

pub fn parse_in<T: TimeZone>(value: &str, zone: &T) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value.bytes().all(|byte| byte.is_ascii_digit()) {
        let number = value.parse::<i64>().ok()?;
        return if value.len() >= 13 {
            Some(number)
        } else {
            number.checked_mul(1000)
        };
    }
    let date_only = value.len() == 10;
    let pattern = match value.len() {
        10 => "dddd-dd-dd",
        16 => "dddd-dd-ddTdd:dd",
        19 => "dddd-dd-ddTdd:dd:dd",
        _ => return None,
    };
    if !value.bytes().zip(pattern.bytes()).all(|(byte, expected)| {
        if expected == b'd' {
            byte.is_ascii_digit()
        } else {
            byte == expected
        }
    }) {
        return None;
    }
    let local = if date_only {
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .ok()?
            .and_hms_opt(0, 0, 0)?
    } else {
        let format = if value.len() == 16 {
            "%Y-%m-%dT%H:%M"
        } else {
            "%Y-%m-%dT%H:%M:%S"
        };
        NaiveDateTime::parse_from_str(value, format).ok()?
    };
    if local.and_utc().timestamp_subsec_nanos() != 0 {
        return None;
    }
    if let Some(instant) = zone.from_local_datetime(&local).earliest() {
        return Some(instant.timestamp_millis());
    }
    if date_only {
        return Some(gap_boundary(local, zone, 1)?.timestamp_millis());
    }
    let before = gap_boundary(local, zone, -1)?;
    before
        .offset()
        .fix()
        .from_local_datetime(&local)
        .single()
        .map(|instant| instant.timestamp_millis())
}

fn gap_boundary<T: TimeZone>(
    local: NaiveDateTime,
    zone: &T,
    direction: i64,
) -> Option<DateTime<T>> {
    let at = |seconds: i64| {
        let delta = TimeDelta::try_seconds(seconds.checked_mul(direction)?)?;
        local.checked_add_signed(delta)
    };
    let mut invalid = 0;
    let mut valid = 1i64;
    let mut instant = loop {
        if let Some(instant) = zone.from_local_datetime(&at(valid)?).earliest() {
            break instant;
        }
        invalid = valid;
        valid = valid.checked_mul(2)?;
    };
    while valid - invalid > 1 {
        let middle = invalid + (valid - invalid) / 2;
        if let Some(candidate) = zone.from_local_datetime(&at(middle)?).earliest() {
            valid = middle;
            instant = candidate;
        } else {
            invalid = middle;
        }
    }
    Some(instant)
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/utils/epoch_millis.rs"]
mod tests;
