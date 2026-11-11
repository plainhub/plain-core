use crate::chrono::{Month, ParseMonthError, ParseWeekdayError, Weekday};
use core::fmt;
use core::str::FromStr;
use std::error::Error;
mod formatting;
pub(crate) mod locales;
mod parse;
mod parsed;
pub(crate) mod scan;
pub mod strftime;
pub use formatting::SecondsFormat;
pub(crate) use formatting::write_hundreds;
pub(crate) use formatting::write_rfc2822;
pub(crate) use formatting::write_rfc3339;
pub use formatting::{DelayedFormat, format, format_item};
pub(crate) use parse::parse_rfc3339;
pub use parse::{parse, parse_and_remainder};
pub use parsed::Parsed;
pub use strftime::StrftimeItems;
#[derive(Clone, PartialEq, Eq, Hash)]
enum Void {}
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(any(), derive(defmt::Format))]
pub enum Pad {
    None,
    Zero,
    Space,
}
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(any(), derive(defmt::Format))]
pub enum Numeric {
    Year,
    YearDiv100,
    YearMod100,
    IsoYear,
    IsoYearDiv100,
    IsoYearMod100,
    Quarter,
    Month,
    Day,
    WeekFromSun,
    WeekFromMon,
    IsoWeek,
    NumDaysFromSun,
    WeekdayFromMon,
    Ordinal,
    Hour,
    Hour12,
    Minute,
    Second,
    Nanosecond,
    Timestamp,
    Internal(InternalNumeric),
}
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct InternalNumeric {
    _dummy: Void,
}
impl fmt::Debug for InternalNumeric {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<InternalNumeric>")
    }
}
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(any(), derive(defmt::Format))]
pub enum Fixed {
    ShortMonthName,
    LongMonthName,
    ShortWeekdayName,
    LongWeekdayName,
    LowerAmPm,
    UpperAmPm,
    Nanosecond,
    Nanosecond3,
    Nanosecond6,
    Nanosecond9,
    TimezoneName,
    TimezoneOffsetColon,
    TimezoneOffsetDoubleColon,
    TimezoneOffsetTripleColon,
    TimezoneOffsetColonZ,
    TimezoneOffset,
    TimezoneOffsetZ,
    RFC2822,
    RFC3339,
    Internal(InternalFixed),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(any(), derive(defmt::Format))]
pub struct InternalFixed {
    val: InternalInternal,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(any(), derive(defmt::Format))]
enum InternalInternal {
    TimezoneOffsetPermissive,
    Nanosecond3NoDot,
    Nanosecond6NoDot,
    Nanosecond9NoDot,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(any(), derive(defmt::Format))]
pub struct OffsetFormat {
    pub precision: OffsetPrecision,
    pub colons: Colons,
    pub allow_zulu: bool,
    pub padding: Pad,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(any(), derive(defmt::Format))]
pub enum OffsetPrecision {
    Hours,
    Minutes,
    Seconds,
    OptionalMinutes,
    OptionalSeconds,
    OptionalMinutesAndSeconds,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(any(), derive(defmt::Format))]
pub enum Colons {
    None,
    Colon,
    Maybe,
}
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum Item<'a> {
    Literal(&'a str),
    OwnedLiteral(Box<str>),
    Space(&'a str),
    OwnedSpace(Box<str>),
    Numeric(Numeric, Pad),
    Fixed(Fixed),
    Error,
}
const fn num(numeric: Numeric) -> Item<'static> {
    Item::Numeric(numeric, Pad::None)
}
const fn num0(numeric: Numeric) -> Item<'static> {
    Item::Numeric(numeric, Pad::Zero)
}
const fn nums(numeric: Numeric) -> Item<'static> {
    Item::Numeric(numeric, Pad::Space)
}
const fn fixed(fixed: Fixed) -> Item<'static> {
    Item::Fixed(fixed)
}
const fn internal_fixed(val: InternalInternal) -> Item<'static> {
    Item::Fixed(Fixed::Internal(InternalFixed { val }))
}
impl Item<'_> {
    pub fn to_owned(self) -> Item<'static> {
        match self {
            Item::Literal(s) => Item::OwnedLiteral(Box::from(s)),
            Item::Space(s) => Item::OwnedSpace(Box::from(s)),
            Item::Numeric(n, p) => Item::Numeric(n, p),
            Item::Fixed(f) => Item::Fixed(f),
            Item::OwnedLiteral(l) => Item::OwnedLiteral(l),
            Item::OwnedSpace(s) => Item::OwnedSpace(s),
            Item::Error => Item::Error,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Copy, Hash)]
#[cfg_attr(any(), derive(defmt::Format))]
pub struct ParseError(ParseErrorKind);
impl ParseError {
    pub const fn kind(&self) -> ParseErrorKind {
        self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Copy, Hash)]
#[cfg_attr(any(), derive(defmt::Format))]
pub enum ParseErrorKind {
    OutOfRange,
    Impossible,
    NotEnough,
    Invalid,
    TooShort,
    TooLong,
    BadFormat,
    __Nonexhaustive,
}
pub type ParseResult<T> = Result<T, ParseError>;
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            ParseErrorKind::OutOfRange => write!(f, "input is out of range"),
            ParseErrorKind::Impossible => write!(f, "no possible date and time matching input"),
            ParseErrorKind::NotEnough => write!(f, "input is not enough for unique date and time"),
            ParseErrorKind::Invalid => write!(f, "input contains invalid characters"),
            ParseErrorKind::TooShort => write!(f, "premature end of input"),
            ParseErrorKind::TooLong => write!(f, "trailing input"),
            ParseErrorKind::BadFormat => write!(f, "bad or unsupported format string"),
            _ => unreachable!(),
        }
    }
}
impl Error for ParseError {
    fn description(&self) -> &str {
        "parser error, see to_string() for details"
    }
}
pub(crate) const OUT_OF_RANGE: ParseError = ParseError(ParseErrorKind::OutOfRange);
const IMPOSSIBLE: ParseError = ParseError(ParseErrorKind::Impossible);
const NOT_ENOUGH: ParseError = ParseError(ParseErrorKind::NotEnough);
const INVALID: ParseError = ParseError(ParseErrorKind::Invalid);
const TOO_SHORT: ParseError = ParseError(ParseErrorKind::TooShort);
pub(crate) const TOO_LONG: ParseError = ParseError(ParseErrorKind::TooLong);
const BAD_FORMAT: ParseError = ParseError(ParseErrorKind::BadFormat);
impl FromStr for Weekday {
    type Err = ParseWeekdayError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(("", w)) = scan::short_or_long_weekday(s) {
            Ok(w)
        } else {
            Err(ParseWeekdayError { _dummy: () })
        }
    }
}
impl FromStr for Month {
    type Err = ParseMonthError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(("", w)) = scan::short_or_long_month0(s) {
            match w {
                0 => Ok(Month::January),
                1 => Ok(Month::February),
                2 => Ok(Month::March),
                3 => Ok(Month::April),
                4 => Ok(Month::May),
                5 => Ok(Month::June),
                6 => Ok(Month::July),
                7 => Ok(Month::August),
                8 => Ok(Month::September),
                9 => Ok(Month::October),
                10 => Ok(Month::November),
                11 => Ok(Month::December),
                _ => Err(ParseMonthError { _dummy: () }),
            }
        } else {
            Err(ParseMonthError { _dummy: () })
        }
    }
}
