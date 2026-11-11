use crate::chrono::OutOfRange;
use crate::chrono::naive::NaiveDate;
use core::fmt;
#[derive(PartialEq, Eq, Copy, Clone, Debug, Hash, PartialOrd, Ord)]
#[cfg_attr(all(any(), all()), derive(arbitrary::Arbitrary))]
#[cfg_attr(any(), derive(defmt::Format))]
pub enum Month {
    January = 0,
    February = 1,
    March = 2,
    April = 3,
    May = 4,
    June = 5,
    July = 6,
    August = 7,
    September = 8,
    October = 9,
    November = 10,
    December = 11,
}
impl Month {
    #[inline]
    #[must_use]
    pub const fn succ(&self) -> Month {
        match *self {
            Month::January => Month::February,
            Month::February => Month::March,
            Month::March => Month::April,
            Month::April => Month::May,
            Month::May => Month::June,
            Month::June => Month::July,
            Month::July => Month::August,
            Month::August => Month::September,
            Month::September => Month::October,
            Month::October => Month::November,
            Month::November => Month::December,
            Month::December => Month::January,
        }
    }
    #[inline]
    #[must_use]
    pub const fn pred(&self) -> Month {
        match *self {
            Month::January => Month::December,
            Month::February => Month::January,
            Month::March => Month::February,
            Month::April => Month::March,
            Month::May => Month::April,
            Month::June => Month::May,
            Month::July => Month::June,
            Month::August => Month::July,
            Month::September => Month::August,
            Month::October => Month::September,
            Month::November => Month::October,
            Month::December => Month::November,
        }
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        match *self {
            Month::January => "January",
            Month::February => "February",
            Month::March => "March",
            Month::April => "April",
            Month::May => "May",
            Month::June => "June",
            Month::July => "July",
            Month::August => "August",
            Month::September => "September",
            Month::October => "October",
            Month::November => "November",
            Month::December => "December",
        }
    }
    pub fn num_days(&self, year: i32) -> Option<u8> {
        Some(match *self {
            Month::January => 31,
            Month::February => match NaiveDate::from_ymd_opt(year, 2, 1)?.leap_year() {
                true => 29,
                false => 28,
            },
            Month::March => 31,
            Month::April => 30,
            Month::May => 31,
            Month::June => 30,
            Month::July => 31,
            Month::August => 31,
            Month::September => 30,
            Month::October => 31,
            Month::November => 30,
            Month::December => 31,
        })
    }
}
impl TryFrom<u8> for Month {
    type Error = OutOfRange;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Month::January),
            2 => Ok(Month::February),
            3 => Ok(Month::March),
            4 => Ok(Month::April),
            5 => Ok(Month::May),
            6 => Ok(Month::June),
            7 => Ok(Month::July),
            8 => Ok(Month::August),
            9 => Ok(Month::September),
            10 => Ok(Month::October),
            11 => Ok(Month::November),
            12 => Ok(Month::December),
            _ => Err(OutOfRange::new()),
        }
    }
}
impl num_traits::FromPrimitive for Month {
    #[inline]
    fn from_u64(n: u64) -> Option<Month> {
        Self::from_u32(n as u32)
    }
    #[inline]
    fn from_i64(n: i64) -> Option<Month> {
        Self::from_u32(n as u32)
    }
    #[inline]
    fn from_u32(n: u32) -> Option<Month> {
        match n {
            1 => Some(Month::January),
            2 => Some(Month::February),
            3 => Some(Month::March),
            4 => Some(Month::April),
            5 => Some(Month::May),
            6 => Some(Month::June),
            7 => Some(Month::July),
            8 => Some(Month::August),
            9 => Some(Month::September),
            10 => Some(Month::October),
            11 => Some(Month::November),
            12 => Some(Month::December),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[cfg_attr(all(any(), all()), derive(arbitrary::Arbitrary))]
#[cfg_attr(any(), derive(defmt::Format))]
pub struct Months(pub(crate) u32);
impl Months {
    pub const fn new(num: u32) -> Self {
        Self(num)
    }
    #[inline]
    pub const fn as_u32(&self) -> u32 {
        self.0
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct ParseMonthError {
    pub(crate) _dummy: (),
}
impl std::error::Error for ParseMonthError {}
impl fmt::Display for ParseMonthError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ParseMonthError {{ .. }}")
    }
}
impl fmt::Debug for ParseMonthError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ParseMonthError {{ .. }}")
    }
}
mod month_serde {
    use super::Month;
    use core::fmt;
    use serde::{de, ser};
    impl ser::Serialize for Month {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: ser::Serializer,
        {
            serializer.collect_str(self.name())
        }
    }
    struct MonthVisitor;
    impl de::Visitor<'_> for MonthVisitor {
        type Value = Month;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("Month")
        }
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            value
                .parse()
                .map_err(|_| E::custom("short (3-letter) or full month names expected"))
        }
    }
    impl<'de> de::Deserialize<'de> for Month {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            deserializer.deserialize_str(MonthVisitor)
        }
    }
}
