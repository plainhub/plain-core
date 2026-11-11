use crate::zip::result::DateTimeRangeError;
use core::fmt;
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DateTime {
    datepart: u16,
    timepart: u16,
}
impl fmt::Debug for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Self::default() {
            return f.write_str("DateTime::default()");
        }
        f.write_fmt(format_args!(
            "DateTime::from_date_and_time({}, {}, {}, {}, {}, {})?",
            self.year(),
            self.month(),
            self.day(),
            self.hour(),
            self.minute(),
            self.second()
        ))
    }
}
impl TryFrom<(u16, u16)> for DateTime {
    type Error = DateTimeRangeError;
    #[inline]
    fn try_from(values: (u16, u16)) -> Result<Self, Self::Error> {
        Self::try_from_msdos(values.0, values.1)
    }
}
impl From<DateTime> for (u16, u16) {
    #[inline]
    fn from(dt: DateTime) -> Self {
        (dt.datepart(), dt.timepart())
    }
}
impl Default for DateTime {
    fn default() -> DateTime {
        DateTime::DEFAULT
    }
}
impl fmt::Display for DateTime {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year(),
            self.month(),
            self.day(),
            self.hour(),
            self.minute(),
            self.second()
        )
    }
}
impl DateTime {
    pub const DEFAULT: Self = DateTime {
        datepart: 0b0000_0000_0010_0001,
        timepart: 0,
    };
    #[must_use]
    pub fn default_for_write() -> Self {
        DateTime::default()
    }

    pub(crate) fn is_leap_year(year: u16) -> bool {
        year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
    }
    pub fn try_from_msdos(datepart: u16, timepart: u16) -> Result<DateTime, DateTimeRangeError> {
        let seconds = (timepart & 0b0000_0000_0001_1111) << 1;
        let minutes = (timepart & 0b0000_0111_1110_0000) >> 5;
        let hours = (timepart & 0b1111_1000_0000_0000) >> 11;
        let days = datepart & 0b0000_0000_0001_1111;
        let months = (datepart & 0b0000_0001_1110_0000) >> 5;
        let years = (datepart & 0b1111_1110_0000_0000) >> 9;
        Self::from_date_and_time(
            years.checked_add(1980).ok_or(DateTimeRangeError)?,
            months.try_into()?,
            days.try_into()?,
            hours.try_into()?,
            minutes.try_into()?,
            seconds.try_into()?,
        )
    }
    pub fn from_date_and_time(
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
    ) -> Result<DateTime, DateTimeRangeError> {
        if (1980..=2107).contains(&year)
            && (1..=12).contains(&month)
            && (1..=31).contains(&day)
            && hour <= 23
            && minute <= 59
            && second <= 60
        {
            let second = second.min(58);
            let max_day = match month {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                2 if Self::is_leap_year(year) => 29,
                2 => 28,
                _ => unreachable!(),
            };
            if day > max_day {
                return Err(DateTimeRangeError);
            }
            let datepart = u16::from(day) | (u16::from(month) << 5) | ((year - 1980) << 9);
            let timepart =
                (u16::from(second) >> 1) | (u16::from(minute) << 5) | (u16::from(hour) << 11);
            Ok(DateTime { datepart, timepart })
        } else {
            Err(DateTimeRangeError)
        }
    }
    #[must_use]
    pub fn is_valid(&self) -> bool {
        Self::try_from_msdos(self.datepart, self.timepart).is_ok()
    }
    #[must_use]
    pub const fn timepart(&self) -> u16 {
        self.timepart
    }
    #[must_use]
    pub const fn datepart(&self) -> u16 {
        self.datepart
    }
    #[must_use]
    pub const fn year(&self) -> u16 {
        (self.datepart >> 9) + 1980
    }
    #[must_use]
    pub const fn month(&self) -> u8 {
        ((self.datepart & 0b0000_0001_1110_0000) >> 5) as u8
    }
    #[must_use]
    pub const fn day(&self) -> u8 {
        (self.datepart & 0b0000_0000_0001_1111) as u8
    }
    #[must_use]
    pub const fn hour(&self) -> u8 {
        (self.timepart >> 11) as u8
    }
    #[must_use]
    pub const fn minute(&self) -> u8 {
        ((self.timepart & 0b0000_0111_1110_0000) >> 5) as u8
    }
    #[must_use]
    pub const fn second(&self) -> u8 {
        ((self.timepart & 0b0000_0000_0001_1111) << 1) as u8
    }
}
