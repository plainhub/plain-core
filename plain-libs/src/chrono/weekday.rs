use crate::chrono::OutOfRange;
use core::fmt;
#[derive(PartialEq, Eq, Copy, Clone, Debug, Hash)]
#[cfg_attr(all(any(), all()), derive(arbitrary::Arbitrary))]
#[cfg_attr(any(), derive(defmt::Format))]
pub enum Weekday {
    Mon = 0,
    Tue = 1,
    Wed = 2,
    Thu = 3,
    Fri = 4,
    Sat = 5,
    Sun = 6,
}
impl Weekday {
    #[inline]
    #[must_use]
    pub const fn succ(&self) -> Weekday {
        match *self {
            Weekday::Mon => Weekday::Tue,
            Weekday::Tue => Weekday::Wed,
            Weekday::Wed => Weekday::Thu,
            Weekday::Thu => Weekday::Fri,
            Weekday::Fri => Weekday::Sat,
            Weekday::Sat => Weekday::Sun,
            Weekday::Sun => Weekday::Mon,
        }
    }
    #[inline]
    #[must_use]
    pub const fn pred(&self) -> Weekday {
        match *self {
            Weekday::Mon => Weekday::Sun,
            Weekday::Tue => Weekday::Mon,
            Weekday::Wed => Weekday::Tue,
            Weekday::Thu => Weekday::Wed,
            Weekday::Fri => Weekday::Thu,
            Weekday::Sat => Weekday::Fri,
            Weekday::Sun => Weekday::Sat,
        }
    }
    #[inline]
    pub const fn number_from_monday(&self) -> u32 {
        self.days_since(Weekday::Mon) + 1
    }

    #[inline]
    pub const fn num_days_from_monday(&self) -> u32 {
        self.days_since(Weekday::Mon)
    }
    #[inline]
    pub const fn num_days_from_sunday(&self) -> u32 {
        self.days_since(Weekday::Sun)
    }
    pub const fn days_since(&self, other: Weekday) -> u32 {
        let lhs = *self as u32;
        let rhs = other as u32;
        if lhs < rhs { 7 + lhs - rhs } else { lhs - rhs }
    }
}
impl fmt::Display for Weekday {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(match *self {
            Weekday::Mon => "Mon",
            Weekday::Tue => "Tue",
            Weekday::Wed => "Wed",
            Weekday::Thu => "Thu",
            Weekday::Fri => "Fri",
            Weekday::Sat => "Sat",
            Weekday::Sun => "Sun",
        })
    }
}
impl TryFrom<u8> for Weekday {
    type Error = OutOfRange;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Weekday::Mon),
            1 => Ok(Weekday::Tue),
            2 => Ok(Weekday::Wed),
            3 => Ok(Weekday::Thu),
            4 => Ok(Weekday::Fri),
            5 => Ok(Weekday::Sat),
            6 => Ok(Weekday::Sun),
            _ => Err(OutOfRange::new()),
        }
    }
}
impl num_traits::FromPrimitive for Weekday {
    #[inline]
    fn from_i64(n: i64) -> Option<Weekday> {
        match n {
            0 => Some(Weekday::Mon),
            1 => Some(Weekday::Tue),
            2 => Some(Weekday::Wed),
            3 => Some(Weekday::Thu),
            4 => Some(Weekday::Fri),
            5 => Some(Weekday::Sat),
            6 => Some(Weekday::Sun),
            _ => None,
        }
    }
    #[inline]
    fn from_u64(n: u64) -> Option<Weekday> {
        match n {
            0 => Some(Weekday::Mon),
            1 => Some(Weekday::Tue),
            2 => Some(Weekday::Wed),
            3 => Some(Weekday::Thu),
            4 => Some(Weekday::Fri),
            5 => Some(Weekday::Sat),
            6 => Some(Weekday::Sun),
            _ => None,
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct ParseWeekdayError {
    pub(crate) _dummy: (),
}
impl std::error::Error for ParseWeekdayError {}
impl fmt::Display for ParseWeekdayError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!("{self:?}"))
    }
}
impl fmt::Debug for ParseWeekdayError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ParseWeekdayError {{ .. }}")
    }
}
mod weekday_serde {
    use super::Weekday;
    use core::fmt;
    use serde::{de, ser};
    impl ser::Serialize for Weekday {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: ser::Serializer,
        {
            serializer.collect_str(&self)
        }
    }
    struct WeekdayVisitor;
    impl de::Visitor<'_> for WeekdayVisitor {
        type Value = Weekday;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("Weekday")
        }
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            value
                .parse()
                .map_err(|_| E::custom("short or long weekday names expected"))
        }
    }
    impl<'de> de::Deserialize<'de> for Weekday {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            deserializer.deserialize_str(WeekdayVisitor)
        }
    }
}
