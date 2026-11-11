use crate::chrono::format::DelayedFormat;
use crate::chrono::format::{
    Fixed, Item, Numeric, Pad, ParseError, ParseResult, Parsed, StrftimeItems, parse,
    parse_and_remainder, write_hundreds,
};
use crate::chrono::{FixedOffset, TimeDelta, Timelike};
use crate::chrono::{expect, try_opt};
use core::borrow::Borrow;
use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::time::Duration;
use core::{fmt, str};
mod serde;
#[derive(PartialEq, Eq, Hash, PartialOrd, Ord, Copy, Clone)]
pub struct NaiveTime {
    secs: u32,
    frac: u32,
}
impl NaiveTime {
    #[inline]
    #[must_use]
    pub const fn from_hms_opt(hour: u32, min: u32, sec: u32) -> Option<NaiveTime> {
        NaiveTime::from_hms_nano_opt(hour, min, sec, 0)
    }

    #[inline]
    #[must_use]
    pub const fn from_hms_milli_opt(
        hour: u32,
        min: u32,
        sec: u32,
        milli: u32,
    ) -> Option<NaiveTime> {
        let nano = try_opt!(milli.checked_mul(1_000_000));
        NaiveTime::from_hms_nano_opt(hour, min, sec, nano)
    }

    #[inline]
    #[must_use]
    pub const fn from_hms_micro_opt(
        hour: u32,
        min: u32,
        sec: u32,
        micro: u32,
    ) -> Option<NaiveTime> {
        let nano = try_opt!(micro.checked_mul(1_000));
        NaiveTime::from_hms_nano_opt(hour, min, sec, nano)
    }

    #[inline]
    #[must_use]
    pub const fn from_hms_nano_opt(hour: u32, min: u32, sec: u32, nano: u32) -> Option<NaiveTime> {
        if (hour >= 24 || min >= 60 || sec >= 60)
            || (nano >= 1_000_000_000 && sec != 59)
            || nano >= 2_000_000_000
        {
            return None;
        }
        let secs = hour * 3600 + min * 60 + sec;
        Some(NaiveTime { secs, frac: nano })
    }

