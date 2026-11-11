use super::{INVALID, OUT_OF_RANGE, ParseResult, TOO_SHORT};
use crate::chrono::Weekday;
#[inline]
pub(super) fn number(s: &str, min: usize, max: usize) -> ParseResult<(&str, i64)> {
    assert!(min <= max);
    let bytes = s.as_bytes();
    if bytes.len() < min {
        return Err(TOO_SHORT);
    }
    let mut n = 0i64;
    for (i, c) in bytes.iter().take(max).cloned().enumerate() {
        if !c.is_ascii_digit() {
            if i < min {
                return Err(INVALID);
            } else {
                return Ok((&s[i..], n));
            }
        }
        n = match n
            .checked_mul(10)
            .and_then(|n| n.checked_add((c - b'0') as i64))
        {
            Some(n) => n,
            None => return Err(OUT_OF_RANGE),
        };
    }
    Ok((&s[core::cmp::min(max, bytes.len())..], n))
}
pub(super) fn nanosecond(s: &str) -> ParseResult<(&str, u32)> {
    let origlen = s.len();
    let (s, v) = number(s, 1, 9)?;
    let v = u32::try_from(v).expect("999,999,999 should fit u32");
    let consumed = origlen - s.len();
    const SCALE: [u32; 10] = [
        0,
        100_000_000,
        10_000_000,
        1_000_000,
        100_000,
        10_000,
        1_000,
        100,
        10,
        1,
    ];
    let v = v.checked_mul(SCALE[consumed]).ok_or(OUT_OF_RANGE)?;
    let s = s.trim_start_matches(|c: char| c.is_ascii_digit());
    Ok((s, v))
}
pub(super) fn nanosecond_fixed(s: &str, digits: usize) -> ParseResult<(&str, i64)> {
    let (s, v) = number(s, digits, digits)?;
    static SCALE: [i64; 10] = [
        0,
        100_000_000,
        10_000_000,
        1_000_000,
        100_000,
        10_000,
        1_000,
        100,
        10,
        1,
    ];
    let v = v.checked_mul(SCALE[digits]).ok_or(OUT_OF_RANGE)?;
    Ok((s, v))
}
pub(super) fn short_month0(s: &str) -> ParseResult<(&str, u8)> {
    if s.len() < 3 {
        return Err(TOO_SHORT);
    }
    let buf = s.as_bytes();
    let month0 = match (buf[0] | 32, buf[1] | 32, buf[2] | 32) {
        (b'j', b'a', b'n') => 0,
        (b'f', b'e', b'b') => 1,
        (b'm', b'a', b'r') => 2,
        (b'a', b'p', b'r') => 3,
        (b'm', b'a', b'y') => 4,
        (b'j', b'u', b'n') => 5,
        (b'j', b'u', b'l') => 6,
        (b'a', b'u', b'g') => 7,
        (b's', b'e', b'p') => 8,
        (b'o', b'c', b't') => 9,
        (b'n', b'o', b'v') => 10,
        (b'd', b'e', b'c') => 11,
        _ => return Err(INVALID),
    };
    Ok((&s[3..], month0))
}
pub(super) fn short_weekday(s: &str) -> ParseResult<(&str, Weekday)> {
    if s.len() < 3 {
        return Err(TOO_SHORT);
    }
    let buf = s.as_bytes();
    let weekday = match (buf[0] | 32, buf[1] | 32, buf[2] | 32) {
        (b'm', b'o', b'n') => Weekday::Mon,
        (b't', b'u', b'e') => Weekday::Tue,
        (b'w', b'e', b'd') => Weekday::Wed,
        (b't', b'h', b'u') => Weekday::Thu,
        (b'f', b'r', b'i') => Weekday::Fri,
        (b's', b'a', b't') => Weekday::Sat,
        (b's', b'u', b'n') => Weekday::Sun,
        _ => return Err(INVALID),
    };
    Ok((&s[3..], weekday))
}
pub(super) fn short_or_long_month0(s: &str) -> ParseResult<(&str, u8)> {
    static LONG_MONTH_SUFFIXES: [&[u8]; 12] = [
        b"uary", b"ruary", b"ch", b"il", b"", b"e", b"y", b"ust", b"tember", b"ober", b"ember",
        b"ember",
    ];
    let (mut s, month0) = short_month0(s)?;
    let suffix = LONG_MONTH_SUFFIXES[month0 as usize];
    if s.len() >= suffix.len() && s.as_bytes()[..suffix.len()].eq_ignore_ascii_case(suffix) {
        s = &s[suffix.len()..];
    }
    Ok((s, month0))
}
pub(super) fn short_or_long_weekday(s: &str) -> ParseResult<(&str, Weekday)> {
    static LONG_WEEKDAY_SUFFIXES: [&[u8]; 7] = [
        b"day", b"sday", b"nesday", b"rsday", b"day", b"urday", b"day",
    ];
    let (mut s, weekday) = short_weekday(s)?;
    let suffix = LONG_WEEKDAY_SUFFIXES[weekday.num_days_from_monday() as usize];
    if s.len() >= suffix.len() && s.as_bytes()[..suffix.len()].eq_ignore_ascii_case(suffix) {
        s = &s[suffix.len()..];
    }
    Ok((s, weekday))
}
pub(super) fn char(s: &str, c1: u8) -> ParseResult<&str> {
    match s.as_bytes().first() {
        Some(&c) if c == c1 => Ok(&s[1..]),
        Some(_) => Err(INVALID),
        None => Err(TOO_SHORT),
    }
}
pub(super) fn space(s: &str) -> ParseResult<&str> {
    let s_ = s.trim_start();
    if s_.len() < s.len() {
        Ok(s_)
    } else if s.is_empty() {
        Err(TOO_SHORT)
    } else {
        Err(INVALID)
    }
}
pub(crate) fn colon_or_space(s: &str) -> ParseResult<&str> {
    Ok(s.trim_start_matches(|c: char| c == ':' || c.is_whitespace()))
}
pub(crate) fn timezone_offset<F>(
    mut s: &str,
    mut consume_colon: F,
    allow_zulu: bool,
    allow_missing_minutes: bool,
    allow_tz_minus_sign: bool,
) -> ParseResult<(&str, i32)>
where
    F: FnMut(&str) -> ParseResult<&str>,
{
    if allow_zulu {
        if let Some(&b'Z' | &b'z') = s.as_bytes().first() {
            return Ok((&s[1..], 0));
        }
    }
    const fn digits(s: &str) -> ParseResult<(u8, u8)> {
        let b = s.as_bytes();
        if b.len() < 2 {
            Err(TOO_SHORT)
        } else {
            Ok((b[0], b[1]))
        }
    }
    let negative = match s.chars().next() {
        Some('+') => {
            s = &s['+'.len_utf8()..];
            false
        }
        Some('-') => {
            s = &s['-'.len_utf8()..];
            true
        }
        Some('−') => {
            if !allow_tz_minus_sign {
                return Err(INVALID);
            }
            s = &s['−'.len_utf8()..];
            true
        }
        Some(_) => return Err(INVALID),
        None => return Err(TOO_SHORT),
    };
    let hours = match digits(s)? {
        (h1 @ b'0'..=b'9', h2 @ b'0'..=b'9') => i32::from((h1 - b'0') * 10 + (h2 - b'0')),
        _ => return Err(INVALID),
    };
    s = &s[2..];
    s = consume_colon(s)?;
    let minutes = if let Ok(ds) = digits(s) {
        match ds {
            (m1 @ b'0'..=b'5', m2 @ b'0'..=b'9') => i32::from((m1 - b'0') * 10 + (m2 - b'0')),
            (b'6'..=b'9', b'0'..=b'9') => return Err(OUT_OF_RANGE),
            _ => return Err(INVALID),
        }
    } else if allow_missing_minutes {
        0
    } else {
        return Err(TOO_SHORT);
    };
    s = match s.len() {
        len if len >= 2 => &s[2..],
        0 => s,
        _ => return Err(TOO_SHORT),
    };
    let seconds = hours * 3600 + minutes * 60;
    Ok((s, if negative { -seconds } else { seconds }))
}
pub(super) fn timezone_offset_2822(s: &str) -> ParseResult<(&str, i32)> {
    let upto = s
        .as_bytes()
        .iter()
        .position(|&c| !c.is_ascii_alphabetic())
        .unwrap_or(s.len());
    if upto > 0 {
        let name = &s.as_bytes()[..upto];
        let s = &s[upto..];
        let offset_hours = |o| Ok((s, o * 3600));
        if name.eq_ignore_ascii_case(b"gmt")
            || name.eq_ignore_ascii_case(b"ut")
            || name.eq_ignore_ascii_case(b"z")
        {
            return offset_hours(0);
        } else if name.eq_ignore_ascii_case(b"edt") {
            return offset_hours(-4);
        } else if name.eq_ignore_ascii_case(b"est") || name.eq_ignore_ascii_case(b"cdt") {
            return offset_hours(-5);
        } else if name.eq_ignore_ascii_case(b"cst") || name.eq_ignore_ascii_case(b"mdt") {
            return offset_hours(-6);
        } else if name.eq_ignore_ascii_case(b"mst") || name.eq_ignore_ascii_case(b"pdt") {
            return offset_hours(-7);
        } else if name.eq_ignore_ascii_case(b"pst") {
            return offset_hours(-8);
        } else if name.len() == 1 {
            if let b'a'..=b'i' | b'k'..=b'y' | b'A'..=b'I' | b'K'..=b'Y' = name[0] {
                return Ok((s, 0));
            }
        }
        Err(INVALID)
    } else {
        timezone_offset(s, |s| Ok(s), false, false, false)
    }
}
pub(super) fn comment_2822(s: &str) -> ParseResult<(&str, ())> {
    use CommentState::*;
    let s = s.trim_start();
    let mut state = Start;
    for (i, c) in s.bytes().enumerate() {
        state = match (state, c) {
            (Start, b'(') => Next(1),
            (Next(1), b')') => return Ok((&s[i + 1..], ())),
            (Next(depth), b'\\') => Escape(depth),
            (Next(depth), b'(') => Next(depth + 1),
            (Next(depth), b')') => Next(depth - 1),
            (Next(depth), _) | (Escape(depth), _) => Next(depth),
            _ => return Err(INVALID),
        };
    }
    Err(TOO_SHORT)
}
enum CommentState {
    Start,
    Next(usize),
    Escape(usize),
}
