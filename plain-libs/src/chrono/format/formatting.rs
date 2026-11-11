use super::locales;
use super::{Colons, OffsetFormat, OffsetPrecision, Pad};
use super::{Fixed, InternalFixed, InternalInternal, Item, Numeric};
use crate::chrono::offset::Offset;
use crate::chrono::{Datelike, FixedOffset, NaiveDateTime, Timelike};
use crate::chrono::{NaiveDate, NaiveTime, Weekday};
use core::borrow::Borrow;
use core::fmt::Display;
use core::fmt::{self, Write};
use locales::*;
#[derive(Debug)]
pub struct DelayedFormat<I> {
    date: Option<NaiveDate>,
    time: Option<NaiveTime>,
    off: Option<(String, FixedOffset)>,
    items: I,
    locale: Locale,
}
impl<'a, I: Iterator<Item = B> + Clone, B: Borrow<Item<'a>>> DelayedFormat<I> {
    #[must_use]
    pub fn new(date: Option<NaiveDate>, time: Option<NaiveTime>, items: I) -> DelayedFormat<I> {
        DelayedFormat {
            date,
            time,
            off: None,
            items,
            locale: default_locale(),
        }
    }
    #[must_use]
    pub fn new_with_offset<Off>(
        date: Option<NaiveDate>,
        time: Option<NaiveTime>,
        offset: &Off,
        items: I,
    ) -> DelayedFormat<I>
    where
        Off: Offset + Display,
    {
        let name_and_diff = (offset.to_string(), offset.fix());
        DelayedFormat {
            date,
            time,
            off: Some(name_and_diff),
            items,
            locale: default_locale(),
        }
    }
    pub fn write_to(&self, w: &mut (impl Write + ?Sized)) -> fmt::Result {
        for item in self.items.clone() {
            match *item.borrow() {
                Item::Literal(s) | Item::Space(s) => w.write_str(s),
                Item::OwnedLiteral(ref s) | Item::OwnedSpace(ref s) => w.write_str(s),
                Item::Numeric(ref spec, pad) => self.format_numeric(w, spec, pad),
                Item::Fixed(ref spec) => self.format_fixed(w, spec),
                Item::Error => Err(fmt::Error),
            }?;
        }
        Ok(())
    }
    fn format_numeric(
        &self,
        w: &mut (impl Write + ?Sized),
        spec: &Numeric,
        pad: Pad,
    ) -> fmt::Result {
        use self::Numeric::*;
        fn write_one(w: &mut (impl Write + ?Sized), v: u8) -> fmt::Result {
            w.write_char((b'0' + v) as char)
        }
        fn write_two(w: &mut (impl Write + ?Sized), v: u8, pad: Pad) -> fmt::Result {
            let ones = b'0' + v % 10;
            match (v / 10, pad) {
                (0, Pad::None) => {}
                (0, Pad::Space) => w.write_char(' ')?,
                (tens, _) => w.write_char((b'0' + tens) as char)?,
            }
            w.write_char(ones as char)
        }
        #[inline]
        fn write_year(w: &mut (impl Write + ?Sized), year: i32, pad: Pad) -> fmt::Result {
            if (1000..=9999).contains(&year) {
                write_hundreds(w, (year / 100) as u8)?;
                write_hundreds(w, (year % 100) as u8)
            } else {
                write_n(w, 4, year as i64, pad, !(0..10_000).contains(&year))
            }
        }
        fn write_n(
            w: &mut (impl Write + ?Sized),
            n: usize,
            v: i64,
            pad: Pad,
            always_sign: bool,
        ) -> fmt::Result {
            if always_sign {
                match pad {
                    Pad::None => write!(w, "{v:+}"),
                    Pad::Zero => write!(w, "{:+01$}", v, n + 1),
                    Pad::Space => write!(w, "{:+1$}", v, n + 1),
                }
            } else {
                match pad {
                    Pad::None => write!(w, "{v}"),
                    Pad::Zero => write!(w, "{v:0n$}"),
                    Pad::Space => write!(w, "{v:n$}"),
                }
            }
        }
        match (spec, self.date, self.time) {
            (Year, Some(d), _) => write_year(w, d.year(), pad),
            (YearDiv100, Some(d), _) => write_two(w, d.year().div_euclid(100) as u8, pad),
            (YearMod100, Some(d), _) => write_two(w, d.year().rem_euclid(100) as u8, pad),
            (IsoYear, Some(d), _) => write_year(w, d.iso_week().year(), pad),
            (IsoYearDiv100, Some(d), _) => {
                write_two(w, d.iso_week().year().div_euclid(100) as u8, pad)
            }
            (IsoYearMod100, Some(d), _) => {
                write_two(w, d.iso_week().year().rem_euclid(100) as u8, pad)
            }
            (Quarter, Some(d), _) => write_one(w, d.quarter() as u8),
            (Month, Some(d), _) => write_two(w, d.month() as u8, pad),
            (Day, Some(d), _) => write_two(w, d.day() as u8, pad),
            (WeekFromSun, Some(d), _) => write_two(w, d.weeks_from(Weekday::Sun) as u8, pad),
            (WeekFromMon, Some(d), _) => write_two(w, d.weeks_from(Weekday::Mon) as u8, pad),
            (IsoWeek, Some(d), _) => write_two(w, d.iso_week().week() as u8, pad),
            (NumDaysFromSun, Some(d), _) => write_one(w, d.weekday().num_days_from_sunday() as u8),
            (WeekdayFromMon, Some(d), _) => write_one(w, d.weekday().number_from_monday() as u8),
            (Ordinal, Some(d), _) => write_n(w, 3, d.ordinal() as i64, pad, false),
            (Hour, _, Some(t)) => write_two(w, t.hour() as u8, pad),
            (Hour12, _, Some(t)) => write_two(w, t.hour12().1 as u8, pad),
            (Minute, _, Some(t)) => write_two(w, t.minute() as u8, pad),
            (Second, _, Some(t)) => {
                write_two(w, (t.second() + t.nanosecond() / 1_000_000_000) as u8, pad)
            }
            (Nanosecond, _, Some(t)) => {
                write_n(w, 9, (t.nanosecond() % 1_000_000_000) as i64, pad, false)
            }
            (Timestamp, Some(d), Some(t)) => {
                let offset = self
                    .off
                    .as_ref()
                    .map(|(_, o)| i64::from(o.local_minus_utc()));
                let timestamp = d.and_time(t).and_utc().timestamp() - offset.unwrap_or(0);
                write_n(w, 9, timestamp, pad, false)
            }
            (Internal(_), _, _) => Ok(()),
            _ => Err(fmt::Error),
        }
    }
    fn format_fixed(&self, w: &mut (impl Write + ?Sized), spec: &Fixed) -> fmt::Result {
        use Fixed::*;
        use InternalInternal::*;
        match (spec, self.date, self.time, self.off.as_ref()) {
            (ShortMonthName, Some(d), _, _) => {
                w.write_str(short_months(self.locale)[d.month0() as usize])
            }
            (LongMonthName, Some(d), _, _) => {
                w.write_str(long_months(self.locale)[d.month0() as usize])
            }
            (ShortWeekdayName, Some(d), _, _) => w.write_str(
                short_weekdays(self.locale)[d.weekday().num_days_from_sunday() as usize],
            ),
            (LongWeekdayName, Some(d), _, _) => {
                w.write_str(long_weekdays(self.locale)[d.weekday().num_days_from_sunday() as usize])
            }
            (LowerAmPm, _, Some(t), _) => {
                let ampm = if t.hour12().0 {
                    am_pm(self.locale)[1]
                } else {
                    am_pm(self.locale)[0]
                };
                for c in ampm.chars().flat_map(|c| c.to_lowercase()) {
                    w.write_char(c)?
                }
                Ok(())
            }
            (UpperAmPm, _, Some(t), _) => {
                let ampm = if t.hour12().0 {
                    am_pm(self.locale)[1]
                } else {
                    am_pm(self.locale)[0]
                };
                w.write_str(ampm)
            }
            (Nanosecond, _, Some(t), _) => {
                let nano = t.nanosecond() % 1_000_000_000;
                if nano == 0 {
                    Ok(())
                } else {
                    w.write_str(decimal_point(self.locale))?;
                    if nano % 1_000_000 == 0 {
                        write!(w, "{:03}", nano / 1_000_000)
                    } else if nano % 1_000 == 0 {
                        write!(w, "{:06}", nano / 1_000)
                    } else {
                        write!(w, "{nano:09}")
                    }
                }
            }
            (Nanosecond3, _, Some(t), _) => {
                w.write_str(decimal_point(self.locale))?;
                write!(w, "{:03}", t.nanosecond() / 1_000_000 % 1000)
            }
            (Nanosecond6, _, Some(t), _) => {
                w.write_str(decimal_point(self.locale))?;
                write!(w, "{:06}", t.nanosecond() / 1_000 % 1_000_000)
            }
            (Nanosecond9, _, Some(t), _) => {
                w.write_str(decimal_point(self.locale))?;
                write!(w, "{:09}", t.nanosecond() % 1_000_000_000)
            }
            (
                Internal(InternalFixed {
                    val: Nanosecond3NoDot,
                }),
                _,
                Some(t),
                _,
            ) => {
                write!(w, "{:03}", t.nanosecond() / 1_000_000 % 1_000)
            }
            (
                Internal(InternalFixed {
                    val: Nanosecond6NoDot,
                }),
                _,
                Some(t),
                _,
            ) => {
                write!(w, "{:06}", t.nanosecond() / 1_000 % 1_000_000)
            }
            (
                Internal(InternalFixed {
                    val: Nanosecond9NoDot,
                }),
                _,
                Some(t),
                _,
            ) => {
                write!(w, "{:09}", t.nanosecond() % 1_000_000_000)
            }
            (TimezoneName, _, _, Some((tz_name, _))) => write!(w, "{tz_name}"),
            (TimezoneOffset | TimezoneOffsetZ, _, _, Some((_, off))) => {
                let offset_format = OffsetFormat {
                    precision: OffsetPrecision::Minutes,
                    colons: Colons::Maybe,
                    allow_zulu: *spec == TimezoneOffsetZ,
                    padding: Pad::Zero,
                };
                offset_format.format(w, *off)
            }
            (TimezoneOffsetColon | TimezoneOffsetColonZ, _, _, Some((_, off))) => {
                let offset_format = OffsetFormat {
                    precision: OffsetPrecision::Minutes,
                    colons: Colons::Colon,
                    allow_zulu: *spec == TimezoneOffsetColonZ,
                    padding: Pad::Zero,
                };
                offset_format.format(w, *off)
            }
            (TimezoneOffsetDoubleColon, _, _, Some((_, off))) => {
                let offset_format = OffsetFormat {
                    precision: OffsetPrecision::Seconds,
                    colons: Colons::Colon,
                    allow_zulu: false,
                    padding: Pad::Zero,
                };
                offset_format.format(w, *off)
            }
            (TimezoneOffsetTripleColon, _, _, Some((_, off))) => {
                let offset_format = OffsetFormat {
                    precision: OffsetPrecision::Hours,
                    colons: Colons::None,
                    allow_zulu: false,
                    padding: Pad::Zero,
                };
                offset_format.format(w, *off)
            }
            (RFC2822, Some(d), Some(t), Some((_, off))) => {
                write_rfc2822(w, crate::chrono::NaiveDateTime::new(d, t), *off)
            }
            (RFC3339, Some(d), Some(t), Some((_, off))) => write_rfc3339(
                w,
                crate::chrono::NaiveDateTime::new(d, t),
                *off,
                SecondsFormat::AutoSi,
                false,
            ),
            _ => Err(fmt::Error),
        }
    }
}
impl<'a, I: Iterator<Item = B> + Clone, B: Borrow<Item<'a>>> Display for DelayedFormat<I> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut result = String::new();
        self.write_to(&mut result)?;
        f.pad(&result)
    }
}
pub fn format<'a, I, B>(
    w: &mut fmt::Formatter,
    date: Option<&NaiveDate>,
    time: Option<&NaiveTime>,
    off: Option<&(String, FixedOffset)>,
    items: I,
) -> fmt::Result
where
    I: Iterator<Item = B> + Clone,
    B: Borrow<Item<'a>>,
{
    DelayedFormat {
        date: date.copied(),
        time: time.copied(),
        off: off.cloned(),
        items,
        locale: default_locale(),
    }
    .fmt(w)
}
pub fn format_item(
    w: &mut fmt::Formatter,
    date: Option<&NaiveDate>,
    time: Option<&NaiveTime>,
    off: Option<&(String, FixedOffset)>,
    item: &Item<'_>,
) -> fmt::Result {
    DelayedFormat {
        date: date.copied(),
        time: time.copied(),
        off: off.cloned(),
        items: [item].into_iter(),
        locale: default_locale(),
    }
    .fmt(w)
}
impl OffsetFormat {
    fn format(&self, w: &mut (impl Write + ?Sized), off: FixedOffset) -> fmt::Result {
        let off = off.local_minus_utc();
        if self.allow_zulu && off == 0 {
            w.write_char('Z')?;
            return Ok(());
        }
        let (sign, off) = if off < 0 { ('-', -off) } else { ('+', off) };
        let hours;
        let mut mins = 0;
        let mut secs = 0;
        let precision = match self.precision {
            OffsetPrecision::Hours => {
                hours = (off / 3600) as u8;
                OffsetPrecision::Hours
            }
            OffsetPrecision::Minutes | OffsetPrecision::OptionalMinutes => {
                let minutes = (off + 30) / 60;
                mins = (minutes % 60) as u8;
                hours = (minutes / 60) as u8;
                if self.precision == OffsetPrecision::OptionalMinutes && mins == 0 {
                    OffsetPrecision::Hours
                } else {
                    OffsetPrecision::Minutes
                }
            }
            OffsetPrecision::Seconds
            | OffsetPrecision::OptionalSeconds
            | OffsetPrecision::OptionalMinutesAndSeconds => {
                let minutes = off / 60;
                secs = (off % 60) as u8;
                mins = (minutes % 60) as u8;
                hours = (minutes / 60) as u8;
                if self.precision != OffsetPrecision::Seconds && secs == 0 {
                    if self.precision == OffsetPrecision::OptionalMinutesAndSeconds && mins == 0 {
                        OffsetPrecision::Hours
                    } else {
                        OffsetPrecision::Minutes
                    }
                } else {
                    OffsetPrecision::Seconds
                }
            }
        };
        let colons = self.colons == Colons::Colon;
        if hours < 10 {
            if self.padding == Pad::Space {
                w.write_char(' ')?;
            }
            w.write_char(sign)?;
            if self.padding == Pad::Zero {
                w.write_char('0')?;
            }
            w.write_char((b'0' + hours) as char)?;
        } else {
            w.write_char(sign)?;
            write_hundreds(w, hours)?;
        }
        if let OffsetPrecision::Minutes | OffsetPrecision::Seconds = precision {
            if colons {
                w.write_char(':')?;
            }
            write_hundreds(w, mins)?;
        }
        if let OffsetPrecision::Seconds = precision {
            if colons {
                w.write_char(':')?;
            }
            write_hundreds(w, secs)?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(any(), derive(defmt::Format))]
pub enum SecondsFormat {
    Secs,
    Millis,
    Micros,
    Nanos,
    AutoSi,
    __NonExhaustive,
}
#[inline]
pub(crate) fn write_rfc3339(
    w: &mut (impl Write + ?Sized),
    dt: NaiveDateTime,
    off: FixedOffset,
    secform: SecondsFormat,
    use_z: bool,
) -> fmt::Result {
    let year = dt.date().year();
    if (0..=9999).contains(&year) {
        write_hundreds(w, (year / 100) as u8)?;
        write_hundreds(w, (year % 100) as u8)?;
    } else {
        write!(w, "{year:+05}")?;
    }
    w.write_char('-')?;
    write_hundreds(w, dt.date().month() as u8)?;
    w.write_char('-')?;
    write_hundreds(w, dt.date().day() as u8)?;
    w.write_char('T')?;
    let (hour, min, mut sec) = dt.time().hms();
    let mut nano = dt.nanosecond();
    if nano >= 1_000_000_000 {
        sec += 1;
        nano -= 1_000_000_000;
    }
    write_hundreds(w, hour as u8)?;
    w.write_char(':')?;
    write_hundreds(w, min as u8)?;
    w.write_char(':')?;
    let sec = sec;
    write_hundreds(w, sec as u8)?;
    match secform {
        SecondsFormat::Secs => {}
        SecondsFormat::Millis => write!(w, ".{:03}", nano / 1_000_000)?,
        SecondsFormat::Micros => write!(w, ".{:06}", nano / 1000)?,
        SecondsFormat::Nanos => write!(w, ".{nano:09}")?,
        SecondsFormat::AutoSi => {
            if nano == 0 {
            } else if nano % 1_000_000 == 0 {
                write!(w, ".{:03}", nano / 1_000_000)?
            } else if nano % 1_000 == 0 {
                write!(w, ".{:06}", nano / 1_000)?
            } else {
                write!(w, ".{nano:09}")?
            }
        }
        SecondsFormat::__NonExhaustive => unreachable!(),
    };
    OffsetFormat {
        precision: OffsetPrecision::Minutes,
        colons: Colons::Colon,
        allow_zulu: use_z,
        padding: Pad::Zero,
    }
    .format(w, off)
}
pub(crate) fn write_rfc2822(
    w: &mut (impl Write + ?Sized),
    dt: NaiveDateTime,
    off: FixedOffset,
) -> fmt::Result {
    let year = dt.year();
    if !(0..=9999).contains(&year) {
        return Err(fmt::Error);
    }
    let english = default_locale();
    w.write_str(short_weekdays(english)[dt.weekday().num_days_from_sunday() as usize])?;
    w.write_str(", ")?;
    let day = dt.day();
    if day < 10 {
        w.write_char((b'0' + day as u8) as char)?;
    } else {
        write_hundreds(w, day as u8)?;
    }
    w.write_char(' ')?;
    w.write_str(short_months(english)[dt.month0() as usize])?;
    w.write_char(' ')?;
    write_hundreds(w, (year / 100) as u8)?;
    write_hundreds(w, (year % 100) as u8)?;
    w.write_char(' ')?;
    let (hour, min, sec) = dt.time().hms();
    write_hundreds(w, hour as u8)?;
    w.write_char(':')?;
    write_hundreds(w, min as u8)?;
    w.write_char(':')?;
    let sec = sec + dt.nanosecond() / 1_000_000_000;
    write_hundreds(w, sec as u8)?;
    w.write_char(' ')?;
    OffsetFormat {
        precision: OffsetPrecision::Minutes,
        colons: Colons::None,
        allow_zulu: false,
        padding: Pad::Zero,
    }
    .format(w, off)
}
pub(crate) fn write_hundreds(w: &mut (impl Write + ?Sized), n: u8) -> fmt::Result {
    if n >= 100 {
        return Err(fmt::Error);
    }
    let tens = b'0' + n / 10;
    let ones = b'0' + n % 10;
    w.write_char(tens as char)?;
    w.write_char(ones as char)
}