    #[inline]
    #[must_use]
    pub const fn from_num_seconds_from_midnight_opt(secs: u32, nano: u32) -> Option<NaiveTime> {
        if secs >= 86_400 || nano >= 2_000_000_000 || (nano >= 1_000_000_000 && secs % 60 != 59) {
            return None;
        }
        Some(NaiveTime { secs, frac: nano })
    }
    pub fn parse_from_str(s: &str, fmt: &str) -> ParseResult<NaiveTime> {
        let mut parsed = Parsed::new();
        parse(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_naive_time()
    }
    pub fn parse_and_remainder<'a>(s: &'a str, fmt: &str) -> ParseResult<(NaiveTime, &'a str)> {
        let mut parsed = Parsed::new();
        let remainder = parse_and_remainder(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_naive_time().map(|t| (t, remainder))
    }
    #[must_use]
    pub const fn overflowing_add_signed(&self, rhs: TimeDelta) -> (NaiveTime, i64) {
        let mut secs = self.secs as i64;
        let mut frac = self.frac as i32;
        let secs_to_add = rhs.num_seconds();
        let frac_to_add = rhs.subsec_nanos();
        if frac >= 1_000_000_000 {
            if secs_to_add > 0 || (frac_to_add > 0 && frac >= 2_000_000_000 - frac_to_add) {
                frac -= 1_000_000_000;
            } else if secs_to_add < 0 {
                frac -= 1_000_000_000;
                secs += 1;
            } else {
                return (
                    NaiveTime {
                        secs: self.secs,
                        frac: (frac + frac_to_add) as u32,
                    },
                    0,
                );
            }
        }
        let mut secs = secs + secs_to_add;
        frac += frac_to_add;
        if frac < 0 {
            frac += 1_000_000_000;
            secs -= 1;
        } else if frac >= 1_000_000_000 {
            frac -= 1_000_000_000;
            secs += 1;
        }
        let secs_in_day = secs.rem_euclid(86_400);
        let remaining = secs - secs_in_day;
        (
            NaiveTime {
                secs: secs_in_day as u32,
                frac: frac as u32,
            },
            remaining,
        )
    }
    #[inline]
    #[must_use]
    pub const fn overflowing_sub_signed(&self, rhs: TimeDelta) -> (NaiveTime, i64) {
        let (time, rhs) = self.overflowing_add_signed(rhs.neg());
        (time, -rhs)
    }
    #[must_use]
    pub const fn signed_duration_since(self, rhs: NaiveTime) -> TimeDelta {
        let mut secs = self.secs as i64 - rhs.secs as i64;
        let frac = self.frac as i64 - rhs.frac as i64;
        if self.secs > rhs.secs && rhs.frac >= 1_000_000_000 {
            secs += 1;
        } else if self.secs < rhs.secs && self.frac >= 1_000_000_000 {
            secs -= 1;
        }
        let secs_from_frac = frac.div_euclid(1_000_000_000);
        let frac = frac.rem_euclid(1_000_000_000) as u32;
        expect(
            TimeDelta::new(secs + secs_from_frac, frac),
            "must be in range",
        )
    }
    pub(super) const fn overflowing_add_offset(&self, offset: FixedOffset) -> (NaiveTime, i32) {
        let secs = self.secs as i32 + offset.local_minus_utc();
        let days = secs.div_euclid(86_400);
        let secs = secs.rem_euclid(86_400);
        (
            NaiveTime {
                secs: secs as u32,
                frac: self.frac,
            },
            days,
        )
    }
    pub(super) const fn overflowing_sub_offset(&self, offset: FixedOffset) -> (NaiveTime, i32) {
        let secs = self.secs as i32 - offset.local_minus_utc();
        let days = secs.div_euclid(86_400);
        let secs = secs.rem_euclid(86_400);
        (
            NaiveTime {
                secs: secs as u32,
                frac: self.frac,
            },
            days,
        )
    }
    #[inline]
    #[must_use]
    pub fn format_with_items<'a, I, B>(&self, items: I) -> DelayedFormat<I>
    where
        I: Iterator<Item = B> + Clone,
        B: Borrow<Item<'a>>,
    {
        DelayedFormat::new(None, Some(*self), items)
    }
    #[inline]
    #[must_use]
    pub fn format<'a>(&self, fmt: &'a str) -> DelayedFormat<StrftimeItems<'a>> {
        self.format_with_items(StrftimeItems::new(fmt))
    }
    pub(crate) fn hms(&self) -> (u32, u32, u32) {
        let sec = self.secs % 60;
        let mins = self.secs / 60;
        let min = mins % 60;
        let hour = mins / 60;
        (hour, min, sec)
    }
    #[inline]
    pub(crate) const fn num_seconds_from_midnight(&self) -> u32 {
        self.secs
    }
    #[inline]
    pub(crate) const fn nanosecond(&self) -> u32 {
        self.frac
    }
    pub const MIN: Self = Self { secs: 0, frac: 0 };
    pub(super) const MAX: Self = Self {
        secs: 23 * 3600 + 59 * 60 + 59,
        frac: 999_999_999,
    };
}
impl Timelike for NaiveTime {
    #[inline]
    fn hour(&self) -> u32 {
        self.hms().0
    }
    #[inline]
    fn minute(&self) -> u32 {
        self.hms().1
    }
    #[inline]
    fn second(&self) -> u32 {
        self.hms().2
    }
    #[inline]
    fn nanosecond(&self) -> u32 {
        self.frac
    }
    #[inline]
    fn with_hour(&self, hour: u32) -> Option<NaiveTime> {
        if hour >= 24 {
            return None;
        }
        let secs = hour * 3600 + self.secs % 3600;
        Some(NaiveTime { secs, ..*self })
    }
    #[inline]
    fn with_minute(&self, min: u32) -> Option<NaiveTime> {
        if min >= 60 {
            return None;
        }
        let secs = self.secs / 3600 * 3600 + min * 60 + self.secs % 60;
        Some(NaiveTime { secs, ..*self })
    }
    #[inline]
    fn with_second(&self, sec: u32) -> Option<NaiveTime> {
        if sec >= 60 {
            return None;
        }
        let secs = self.secs / 60 * 60 + sec;
        Some(NaiveTime { secs, ..*self })
    }
    #[inline]
    fn with_nanosecond(&self, nano: u32) -> Option<NaiveTime> {
        if nano >= 2_000_000_000 {
            return None;
        }
        Some(NaiveTime {
            frac: nano,
            ..*self
        })
    }
    #[inline]
    fn num_seconds_from_midnight(&self) -> u32 {
        self.secs
    }
}
impl Add<TimeDelta> for NaiveTime {
    type Output = NaiveTime;
    #[inline]
    fn add(self, rhs: TimeDelta) -> NaiveTime {
        self.overflowing_add_signed(rhs).0
    }
}
impl AddAssign<TimeDelta> for NaiveTime {
    #[inline]
    fn add_assign(&mut self, rhs: TimeDelta) {
        *self = self.add(rhs);
    }
}
impl Add<Duration> for NaiveTime {
    type Output = NaiveTime;
    #[inline]
    fn add(self, rhs: Duration) -> NaiveTime {
        let secs = rhs.as_secs() % (2 * 24 * 60 * 60);
        let d = TimeDelta::new(secs as i64, rhs.subsec_nanos()).unwrap();
        self.overflowing_add_signed(d).0
    }
}
impl AddAssign<Duration> for NaiveTime {
    #[inline]
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs;
    }
}
impl Add<FixedOffset> for NaiveTime {
    type Output = NaiveTime;
    #[inline]
    fn add(self, rhs: FixedOffset) -> NaiveTime {
        self.overflowing_add_offset(rhs).0
    }
}
impl Sub<TimeDelta> for NaiveTime {
    type Output = NaiveTime;
    #[inline]
    fn sub(self, rhs: TimeDelta) -> NaiveTime {
        self.overflowing_sub_signed(rhs).0
    }
}
impl SubAssign<TimeDelta> for NaiveTime {
    #[inline]
    fn sub_assign(&mut self, rhs: TimeDelta) {
        *self = self.sub(rhs);
    }
}
impl Sub<Duration> for NaiveTime {
    type Output = NaiveTime;
    #[inline]
    fn sub(self, rhs: Duration) -> NaiveTime {
        let secs = rhs.as_secs() % (2 * 24 * 60 * 60);
        let d = TimeDelta::new(secs as i64, rhs.subsec_nanos()).unwrap();
        self.overflowing_sub_signed(d).0
    }
}
impl SubAssign<Duration> for NaiveTime {
    #[inline]
    fn sub_assign(&mut self, rhs: Duration) {
        *self = *self - rhs;
    }
}
impl Sub<FixedOffset> for NaiveTime {
    type Output = NaiveTime;
    #[inline]
    fn sub(self, rhs: FixedOffset) -> NaiveTime {
        self.overflowing_sub_offset(rhs).0
    }
}
impl Sub<NaiveTime> for NaiveTime {
    type Output = TimeDelta;
    #[inline]
    fn sub(self, rhs: NaiveTime) -> TimeDelta {
        self.signed_duration_since(rhs)
    }
}
impl fmt::Debug for NaiveTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (hour, min, sec) = self.hms();
        let (sec, nano) = if self.frac >= 1_000_000_000 {
            (sec + 1, self.frac - 1_000_000_000)
        } else {
            (sec, self.frac)
        };
        use core::fmt::Write;
        write_hundreds(f, hour as u8)?;
        f.write_char(':')?;
        write_hundreds(f, min as u8)?;
        f.write_char(':')?;
        write_hundreds(f, sec as u8)?;
        if nano == 0 {
            Ok(())
        } else if nano % 1_000_000 == 0 {
            write!(f, ".{:03}", nano / 1_000_000)
        } else if nano % 1_000 == 0 {
            write!(f, ".{:06}", nano / 1_000)
        } else {
            write!(f, ".{nano:09}")
        }
    }
}
impl fmt::Display for NaiveTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
impl str::FromStr for NaiveTime {
    type Err = ParseError;
    fn from_str(s: &str) -> ParseResult<NaiveTime> {
        const HOUR_AND_MINUTE: &[Item<'static>] = &[
            Item::Numeric(Numeric::Hour, Pad::Zero),
            Item::Space(""),
            Item::Literal(":"),
            Item::Numeric(Numeric::Minute, Pad::Zero),
        ];
        const SECOND_AND_NANOS: &[Item<'static>] = &[
            Item::Space(""),
            Item::Literal(":"),
            Item::Numeric(Numeric::Second, Pad::Zero),
            Item::Fixed(Fixed::Nanosecond),
            Item::Space(""),
        ];
        const TRAILING_WHITESPACE: [Item<'static>; 1] = [Item::Space("")];
        let mut parsed = Parsed::new();
        let s = parse_and_remainder(&mut parsed, s, HOUR_AND_MINUTE.iter())?;
        let s = parse_and_remainder(&mut parsed, s, SECOND_AND_NANOS.iter()).unwrap_or(s);
        parse(&mut parsed, s, TRAILING_WHITESPACE.iter())?;
        parsed.to_naive_time()
    }
}
impl Default for NaiveTime {
    fn default() -> Self {
        NaiveTime::from_hms_opt(0, 0, 0).unwrap()
    }
}
