use super::{MappedLocalTime, Offset, TimeZone};
use crate::chrono::format::{OUT_OF_RANGE, ParseError, scan};
use crate::chrono::naive::{NaiveDate, NaiveDateTime};
use core::fmt;
use core::str::FromStr;
#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub struct FixedOffset {
    local_minus_utc: i32,
}
impl FixedOffset {
    #[must_use]
    pub fn east(secs: i32) -> FixedOffset {
        FixedOffset::east_opt(secs).expect("FixedOffset::east out of bounds")
    }
    #[must_use]
    pub const fn east_opt(secs: i32) -> Option<FixedOffset> {
        if -86_400 < secs && secs < 86_400 {
            Some(FixedOffset {
                local_minus_utc: secs,
            })
        } else {
            None
        }
    }
    #[must_use]
    pub fn west(secs: i32) -> FixedOffset {
        FixedOffset::west_opt(secs).expect("FixedOffset::west out of bounds")
    }
    #[must_use]
    pub const fn west_opt(secs: i32) -> Option<FixedOffset> {
        if -86_400 < secs && secs < 86_400 {
            Some(FixedOffset {
                local_minus_utc: -secs,
            })
        } else {
            None
        }
    }
    #[inline]
    pub const fn local_minus_utc(&self) -> i32 {
        self.local_minus_utc
    }
}
impl FromStr for FixedOffset {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (_, offset) = scan::timezone_offset(s, scan::colon_or_space, false, false, true)?;
        Self::east_opt(offset).ok_or(OUT_OF_RANGE)
    }
}
impl TimeZone for FixedOffset {
    type Offset = FixedOffset;
    fn from_offset(offset: &FixedOffset) -> FixedOffset {
        *offset
    }
    fn offset_from_local_date(&self, _local: &NaiveDate) -> MappedLocalTime<FixedOffset> {
        MappedLocalTime::Single(*self)
    }
    fn offset_from_local_datetime(&self, _local: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
        MappedLocalTime::Single(*self)
    }
    fn offset_from_utc_date(&self, _utc: &NaiveDate) -> FixedOffset {
        *self
    }
    fn offset_from_utc_datetime(&self, _utc: &NaiveDateTime) -> FixedOffset {
        *self
    }
}
impl Offset for FixedOffset {
    fn fix(&self) -> FixedOffset {
        *self
    }
}
impl fmt::Debug for FixedOffset {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let offset = self.local_minus_utc;
        let (sign, offset) = if offset < 0 {
            ('-', -offset)
        } else {
            ('+', offset)
        };
        let sec = offset.rem_euclid(60);
        let mins = offset.div_euclid(60);
        let min = mins.rem_euclid(60);
        let hour = mins.div_euclid(60);
        if sec == 0 {
            write!(f, "{sign}{hour:02}:{min:02}")
        } else {
            write!(f, "{sign}{hour:02}:{min:02}:{sec:02}")
        }
    }
}
impl fmt::Display for FixedOffset {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
