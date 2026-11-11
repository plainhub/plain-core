use super::scan;
use super::{BAD_FORMAT, INVALID, OUT_OF_RANGE, TOO_LONG, TOO_SHORT};
use super::{Fixed, InternalFixed, InternalInternal, Item, Numeric, Pad, Parsed};
use super::{ParseError, ParseResult};
use crate::chrono::{DateTime, FixedOffset, MappedLocalTime, NaiveDate, NaiveTime, Weekday};
use core::borrow::Borrow;
use core::str;
fn set_weekday_with_num_days_from_sunday(p: &mut Parsed, v: i64) -> ParseResult<()> {
    p.set_weekday(match v {
        0 => Weekday::Sun,
        1 => Weekday::Mon,
        2 => Weekday::Tue,
        3 => Weekday::Wed,
        4 => Weekday::Thu,
        5 => Weekday::Fri,
        6 => Weekday::Sat,
        _ => return Err(OUT_OF_RANGE),
    })
}
fn set_weekday_with_number_from_monday(p: &mut Parsed, v: i64) -> ParseResult<()> {
    p.set_weekday(match v {
        1 => Weekday::Mon,
        2 => Weekday::Tue,
        3 => Weekday::Wed,
        4 => Weekday::Thu,
        5 => Weekday::Fri,
        6 => Weekday::Sat,
        7 => Weekday::Sun,
        _ => return Err(OUT_OF_RANGE),
    })
}
fn parse_rfc2822<'a>(parsed: &mut Parsed, mut s: &'a str) -> ParseResult<(&'a str, ())> {
    macro_rules! try_consume {
        ($ e : expr) => {{
            let (s_, v) = $e?;
            s = s_;
            v
        }};
    }
    s = s.trim_start();
    if let Ok((s_, weekday)) = scan::short_weekday(s) {
        if !s_.starts_with(',') {
            return Err(INVALID);
        }
        s = &s_[1..];
        parsed.set_weekday(weekday)?;
    }
    s = s.trim_start();
    parsed.set_day(try_consume!(scan::number(s, 1, 2)))?;
    s = scan::space(s)?;
    parsed.set_month(1 + i64::from(try_consume!(scan::short_month0(s))))?;
    s = scan::space(s)?;
    let prevlen = s.len();
    let mut year = try_consume!(scan::number(s, 2, usize::MAX));
    let yearlen = prevlen - s.len();
    match (yearlen, year) {
        (2, 0..=49) => {
            year += 2000;
        }
        (2, 50..=99) => {
            year += 1900;
        }
        (3, _) => {
            year += 1900;
        }
        (_, _) => {}
    }
    parsed.set_year(year)?;
    s = scan::space(s)?;
    parsed.set_hour(try_consume!(scan::number(s, 2, 2)))?;
    s = scan::char(s.trim_start(), b':')?.trim_start();
    parsed.set_minute(try_consume!(scan::number(s, 2, 2)))?;
    if let Ok(s_) = scan::char(s.trim_start(), b':') {
        parsed.set_second(try_consume!(scan::number(s_, 2, 2)))?;
    }
    s = scan::space(s)?;
    parsed.set_offset(i64::from(try_consume!(scan::timezone_offset_2822(s))))?;
    while let Ok((s_out, ())) = scan::comment_2822(s) {
        s = s_out;
    }
    Ok((s, ()))
}
pub(crate) fn parse_rfc3339(mut s: &str) -> ParseResult<DateTime<FixedOffset>> {
    macro_rules! try_consume {
        ($ e : expr) => {{
            let (s_, v) = $e?;
            s = s_;
            v
        }};
    }
    let bytes = s.as_bytes();
    if bytes.len() < 19 {
        return Err(TOO_SHORT);
    }
    let fixed = <&[u8; 19]>::try_from(&bytes[..19]).unwrap();
    let year = digit(fixed, 0)? as u16 * 1000
        + digit(fixed, 1)? as u16 * 100
        + digit(fixed, 2)? as u16 * 10
        + digit(fixed, 3)? as u16;
    if bytes.get(4) != Some(&b'-') {
        return Err(INVALID);
    }
    let month = digit(fixed, 5)? * 10 + digit(fixed, 6)?;
    if bytes.get(7) != Some(&b'-') {
        return Err(INVALID);
    }
    let day = digit(fixed, 8)? * 10 + digit(fixed, 9)?;
    let date =
        NaiveDate::from_ymd_opt(year as i32, month as u32, day as u32).ok_or(OUT_OF_RANGE)?;
    if !matches!(bytes.get(10), Some(&b't' | &b'T' | &b' ')) {
        return Err(INVALID);
    }
    let hour = digit(fixed, 11)? * 10 + digit(fixed, 12)?;
    if bytes.get(13) != Some(&b':') {
        return Err(INVALID);
    }
    let min = digit(fixed, 14)? * 10 + digit(fixed, 15)?;
    if bytes.get(16) != Some(&b':') {
        return Err(INVALID);
    }
    let sec = digit(fixed, 17)? * 10 + digit(fixed, 18)?;
    let (sec, extra_nanos) = match sec {
        60 => (59, 1_000_000_000),
        _ => (sec, 0),
    };
    let nano = if bytes.get(19) == Some(&b'.') {
        let nanosecond = try_consume!(scan::nanosecond(&s[20..]));
        extra_nanos + nanosecond
    } else {
        s = &s[19..];
        extra_nanos
    };
    let time = NaiveTime::from_hms_nano_opt(hour as u32, min as u32, sec as u32, nano)
        .ok_or(OUT_OF_RANGE)?;
    let offset = try_consume!(scan::timezone_offset(
        s,
        |s| scan::char(s, b':'),
        true,
        false,
        true
    ));
    if !s.is_empty() {
        return Err(TOO_LONG);
    }
    let tz = FixedOffset::east_opt(offset).ok_or(OUT_OF_RANGE)?;
    Ok(match date.and_time(time).and_local_timezone(tz) {
        MappedLocalTime::Single(dt) => dt,
        MappedLocalTime::Ambiguous(_, _) | MappedLocalTime::None => unreachable!(),
    })
}
#[inline]
fn digit(bytes: &[u8; 19], index: usize) -> ParseResult<u8> {
    match bytes[index].is_ascii_digit() {
        true => Ok(bytes[index] - b'0'),
        false => Err(INVALID),
    }
}
pub fn parse<'a, I, B>(parsed: &mut Parsed, s: &str, items: I) -> ParseResult<()>
where
    I: Iterator<Item = B>,
    B: Borrow<Item<'a>>,
{
    match parse_internal(parsed, s, items) {
        Ok("") => Ok(()),
        Ok(_) => Err(TOO_LONG),
        Err(e) => Err(e),
    }
}
pub fn parse_and_remainder<'a, 'b, I, B>(
    parsed: &mut Parsed,
    s: &'b str,
    items: I,
) -> ParseResult<&'b str>
where
    I: Iterator<Item = B>,
    B: Borrow<Item<'a>>,
{
    parse_internal(parsed, s, items)
}
fn parse_internal<'a, 'b, I, B>(
    parsed: &mut Parsed,
    mut s: &'b str,
    items: I,
) -> Result<&'b str, ParseError>
where
    I: Iterator<Item = B>,
    B: Borrow<Item<'a>>,
{
    macro_rules! try_consume {
        ($ e : expr) => {{
            match $e {
                Ok((s_, v)) => {
                    s = s_;
                    v
                }
                Err(e) => return Err(e),
            }
        }};
    }
    for item in items {
        match *item.borrow() {
            Item::Literal(prefix) => {
                if s.len() < prefix.len() {
                    return Err(TOO_SHORT);
                }
                if !s.starts_with(prefix) {
                    return Err(INVALID);
                }
                s = &s[prefix.len()..];
            }
            Item::OwnedLiteral(ref prefix) => {
                if s.len() < prefix.len() {
                    return Err(TOO_SHORT);
                }
                if !s.starts_with(&prefix[..]) {
                    return Err(INVALID);
                }
                s = &s[prefix.len()..];
            }
            Item::Space(_) => {
                s = s.trim_start();
            }
            Item::OwnedSpace(_) => {
                s = s.trim_start();
            }
            Item::Numeric(ref spec, ref _pad) => {
                use super::Numeric::*;
                type Setter = fn(&mut Parsed, i64) -> ParseResult<()>;
                let (width, signed, set): (usize, bool, Setter) = match *spec {
                    Year => (4, true, Parsed::set_year),
                    YearDiv100 => (2, false, Parsed::set_year_div_100),
                    YearMod100 => (2, false, Parsed::set_year_mod_100),
                    IsoYear => (4, true, Parsed::set_isoyear),
                    IsoYearDiv100 => (2, false, Parsed::set_isoyear_div_100),
                    IsoYearMod100 => (2, false, Parsed::set_isoyear_mod_100),
                    Quarter => (1, false, Parsed::set_quarter),
                    Month => (2, false, Parsed::set_month),
                    Day => (2, false, Parsed::set_day),
                    WeekFromSun => (2, false, Parsed::set_week_from_sun),
                    WeekFromMon => (2, false, Parsed::set_week_from_mon),
                    IsoWeek => (2, false, Parsed::set_isoweek),
                    NumDaysFromSun => (1, false, set_weekday_with_num_days_from_sunday),
                    WeekdayFromMon => (1, false, set_weekday_with_number_from_monday),
                    Ordinal => (3, false, Parsed::set_ordinal),
                    Hour => (2, false, Parsed::set_hour),
                    Hour12 => (2, false, Parsed::set_hour12),
                    Minute => (2, false, Parsed::set_minute),
                    Second => (2, false, Parsed::set_second),
                    Nanosecond => (9, false, Parsed::set_nanosecond),
                    Timestamp => (usize::MAX, false, Parsed::set_timestamp),
                    Internal(ref int) => match int._dummy {},
                };
                s = s.trim_start();
                let v = if signed {
                    if s.starts_with('-') {
                        let v = try_consume!(scan::number(&s[1..], 1, usize::MAX));
                        0i64.checked_sub(v).ok_or(OUT_OF_RANGE)?
                    } else if s.starts_with('+') {
                        try_consume!(scan::number(&s[1..], 1, usize::MAX))
                    } else {
                        try_consume!(scan::number(s, 1, width))
                    }
                } else {
                    try_consume!(scan::number(s, 1, width))
                };
                set(parsed, v)?;
            }
            Item::Fixed(ref spec) => {
                use super::Fixed::*;
                match spec {
                    &ShortMonthName => {
                        let month0 = try_consume!(scan::short_month0(s));
                        parsed.set_month(i64::from(month0) + 1)?;
                    }
                    &LongMonthName => {
                        let month0 = try_consume!(scan::short_or_long_month0(s));
                        parsed.set_month(i64::from(month0) + 1)?;
                    }
                    &ShortWeekdayName => {
                        let weekday = try_consume!(scan::short_weekday(s));
                        parsed.set_weekday(weekday)?;
                    }
                    &LongWeekdayName => {
                        let weekday = try_consume!(scan::short_or_long_weekday(s));
                        parsed.set_weekday(weekday)?;
                    }
                    &LowerAmPm | &UpperAmPm => {
                        if s.len() < 2 {
                            return Err(TOO_SHORT);
                        }
                        let ampm = match (s.as_bytes()[0] | 32, s.as_bytes()[1] | 32) {
                            (b'a', b'm') => false,
                            (b'p', b'm') => true,
                            _ => return Err(INVALID),
                        };
                        parsed.set_ampm(ampm)?;
                        s = &s[2..];
                    }
                    &Nanosecond => {
                        if s.starts_with('.') {
                            let nano = try_consume!(scan::nanosecond(&s[1..]));
                            parsed.set_nanosecond(nano as i64)?;
                        }
                    }
                    &Nanosecond3 => {
                        if s.starts_with('.') {
                            let nano = try_consume!(scan::nanosecond_fixed(&s[1..], 3));
                            parsed.set_nanosecond(nano)?;
                        }
                    }
                    &Nanosecond6 => {
                        if s.starts_with('.') {
                            let nano = try_consume!(scan::nanosecond_fixed(&s[1..], 6));
                            parsed.set_nanosecond(nano)?;
                        }
                    }
                    &Nanosecond9 => {
                        if s.starts_with('.') {
                            let nano = try_consume!(scan::nanosecond_fixed(&s[1..], 9));
                            parsed.set_nanosecond(nano)?;
                        }
                    }
                    &Internal(InternalFixed {
                        val: InternalInternal::Nanosecond3NoDot,
                    }) => {
                        if s.len() < 3 {
                            return Err(TOO_SHORT);
                        }
                        let nano = try_consume!(scan::nanosecond_fixed(s, 3));
                        parsed.set_nanosecond(nano)?;
                    }
                    &Internal(InternalFixed {
                        val: InternalInternal::Nanosecond6NoDot,
                    }) => {
                        if s.len() < 6 {
                            return Err(TOO_SHORT);
                        }
                        let nano = try_consume!(scan::nanosecond_fixed(s, 6));
                        parsed.set_nanosecond(nano)?;
                    }
                    &Internal(InternalFixed {
                        val: InternalInternal::Nanosecond9NoDot,
                    }) => {
                        if s.len() < 9 {
                            return Err(TOO_SHORT);
                        }
                        let nano = try_consume!(scan::nanosecond_fixed(s, 9));
                        parsed.set_nanosecond(nano)?;
                    }
                    &TimezoneName => {
                        try_consume!(Ok((s.trim_start_matches(|c: char| !c.is_whitespace()), ())));
                    }
                    &TimezoneOffsetColon
                    | &TimezoneOffsetDoubleColon
                    | &TimezoneOffsetTripleColon
                    | &TimezoneOffset => {
                        let offset = try_consume!(scan::timezone_offset(
                            s.trim_start(),
                            scan::colon_or_space,
                            false,
                            false,
                            true,
                        ));
                        parsed.set_offset(i64::from(offset))?;
                    }
                    &TimezoneOffsetColonZ | &TimezoneOffsetZ => {
                        let offset = try_consume!(scan::timezone_offset(
                            s.trim_start(),
                            scan::colon_or_space,
                            true,
                            false,
                            true,
                        ));
                        parsed.set_offset(i64::from(offset))?;
                    }
                    &Internal(InternalFixed {
                        val: InternalInternal::TimezoneOffsetPermissive,
                    }) => {
                        let offset = try_consume!(scan::timezone_offset(
                            s.trim_start(),
                            scan::colon_or_space,
                            true,
                            true,
                            true,
                        ));
                        parsed.set_offset(i64::from(offset))?;
                    }
                    &RFC2822 => try_consume!(parse_rfc2822(parsed, s)),
                    &RFC3339 => {
                        try_consume!(parse_rfc3339_relaxed(parsed, s))
                    }
                }
            }
            Item::Error => {
                return Err(BAD_FORMAT);
            }
        }
    }
    Ok(s)
}
impl str::FromStr for DateTime<FixedOffset> {
    type Err = ParseError;
    fn from_str(s: &str) -> ParseResult<DateTime<FixedOffset>> {
        let mut parsed = Parsed::new();
        let (s, _) = parse_rfc3339_relaxed(&mut parsed, s)?;
        if !s.trim_start().is_empty() {
            return Err(TOO_LONG);
        }
        parsed.to_datetime()
    }
}
fn parse_rfc3339_relaxed<'a>(parsed: &mut Parsed, mut s: &'a str) -> ParseResult<(&'a str, ())> {
    const DATE_ITEMS: &[Item<'static>] = &[
        Item::Numeric(Numeric::Year, Pad::Zero),
        Item::Space(""),
        Item::Literal("-"),
        Item::Numeric(Numeric::Month, Pad::Zero),
        Item::Space(""),
        Item::Literal("-"),
        Item::Numeric(Numeric::Day, Pad::Zero),
    ];
    const TIME_ITEMS: &[Item<'static>] = &[
        Item::Numeric(Numeric::Hour, Pad::Zero),
        Item::Space(""),
        Item::Literal(":"),
        Item::Numeric(Numeric::Minute, Pad::Zero),
        Item::Space(""),
        Item::Literal(":"),
        Item::Numeric(Numeric::Second, Pad::Zero),
        Item::Fixed(Fixed::Nanosecond),
        Item::Space(""),
    ];
    s = parse_internal(parsed, s, DATE_ITEMS.iter())?;
    s = match s.as_bytes().first() {
        Some(&b't' | &b'T' | &b' ') => &s[1..],
        Some(_) => return Err(INVALID),
        None => return Err(TOO_SHORT),
    };
    s = parse_internal(parsed, s, TIME_ITEMS.iter())?;
    s = s.trim_start();
    let (s, offset) = if s.len() >= 3 && "UTC".as_bytes().eq_ignore_ascii_case(&s.as_bytes()[..3]) {
        (&s[3..], 0)
    } else {
        scan::timezone_offset(s, scan::colon_or_space, true, false, true)?
    };
    parsed.set_offset(i64::from(offset))?;
    Ok((s, ()))
}
