mod time_delta;
pub use time_delta::OutOfRangeError;
pub use time_delta::TimeDelta;
pub type Duration = TimeDelta;
use core::fmt;
mod date;
pub use date::Date;
pub use date::{MAX_DATE, MIN_DATE};
mod datetime;
pub use datetime::DateTime;
pub use datetime::{MAX_DATETIME, MIN_DATETIME};
pub mod format;
pub use format::{ParseError, ParseResult, SecondsFormat};
pub mod naive;
pub use naive::{Days, NaiveDate, NaiveDateTime, NaiveTime};
pub use naive::{IsoWeek, NaiveWeek};
pub mod offset;
pub use offset::LocalResult;
pub use offset::MappedLocalTime;
pub use offset::{FixedOffset, Offset, TimeZone, Utc};
pub mod round;
pub use round::{DurationRound, RoundingError, SubsecRound};
mod weekday;
pub use weekday::ParseWeekdayError;
pub use weekday::Weekday;
mod month;
pub use month::ParseMonthError;
pub use month::{Month, Months};
mod traits;
pub use traits::{Datelike, Timelike};
pub mod serde {
    pub use super::datetime::serde::*;
    use core::fmt;
    use serde::de;
    pub(crate) fn invalid_ts<E, T>(value: T) -> E
    where
        E: de::Error,
        T: fmt::Display,
    {
        E::custom(SerdeError::InvalidTimestamp(value))
    }
    enum SerdeError<T: fmt::Display> {
        InvalidTimestamp(T),
    }
    impl<T: fmt::Display> fmt::Display for SerdeError<T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                SerdeError::InvalidTimestamp(ts) => {
                    write!(f, "value is not a legal timestamp: {ts}")
                }
            }
        }
    }
}
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct OutOfRange {
    _private: (),
}
impl OutOfRange {
    const fn new() -> OutOfRange {
        OutOfRange { _private: () }
    }
}
impl fmt::Display for OutOfRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "out of range")
    }
}
impl fmt::Debug for OutOfRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "out of range")
    }
}
impl std::error::Error for OutOfRange {}
macro_rules! try_opt {
    ($ e : expr) => {
        match $e {
            Some(v) => v,
            None => return None,
        }
    };
}
pub(crate) const fn expect<T: Copy>(opt: Option<T>, msg: &str) -> T {
    match opt {
        Some(val) => val,
        None => panic!("{}", msg),
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/chrono.rs"]
mod tests;

pub(crate) use try_opt;
