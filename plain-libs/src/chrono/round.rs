use crate::chrono::{DateTime, NaiveDateTime, TimeDelta, TimeZone, Timelike};
use core::cmp::Ordering;
use core::fmt;
use core::ops::{Add, Sub};
pub trait SubsecRound {
    fn round_subsecs(self, digits: u16) -> Self;
    fn trunc_subsecs(self, digits: u16) -> Self;
}
impl<T> SubsecRound for T
where
    T: Timelike + Add<TimeDelta, Output = T> + Sub<TimeDelta, Output = T>,
{
    fn round_subsecs(self, digits: u16) -> T {
        let span = span_for_digits(digits);
        let delta_down = self.nanosecond() % span;
        if delta_down > 0 {
            let delta_up = span - delta_down;
            if delta_up <= delta_down {
                self + TimeDelta::nanoseconds(delta_up.into())
            } else {
                self - TimeDelta::nanoseconds(delta_down.into())
            }
        } else {
            self
        }
    }
    fn trunc_subsecs(self, digits: u16) -> T {
        let span = span_for_digits(digits);
        let delta_down = self.nanosecond() % span;
        if delta_down > 0 {
            self - TimeDelta::nanoseconds(delta_down.into())
        } else {
            self
        }
    }
}
const fn span_for_digits(digits: u16) -> u32 {
    match digits {
        0 => 1_000_000_000,
        1 => 100_000_000,
        2 => 10_000_000,
        3 => 1_000_000,
        4 => 100_000,
        5 => 10_000,
        6 => 1_000,
        7 => 100,
        8 => 10,
        _ => 1,
    }
}
pub trait DurationRound: Sized {
    type Err: std::error::Error;
    fn duration_round(self, duration: TimeDelta) -> Result<Self, Self::Err>;
    fn duration_trunc(self, duration: TimeDelta) -> Result<Self, Self::Err>;
    fn duration_round_up(self, duration: TimeDelta) -> Result<Self, Self::Err>;
}
impl<Tz: TimeZone> DurationRound for DateTime<Tz> {
    type Err = RoundingError;
    fn duration_round(self, duration: TimeDelta) -> Result<Self, Self::Err> {
        duration_round(self.naive_local(), self, duration)
    }
    fn duration_trunc(self, duration: TimeDelta) -> Result<Self, Self::Err> {
        duration_trunc(self.naive_local(), self, duration)
    }
    fn duration_round_up(self, duration: TimeDelta) -> Result<Self, Self::Err> {
        duration_round_up(self.naive_local(), self, duration)
    }
}
impl DurationRound for NaiveDateTime {
    type Err = RoundingError;
    fn duration_round(self, duration: TimeDelta) -> Result<Self, Self::Err> {
        duration_round(self, self, duration)
    }
    fn duration_trunc(self, duration: TimeDelta) -> Result<Self, Self::Err> {
        duration_trunc(self, self, duration)
    }
    fn duration_round_up(self, duration: TimeDelta) -> Result<Self, Self::Err> {
        duration_round_up(self, self, duration)
    }
}
fn duration_round<T>(
    naive: NaiveDateTime,
    original: T,
    duration: TimeDelta,
) -> Result<T, RoundingError>
where
    T: Timelike + Add<TimeDelta, Output = T> + Sub<TimeDelta, Output = T>,
{
    if let Some(span) = duration.num_nanoseconds() {
        if span <= 0 {
            return Err(RoundingError::DurationExceedsLimit);
        }
        let stamp = naive
            .and_utc()
            .timestamp_nanos_opt()
            .ok_or(RoundingError::TimestampExceedsLimit)?;
        let delta_down = stamp % span;
        if delta_down == 0 {
            Ok(original)
        } else {
            let (delta_up, delta_down) = if delta_down < 0 {
                (delta_down.abs(), span - delta_down.abs())
            } else {
                (span - delta_down, delta_down)
            };
            if delta_up <= delta_down {
                Ok(original + TimeDelta::nanoseconds(delta_up))
            } else {
                Ok(original - TimeDelta::nanoseconds(delta_down))
            }
        }
    } else {
        Err(RoundingError::DurationExceedsLimit)
    }
}
fn duration_trunc<T>(
    naive: NaiveDateTime,
    original: T,
    duration: TimeDelta,
) -> Result<T, RoundingError>
where
    T: Timelike + Add<TimeDelta, Output = T> + Sub<TimeDelta, Output = T>,
{
    if let Some(span) = duration.num_nanoseconds() {
        if span <= 0 {
            return Err(RoundingError::DurationExceedsLimit);
        }
        let stamp = naive
            .and_utc()
            .timestamp_nanos_opt()
            .ok_or(RoundingError::TimestampExceedsLimit)?;
        let delta_down = stamp % span;
        match delta_down.cmp(&0) {
            Ordering::Equal => Ok(original),
            Ordering::Greater => Ok(original - TimeDelta::nanoseconds(delta_down)),
            Ordering::Less => Ok(original - TimeDelta::nanoseconds(span - delta_down.abs())),
        }
    } else {
        Err(RoundingError::DurationExceedsLimit)
    }
}
fn duration_round_up<T>(
    naive: NaiveDateTime,
    original: T,
    duration: TimeDelta,
) -> Result<T, RoundingError>
where
    T: Timelike + Add<TimeDelta, Output = T> + Sub<TimeDelta, Output = T>,
{
    if let Some(span) = duration.num_nanoseconds() {
        if span <= 0 {
            return Err(RoundingError::DurationExceedsLimit);
        }
        let stamp = naive
            .and_utc()
            .timestamp_nanos_opt()
            .ok_or(RoundingError::TimestampExceedsLimit)?;
        let delta_down = stamp % span;
        match delta_down.cmp(&0) {
            Ordering::Equal => Ok(original),
            Ordering::Greater => Ok(original + TimeDelta::nanoseconds(span - delta_down)),
            Ordering::Less => Ok(original + TimeDelta::nanoseconds(delta_down.abs())),
        }
    } else {
        Err(RoundingError::DurationExceedsLimit)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(any(), derive(defmt::Format))]
pub enum RoundingError {
    DurationExceedsTimestamp,
    DurationExceedsLimit,
    TimestampExceedsLimit,
}
impl fmt::Display for RoundingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            RoundingError::DurationExceedsTimestamp => {
                write!(f, "duration in nanoseconds exceeds timestamp")
            }
            RoundingError::DurationExceedsLimit => {
                write!(f, "duration exceeds num_nanoseconds limit")
            }
            RoundingError::TimestampExceedsLimit => {
                write!(f, "timestamp exceeds num_nanoseconds limit")
            }
        }
    }
}
impl std::error::Error for RoundingError {
    fn description(&self) -> &str {
        "error from rounding or truncating with DurationRound"
    }
}
